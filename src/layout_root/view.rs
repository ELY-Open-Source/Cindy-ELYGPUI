//! `LayoutRoot`'s view: the content tree as panes, splits and dividers.

use std::{collections::HashMap, time::Duration};

use gpui::{
    AnyElement, App, BorrowAppContext, Context, CursorStyle, DispatchPhase, Entity,
    InteractiveElement, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ParentElement, Pixels, Point, Render, StatefulInteractiveElement, Styled, Task, Window, canvas,
    div, prelude::FluentBuilder, px,
};

use super::divider::{ColumnDrag, RootDrag, equalize_column, equalize_root};
use super::geometry::{Entry, Ledger, Presence, Seam, normalize_sub_min_fractions, root_widths};
use crate::layout_tree::{Direction, Layout, LayoutStore, Node};

mod rows;

const SELF_HEAL_SETTLE: Duration = Duration::from_millis(120);
const CHAT: &str = "chat-main";

/// The kinds with a panel today.
fn mounted(kind: &str) -> bool {
    matches!(kind, CHAT | "right-tabs")
}

fn never(_: &str) -> bool {
    false
}

const PRESENCE: Presence<'static> = Presence {
    mounted: &mounted,
    parked: &never,
};

fn f(p: Pixels) -> f64 {
    f64::from(f32::from(p))
}

/// A divider gesture under way; `start` is the pointer at the press.
enum Drag {
    Root {
        drag: Box<RootDrag>,
        start: f64,
        delta: f64,
    },
    Column {
        drag: ColumnDrag,
        start: f64,
        delta: f64,
    },
}

/// A seam between two visible children of one split, by tree index.
#[derive(Clone)]
struct SeamAt {
    split_id: String,
    first: usize,
    second: usize,
    scale: f64,
    direction: Direction,
}

impl SeamAt {
    fn seam(&self) -> Seam<'_> {
        Seam {
            split_id: &self.split_id,
            left: self.first,
            right: self.second,
            scale: self.scale,
        }
    }

    fn key(&self) -> String {
        format!("{}:{}", self.split_id, self.second)
    }
}

pub struct LayoutRoot {
    sidebar_width: f64,
    avail: f64,
    height: f64,
    live: Option<HashMap<String, f64>>,
    drag: Option<Drag>,
    hovered: Option<String>,
    heal_key: Option<(f64, Layout)>,
    heal: Option<Task<()>>,
}

impl LayoutRoot {
    /// `sidebar_width` is the column left of the content.
    pub fn new(sidebar_width: f64, cx: &mut Context<Self>) -> Self {
        cx.observe_global::<LayoutStore>(|_, cx| cx.notify())
            .detach();
        LayoutRoot {
            sidebar_width,
            avail: 0.0,
            height: 0.0,
            live: None,
            drag: None,
            hovered: None,
            heal_key: None,
            heal: None,
        }
    }

    fn widths(&self, layout: &Layout, live: bool) -> HashMap<String, f64> {
        root_widths(
            layout,
            self.avail,
            self.live.as_ref().filter(|_| live),
            &PRESENCE,
            &never,
        )
    }

    /// `preserveChatWidth`: a root pane keeps a width the user chose.
    fn preserve_chat_width(layout: &Layout) -> bool {
        Ledger::root(layout, &PRESENCE).is_some_and(|l| {
            l.entries()
                .any(|e| matches!(e.node, Node::Pane(p) if p.preferred_width.is_some()))
        })
    }

    fn press(&mut self, at: &SeamAt, event: &MouseDownEvent, cx: &mut Context<Self>) {
        if self.drag.is_some() {
            return;
        }
        cx.stop_propagation();
        let layout = cx.global::<LayoutStore>().layout().clone();
        let widths = self.widths(&layout, false);
        let measured = |n: &Node| widths.get(n.id()).copied();
        let double = event.click_count >= 2;
        let column = at.direction == Direction::Column;
        let entries = layout
            .content
            .split(&at.split_id)
            .map(|s| Ledger::new(&s.children, &PRESENCE));
        let edited = match (double, column) {
            (true, false) => {
                let preserve = Self::preserve_chat_width(&layout);
                equalize_root(
                    &layout,
                    &PRESENCE,
                    at.seam(),
                    self.avail,
                    preserve,
                    &measured,
                )
            }
            (true, true) => entries
                .as_ref()
                .and_then(|l| pair(l, at))
                .and_then(|(top, bottom)| {
                    equalize_column(&layout, &at.split_id, &top, &bottom, at.scale)
                }),
            (false, false) => {
                let preserve = Self::preserve_chat_width(&layout);
                let drag = RootDrag::start(
                    &layout,
                    &PRESENCE,
                    at.seam(),
                    self.avail,
                    preserve,
                    &measured,
                );
                self.drag = drag.map(|drag| Drag::Root {
                    drag: Box::new(drag),
                    start: f(event.position.x),
                    delta: 0.0,
                });
                None
            }
            (false, true) => {
                let started =
                    entries
                        .as_ref()
                        .and_then(|l| pair(l, at))
                        .and_then(|(top, bottom)| {
                            ColumnDrag::start(&at.split_id, &top, &bottom, at.scale, self.height)
                        });
                self.drag = started.map(|drag| Drag::Column {
                    drag,
                    start: f(event.position.y),
                    delta: 0.0,
                });
                None
            }
        };
        if !double && self.drag.is_none() {
            log::warn!(target: "layout", "divider {} has no drag room", at.key());
        }
        if let Some(next) = edited {
            save(next, cx);
        }
        cx.notify();
    }

