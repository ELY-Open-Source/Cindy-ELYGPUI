//! `PanelDragController.tsx`: hold a panel, drag it onto another root region, swap them.

use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Duration};

use gpui::{
    Animation, AnimationExt, AnyElement, App, BorrowAppContext, Bounds, Context,
    InteractiveElement, IntoElement, ParentElement, Pixels, Point, Styled, Subscription, Task,
    Window, WindowId, canvas, deferred, div, point, px, relative, size,
};

use super::LayoutRoot;
use crate::layout_tree::{Layout, LayoutStore, Node};
use crate::{i18n::Translator, motion, theme::Appearance, toast};

const LONG_PRESS: Duration = Duration::from_millis(600);
const LONG_PRESS_MOVE_TOLERANCE: f32 = 8.0;
const DROP_ZONE_INSET: f32 = 6.0;
const MIN_DROP_TARGET_SIZE: f32 = 40.0;
const RADIUS_XL: f32 = 12.0;
const RADIUS_MD: f32 = 6.0;
const RADIUS: f32 = 4.0;

/// Painted bounds, by pane kind and by split root child id, and the content area's window origin.
#[derive(Default)]
pub struct Rects {
    pub panes: HashMap<String, Bounds<Pixels>>,
    pub roots: HashMap<String, Bounds<Pixels>>,
    pub origin: Point<Pixels>,
}

pub type SharedRects = Rc<RefCell<Rects>>;

/// `animate-fade-in` on `--motion-fast`, which reduced motion sets to 0.
trait FadeIn: Styled + IntoElement + 'static {
    fn with_fade_in(self, id: &'static str, to: f32, reduced: bool) -> AnyElement {
        if reduced {
            return self.opacity(to).into_any_element();
        }
        let fade = Animation::new(motion::FAST).with_easing(motion::motion_ease_out);
        self.with_animation(id, fade, move |el, d| el.opacity(d * to))
            .into_any_element()
    }
}

impl FadeIn for gpui::Div {}

