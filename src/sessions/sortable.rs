//! `SortableList` with native drag, as `PinnedSection` mounts it: the entry under the pointer
//! makes way, a drop on the list saves, anything else puts the order back.

use std::{collections::HashMap, rc::Rc, time::Instant};

use gpui::{
    AnyElement, App, AppContext as _, Bounds, CursorStyle, Element, ElementId, GlobalElementId,
    InspectorElementId, InteractiveElement as _, IntoElement, LayoutId, ParentElement as _, Pixels,
    Point, Render, StatefulInteractiveElement as _, Styled as _, Subscription, Window, div, point,
    px,
};

use super::Sessions;
use crate::{
    motion,
    theme::Appearance,
    units::{self, css},
};

/// SortableJS's `animation: 150`.
const SLIDE: std::time::Duration = std::time::Duration::from_millis(150);

#[derive(Default)]
pub struct State {
    active: Option<Active>,
    /// Each entry's laid-out bounds, from its last prepaint.
    bounds: HashMap<String, Bounds<Pixels>>,
    tracks: HashMap<String, Track>,
    /// Bumped by each move; an entry slides once per move.
    moves: u64,
}

struct Active {
    id: String,
    order: Vec<String>,
    start: Vec<String>,
    _escape: Subscription,
}

struct Track {
    top: Pixels,
    from: Pixels,
    start: Instant,
    moves: u64,
}

impl Track {
    /// Where the slide has the entry now, from its laid-out top.
    fn offset(&self, now: Instant, length: std::time::Duration) -> Pixels {
        let t = match length.is_zero() {
            true => 1.0,
            false => now.saturating_duration_since(self.start).as_secs_f32() / length.as_secs_f32(),
        };
        match t < 1.0 {
            true => self.from * (1.0 - motion::ease(t)),
            false => px(0.0),
        }
    }
}

/// A pinned entry on the move; `State` knows which.
pub struct Dragged;

/// What a drop does with the new order of the shown entries.
pub type OnReorder = Rc<dyn Fn(&[String], &mut Window, &mut App)>;
/// Draws an entry again for the drag image.
pub type Draw = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

/// The entries in the drag's order while one is dragged.
pub fn arrange<T>(entries: Vec<T>, id: impl Fn(&T) -> String, cx: &App) -> Vec<T> {
    let Some(active) = &cx.global::<Sessions>().sortable.active else {
        return entries;
    };
    let mut keyed: Vec<(String, T)> = entries.into_iter().map(|e| (id(&e), e)).collect();
    if keyed.len() != active.order.len() || keyed.iter().any(|(k, _)| !active.order.contains(k)) {
        return keyed.into_iter().map(|(_, e)| e).collect();
    }
    keyed.sort_by_key(|(k, _)| active.order.iter().position(|o| o == k));
    keyed.into_iter().map(|(_, e)| e).collect()
}

/// Forgets entries not painted since the last list render, and drops a drag that ended off
/// the list or lost its window; nothing is saved.
pub fn sweep(window: &mut Window, cx: &mut App) {
    let dragging = cx.has_active_drag();
    let state = &mut cx.global_mut::<Sessions>().sortable;
    let bounds = std::mem::take(&mut state.bounds);
    state.tracks.retain(|id, _| bounds.contains_key(id));
    let Some(active) = &state.active else {
        return;
    };
    let reason = match (dragging, window.is_window_active()) {
        (false, _) => "dropped off the list",
        (true, false) => "the window lost focus",
        (true, true) => return,
    };
    log::info!(target: "sessions", "pinned drag of {} cancelled: {reason}", active.id);
    state.active = None;
    if dragging {
        cx.stop_active_drag(window);
    }
}

fn cancel(window: &mut Window, cx: &mut App) {
    let state = &mut cx.global_mut::<Sessions>().sortable;
    if let Some(active) = state.active.take() {
        log::info!(target: "sessions", "pinned drag of {} cancelled: escape", active.id);
        cx.stop_active_drag(window);
    }
}

fn begin(id: &str, ids: &[String], window: &mut Window, cx: &mut App) {
    let handle = window.window_handle().window_id();
    let escape = cx.intercept_keystrokes(move |event, window, cx| {
        if window.window_handle().window_id() == handle && event.keystroke.key == "escape" {
            cancel(window, cx);
        }
    });
    log::info!(target: "sessions", "pinned drag of {id} started among {}", ids.len());
    cx.global_mut::<Sessions>().sortable.active = Some(Active {
        id: id.to_owned(),
        order: ids.to_vec(),
        start: ids.to_vec(),
        _escape: escape,
    });
}