    fn moved(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.live = match &mut self.drag {
            Some(Drag::Root { drag, start, delta }) => {
                *delta = drag.delta(f(position.x) - *start);
                Some(drag.live(*delta))
            }
            Some(Drag::Column { drag, start, delta }) => {
                *delta = drag.delta(f(position.y) - *start);
                Some(drag.live(*delta))
            }
            None => return,
        };
        cx.notify();
    }

    /// Release: one write of the whole move, then self-heal may run again.
    fn released(&mut self, cx: &mut Context<Self>) {
        let Some(drag) = self.drag.take() else { return };
        let layout = cx.global::<LayoutStore>().layout().clone();
        let next = match drag {
            Drag::Root { drag, delta, .. } => drag.commit(&layout, delta, &PRESENCE, self.avail),
            Drag::Column { drag, delta, .. } => (delta != 0.0)
                .then(|| drag.commit(&layout, delta))
                .flatten(),
        };
        self.live = None;
        self.heal_key = None;
        if let Some(next) = next {
            save(next, cx);
        }
        cx.notify();
    }

    /// Runs the heal once the width and tree hold still.
    fn schedule_heal(&mut self, layout: &Layout, cx: &mut Context<Self>) {
        let key = (self.avail, layout.clone());
        if self.drag.is_some() || self.heal_key.as_ref() == Some(&key) {
            return;
        }
        self.heal_key = Some(key);
        self.heal = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SELF_HEAL_SETTLE).await;
            if let Err(error) = this.update(cx, |this, cx| this.self_heal(cx)) {
                log::debug!(target: "layout", "layout root gone before self-heal: {error}");
            }
        }));
    }

    fn self_heal(&mut self, cx: &mut Context<Self>) {
        if self.drag.is_some() || self.avail <= 0.0 {
            return;
        }
        let layout = cx.global::<LayoutStore>().layout();
        if let Some(fixed) =
            normalize_sub_min_fractions(layout, self.avail, &|k| PRESENCE.visible(k))
        {
            log::info!(target: "layout", "self-heal lifted panes below their floor at {}px", self.avail);
            save(fixed, cx);
        }
    }

    fn divider(&self, at: SeamAt, cx: &mut Context<Self>) -> AnyElement {
        let key = at.key();
        let row = at.direction == Direction::Row;
        let hovered = self.hovered.as_deref() == Some(key.as_str());
        let line = rows::line(row, hovered, cx);
        let grab = div()
            .id(gpui::ElementId::Name(format!("divider-{key}").into()))
            .absolute();
        let grab = if row {
            grab.top_0()
                .bottom_0()
                .left(px(-3.0))
                .w(px(7.0))
                .cursor(CursorStyle::ResizeLeftRight)
        } else {
            grab.left_0()
                .right_0()
                .top(px(-3.0))
                .h(px(7.0))
                .cursor(CursorStyle::ResizeUpDown)
        };
        let hover_key = key.clone();
        let grab = grab
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                this.hovered = hovered.then(|| hover_key.clone());
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event, _, cx| this.press(&at, event, cx)),
            );
        line.child(grab).into_any_element()
    }
}

fn pair<'a>(ledger: &Ledger<'a>, at: &SeamAt) -> Option<(Entry<'a>, Entry<'a>)> {
    Some((*ledger.find(at.first)?, *ledger.find(at.second)?))
}

/// `layout.set`: a refused tree is a bug in its caller; a failed write is logged by the store.
fn save(layout: Layout, cx: &mut App) {
    if let Err(reason) = cx.update_global::<LayoutStore, _>(|store, _| store.set(layout)) {
        log::warn!(target: "layout", "layout rejected: {reason}");
    }
}

impl Render for LayoutRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size();
        self.avail = f(viewport.width) - self.sidebar_width;
        self.height = f(viewport.height);
        let layout = cx.global::<LayoutStore>().layout().clone();
        self.schedule_heal(&layout, cx);
        let dragging = self.drag.as_ref().map(|d| match d {
            Drag::Root { .. } => CursorStyle::ResizeLeftRight,
            Drag::Column { .. } => CursorStyle::ResizeUpDown,
        });
        let this: Entity<Self> = cx.entity();
        div()
            .flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_hidden()
            .children(self.root_row(&layout, cx))
            .when_some(dragging, |el, cursor| {
                el.child(
                    canvas(
                        |_, _, _| {},
                        move |_, _, window, _| {
                            window.set_window_cursor_style(cursor);
                            let mover = this.clone();
                            window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                                if phase == DispatchPhase::Bubble {
                                    mover.update(cx, |root, cx| root.moved(event.position, cx));
                                }
                            });
                            window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                                if phase == DispatchPhase::Bubble
                                    && event.button == MouseButton::Left
                                {
                                    this.update(cx, |root, cx| root.released(cx));
                                }
                            });
                        },
                    )
                    .absolute()
                    .size_0(),
                )
            })
    }
}