/// Records the bounds its parent paints at.
pub fn recorder(rects: &SharedRects, pane: bool, key: &str) -> AnyElement {
    let (rects, key) = (rects.clone(), key.to_owned());
    canvas(
        move |bounds, _, _| {
            let mut rects = rects.borrow_mut();
            let map = if pane {
                &mut rects.panes
            } else {
                &mut rects.roots
            };
            map.insert(key, bounds);
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
    .into_any_element()
}

/// Records the content area's origin in the window.
pub fn origin_recorder(rects: &SharedRects) -> AnyElement {
    let rects = rects.clone();
    canvas(
        move |bounds, _, _| rects.borrow_mut().origin = bounds.origin,
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
    .into_any_element()
}

/// `isDroppableRect`: a collapsed or hidden panel has no body to drop on.
pub fn is_droppable(bounds: &Bounds<Pixels>) -> bool {
    bounds.size.width >= px(MIN_DROP_TARGET_SIZE) && bounds.size.height >= px(MIN_DROP_TARGET_SIZE)
}

/// `computePanelDropIntent`: a swap when the pointer is over a pane of another root region.
pub fn is_swap(
    pointer: Point<Pixels>,
    pane: &Bounds<Pixels>,
    source_root: usize,
    target_root: usize,
) -> bool {
    pane.contains(&pointer) && source_root != target_root
}

/// `computePanelDropZone`: the whole root region, 6px in on every side.
pub fn drop_zone(root: &Bounds<Pixels>) -> Bounds<Pixels> {
    let inset = px(DROP_ZONE_INSET);
    let width = (root.size.width - inset * 2.0).max(px(0.0));
    let height = (root.size.height - inset * 2.0).max(px(0.0));
    Bounds {
        origin: root.origin + point(inset, inset),
        size: size(width, height),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    pub kind: String,
    pub pane: Bounds<Pixels>,
    pub root: Bounds<Pixels>,
    pub root_index: usize,
}

fn kinds(node: &Node) -> Vec<&str> {
    match node {
        Node::Pane(pane) => vec![pane.panel_kind.as_str()],
        Node::Split(split) => split.children.iter().flat_map(|c| kinds(&c.node)).collect(),
    }
}

/// `activateDrag`'s geometry: the source's root index and every pane it may land on.
pub fn targets(layout: &Layout, source: &str, rects: &Rects) -> Option<(usize, Vec<Target>)> {
    let Node::Split(root) = &layout.content else {
        return None;
    };
    if root.direction != crate::layout_tree::Direction::Row {
        return None;
    }
    let source_root = root
        .children
        .iter()
        .position(|c| kinds(&c.node).contains(&source))?;
    let mut targets = Vec::new();
    for (root_index, child) in root.children.iter().enumerate() {
        let root_rect = match &child.node {
            Node::Split(split) => rects.roots.get(&split.id),
            Node::Pane(pane) => rects.panes.get(&pane.panel_kind),
        };
        let Some(root_rect) = root_rect.filter(|r| is_droppable(r)) else {
            continue;
        };
        for kind in kinds(&child.node) {
            if kind == source || kind.starts_with("tab-group:") {
                continue;
            }
            let Some(pane) = rects.panes.get(kind).filter(|r| is_droppable(r)) else {
                continue;
            };
            targets.push(Target {
                kind: kind.to_owned(),
                pane: *pane,
                root: *root_rect,
                root_index,
            });
        }
    }
    Some((source_root, targets))
}

pub enum Gesture {
    /// Held still on a panel body; the drag starts after 600ms.
    Pressing {
        kind: String,
        start: Point<Pixels>,
        window: WindowId,
        _timer: Task<()>,
    },
    /// Escape anywhere in the window cancels, as Cindy's capture listener does.
    Dragging {
        kind: String,
        source_root: usize,
        targets: Vec<Target>,
        point: Point<Pixels>,
        target: Option<usize>,
        _escape: Subscription,
    },
}

impl LayoutRoot {
    /// A press on a panel body starts the long press.
    pub(super) fn panel_press(
        &mut self,
        kind: &str,
        at: Point<Pixels>,
        window: WindowId,
        cx: &mut Context<Self>,
    ) {
        if self.gesture.is_some() || self.drag.is_some() || kind.starts_with("tab-group:") {
            return;
        }
        let timer = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(LONG_PRESS).await;
            if let Err(error) = this.update(cx, |this, cx| this.panel_activate(cx)) {
                log::debug!(target: "layout", "layout root gone before the long press: {error}");
            }
        });
        self.gesture = Some(Gesture::Pressing {
            kind: kind.to_owned(),
            start: at,
            window,
            _timer: timer,
        });
        cx.notify();
    }

    fn panel_activate(&mut self, cx: &mut Context<Self>) {
        let Some(Gesture::Pressing {
            kind,
            start,
            window,
            ..
        }) = self.gesture.take()
        else {
            return;
        };
        let layout = cx.global::<LayoutStore>().layout();
        let found = targets(layout, &kind, &self.rects.borrow());
        match found {
            Some((source_root, targets)) if !targets.is_empty() => {
                log::info!(target: "layout", "panel drag {kind} over {} targets", targets.len());
                let root = cx.entity();
                let escape = cx.intercept_keystrokes(move |event, w, cx| {
                    if w.window_handle().window_id() == window && event.keystroke.key == "escape" {
                        root.update(cx, |root, cx| root.panel_finish(false, w, cx));
                    }
                });
                self.gesture = Some(Gesture::Dragging {
                    kind,
                    source_root,
                    targets,
                    point: start,
                    target: None,
                    _escape: escape,
                });
            }
            _ => log::debug!(target: "layout", "panel {kind} has nowhere to land"),
        }
        cx.notify();
    }

    pub(super) fn panel_moved(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        match &mut self.gesture {
            Some(Gesture::Pressing { start, .. }) => {
                let tolerance = px(LONG_PRESS_MOVE_TOLERANCE);
                if (at.x - start.x).abs() > tolerance || (at.y - start.y).abs() > tolerance {
                    self.gesture = None;
                }
            }
            Some(Gesture::Dragging {
                source_root,
                targets,
                point,
                target,
                ..
            }) => {
                *point = at;
                *target = targets
                    .iter()
                    .position(|t| is_swap(at, &t.pane, *source_root, t.root_index));
            }
            None => return,
        }
        cx.notify();
    }

    /// Release commits a swap over a target; Escape and a press release cancel.
    pub(super) fn panel_finish(
        &mut self,
        commit: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(Gesture::Dragging {
            kind,
            targets,
            target,
            ..
        }) = self.gesture.take()
        else {
            self.gesture = None;
            return;
        };
        self.heal_key = None;
        cx.notify();
        let Some(target) = target.filter(|_| commit).map(|i| targets[i].kind.clone()) else {
            return;
        };
        let swapped = cx
            .global::<LayoutStore>()
            .layout()
            .swap_root_children_by_kind(&kind, &target);
        let Ok(next) =
            swapped.map_err(|reason| log::debug!(target: "layout", "swap refused: {reason}"))
        else {
            return;
        };
        let saved = cx.update_global::<LayoutStore, _>(|store, _| store.set(next));
        let t = |key: &str, cx: &App| cx.global::<Translator>().t(key, &[]);
        match saved {
            Ok(true) => {
                log::info!(target: "layout", "swapped {kind} with {target}");
                toast::success(
                    window,
                    t("settings.appearance.layout.saved", cx),
                    toast::Options::default(),
                    cx,
                );
            }
            Ok(false) => {
                toast::error(
                    window,
                    t("settings.appearance.layout.saveFailed", cx),
                    toast::Options::default(),
                    cx,
                );
            }
            Err(reason) => {
                log::warn!(target: "layout", "layout rejected: {reason}");
                toast::error(
                    window,
                    t("settings.appearance.layout.saveFailed", cx),
                    toast::Options::default(),
                    cx,
                );
            }
        }
    }

    pub(super) fn panel_dragging(&self) -> bool {
        matches!(self.gesture, Some(Gesture::Dragging { .. }))
    }

    /// The drop zone and the card that follows the pointer.
    pub(super) fn panel_overlay(&self, cx: &App) -> Option<AnyElement> {
        let Some(Gesture::Dragging {
            targets,
            point,
            target,
            ..
        }) = &self.gesture
        else {
            return None;
        };
        let appearance = cx.global::<Appearance>();
        let ring = appearance.token("focus-ring");
        let origin = self.rects.borrow().origin;
        let reduced = cx.global::<motion::ReducedMotion>().0;
        let zone = target.map(|i| {
            let zone = drop_zone(&targets[i].root);
            let at = zone.origin - origin;
            div()
                .debug_selector(|| "panel-drop-zone".into())
                .absolute()
                .left(at.x)
                .top(at.y)
                .w(zone.size.width)
                .h(zone.size.height)
                .rounded(px(RADIUS_XL))
                .border_1()
                .border_color(ring.opacity(0.55))
                .bg(ring.opacity(0.1))
                .with_fade_in("panel-drop-zone", 1.0, reduced)
        });
        let border = if target.is_some() {
            ring
        } else {
            appearance.token("border-default")
        };
        let bar = |height: f32, width: f32, radius: f32| {
            div()
                .mx_2()
                .h(px(height))
                .w(relative(width))
                .rounded(px(radius))
                .bg(appearance.token("surface-chip"))
        };
        let at = *point - origin - gpui::point(px(85.0), px(110.0 * 0.58));
        let card = div()
            .debug_selector(|| "panel-drag-ghost".into())
            .absolute()
            .left(at.x)
            .top(at.y)
            .w(px(170.0))
            .h(px(110.0))
            .rounded(px(RADIUS_XL))
            .border_1()
            .border_color(border)
            .bg(appearance.token("surface-elevated"))
            .shadow(vec![appearance.shadow_token("shadow-menu")])
            .child(bar(14.0, 0.7, RADIUS_MD).mt_2())
            .child(bar(10.0, 0.85, RADIUS).mt_2().opacity(0.7))
            .child(bar(10.0, 0.6, RADIUS).mt(px(6.0)).opacity(0.7))
            .with_fade_in("panel-drag-ghost", 0.92, reduced);
        let layer = deferred(div().children(zone).child(card)).with_priority(1);
        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .child(layer)
                .into_any_element(),
        )
    }
}