/// SortableJS's `swapThreshold: 1`: entering an entry takes its place.
fn moved(position: Point<Pixels>, window: &mut Window, cx: &mut App) {
    cx.set_active_drag_cursor_style(CursorStyle::ClosedHand, window);
    let state = &mut cx.global_mut::<Sessions>().sortable;
    let Some(active) = &mut state.active else {
        return;
    };
    let target = active
        .order
        .iter()
        .find(|id| state.bounds.get(*id).is_some_and(|b| b.contains(&position)))
        .cloned();
    let Some(target) = target.filter(|t| *t != active.id) else {
        return;
    };
    let (from, to) = (
        active.order.iter().position(|o| *o == active.id),
        active.order.iter().position(|o| *o == target),
    );
    let (Some(from), Some(to)) = (from, to) else {
        return;
    };
    let id = active.order.remove(from);
    active.order.insert(to, id);
    state.moves += 1;
    window.refresh();
}

fn dropped(on_reorder: &OnReorder, window: &mut Window, cx: &mut App) {
    let Some(active) = cx.global_mut::<Sessions>().sortable.active.take() else {
        return;
    };
    if active.order == active.start {
        log::info!(target: "sessions", "pinned drag of {} dropped in place", active.id);
        return;
    }
    log::info!(target: "sessions", "pinned drag of {} dropped", active.id);
    on_reorder(&active.order, window, cx);
}

/// The list: drags move over it and drop on it.
pub fn list(
    id: &'static str,
    children: Vec<AnyElement>,
    on_reorder: OnReorder,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .on_drag_move::<Dragged>(|event, window, cx| moved(event.event.position, window, cx))
        .on_drop::<Dragged>(move |_, window, cx| dropped(&on_reorder, window, cx))
        .children(children)
}

/// The drag image: the entry drawn again with `xdt-sortable-drag`'s face, held by the pointer
/// where `setDragImage` holds it.
struct Image {
    draw: Draw,
    shift: Point<Pixels>,
    width: Pixels,
}

impl Render for Image {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let appearance = cx.global::<Appearance>();
        div()
            .relative()
            .left(self.shift.x)
            .top(self.shift.y)
            .w(self.width)
            .rounded(css(8.0))
            .bg(appearance.hsl_token("sidebar-item-hover"))
            .shadow(vec![appearance.shadow_token("shadow-menu", cx)])
            .child((self.draw)(window, cx))
    }
}

/// One entry; `ids` are the shown entries in order. A disabled entry does not drag.
pub fn item(
    id: &str,
    ids: Rc<Vec<String>>,
    enabled: bool,
    child: AnyElement,
    draw: Draw,
    cx: &App,
) -> AnyElement {
    let ghost = cx
        .global::<Sessions>()
        .sortable
        .active
        .as_ref()
        .is_some_and(|a| a.id == id);
    let body = div()
        .id(ElementId::Name(format!("sortable-{id}").into()))
        .opacity(if ghost { 0.3 } else { 1.0 })
        .child(child);
    let body = if enabled {
        let key = id.to_owned();
        body.on_drag(Dragged, move |_, offset, window, cx| {
            begin(&key, &ids, window, cx);
            let size = cx
                .global::<Sessions>()
                .sortable
                .bounds
                .get(&key)
                .map(|b| b.size);
            let size = size.unwrap_or_default();
            let hold = |length: Pixels| units::window_px(24.0, cx).min(length / 2.0);
            let shift = point(offset.x - hold(size.width), offset.y - hold(size.height));
            let draw = draw.clone();
            cx.new(|_| Image {
                draw,
                shift,
                width: size.width,
            })
        })
        .into_any_element()
    } else {
        body.into_any_element()
    };
    Slide {
        id: id.to_owned(),
        child: body,
    }
    .into_any_element()
}

/// SortableJS's `animateAll`: an entry a move displaced slides from its old top.
struct Slide {
    id: String,
    child: AnyElement,
}

impl IntoElement for Slide {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Slide {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let now = cx.background_executor().now();
        let length = motion::length(SLIDE, cx);
        let state = &mut cx.global_mut::<Sessions>().sortable;
        state.bounds.insert(self.id.clone(), bounds);
        let moves = state.moves;
        let top = bounds.origin.y;
        let track = state.tracks.entry(self.id.clone()).or_insert(Track {
            top,
            from: px(0.0),
            start: now,
            moves,
        });
        let shown = track.offset(now, length);
        if track.moves < moves && (track.top + shown - top).abs() >= px(1.0) && !length.is_zero() {
            track.from = track.top + shown - top;
            track.start = now;
        }
        track.top = top;
        track.moves = moves;
        let offset = track.offset(now, length);
        if offset != px(0.0) {
            window.request_animation_frame();
        }
        window.with_element_offset(point(px(0.0), offset), |window| {
            self.child.prepaint(window, cx)
        });
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}

#[cfg(test)]
mod tests;
