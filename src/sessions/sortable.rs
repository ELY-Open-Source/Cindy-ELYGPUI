//! `SortableList` with native drag, as `PinnedSection` mounts it, and `DraggableCardColumns`'
//! columns: the entry under the pointer makes way, a drop on the list saves, anything else puts
//! the order back.

mod order;

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
    /// Each card column's box, by index.
    columns: HashMap<usize, Bounds<Pixels>>,
    tracks: HashMap<String, Track>,
    /// Bumped by each move; an entry slides once per move.
    moves: u64,
}

struct Active {
    id: String,
    /// The shown columns as the drag has them; text and list modes have one.
    columns: Vec<Vec<String>>,
    start: Vec<String>,
    _escape: Subscription,
}

struct Track {
    origin: Point<Pixels>,
    from: Point<Pixels>,
    start: Instant,
    moves: u64,
}

impl Track {
    /// Where the slide has the entry now, from its laid-out origin.
    fn offset(&self, now: Instant, length: std::time::Duration) -> Point<Pixels> {
        let t = match length.is_zero() {
            true => 1.0,
            false => now.saturating_duration_since(self.start).as_secs_f32() / length.as_secs_f32(),
        };
        match t < 1.0 {
            true => self.from * (1.0 - motion::ease(t)),
            false => Point::default(),
        }
    }
}

/// A pinned entry on the move; `State` knows which.
pub struct Dragged;

/// What a drop does with the new order of the shown entries.
pub type OnReorder = Rc<dyn Fn(&[String], &mut Window, &mut App)>;
/// Draws an entry again for the drag image.
pub type Draw = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

/// The entries in `n` round-robin columns, as the drag has them while one is dragged.
pub fn arrange<T>(entries: Vec<T>, n: usize, id: impl Fn(&T) -> String, cx: &App) -> Vec<Vec<T>> {
    let active = cx.global::<Sessions>().sortable.active.as_ref();
    let Some(active) = active.filter(|a| a.columns.len() == n.max(1)) else {
        return order::buckets(entries, n);
    };
    let mut keyed: Vec<(String, T)> = entries.into_iter().map(|e| (id(&e), e)).collect();
    let fits = keyed.len() == active.start.len()
        && keyed
            .iter()
            .all(|(k, _)| order::locate(&active.columns, k).is_some());
    if !fits {
        return order::buckets(keyed.into_iter().map(|(_, e)| e).collect(), n);
    }
    let mut out: Vec<Vec<T>> = active.columns.iter().map(|_| Vec::new()).collect();
    keyed.sort_by_key(|(k, _)| order::locate(&active.columns, k));
    for (k, e) in keyed {
        if let Some((c, _)) = order::locate(&active.columns, &k) {
            out[c].push(e);
        }
    }
    out
}

/// Forgets entries not painted since the last list render, and drops a drag that ended off
/// the list or lost its window; nothing is saved.
pub fn sweep(window: &mut Window, cx: &mut App) {
    let dragging = cx.has_active_drag();
    let state = &mut cx.global_mut::<Sessions>().sortable;
    let bounds = std::mem::take(&mut state.bounds);
    state.columns.clear();
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

fn begin(id: &str, ids: &[String], n: usize, window: &mut Window, cx: &mut App) {
    let handle = window.window_handle().window_id();
    let escape = cx.intercept_keystrokes(move |event, window, cx| {
        if window.window_handle().window_id() == handle && event.keystroke.key == "escape" {
            cancel(window, cx);
        }
    });
    log::info!(target: "sessions", "pinned drag of {id} started among {} in {n} columns", ids.len());
    cx.global_mut::<Sessions>().sortable.active = Some(Active {
        id: id.to_owned(),
        columns: order::buckets(ids.to_vec(), n),
        start: ids.to_vec(),
        _escape: escape,
    });
}

/// The entry under the pointer, else the card column whose empty tail it is in.
fn target(state: &State, columns: &[Vec<String>], at: Point<Pixels>) -> Option<order::Target> {
    let bounds = |id: &String| state.bounds.get(id);
    if let Some(id) = columns
        .iter()
        .flatten()
        .find(|id| bounds(id).is_some_and(|b| b.contains(&at)))
    {
        return Some(order::Target::Entry(id.clone()));
    }
    columns.iter().enumerate().find_map(|(c, col)| {
        state.columns.get(&c).filter(|b| b.contains(&at))?;
        let below = col
            .last()
            .and_then(bounds)
            .is_none_or(|b| at.y > b.bottom());
        below.then_some(order::Target::Tail(c))
    })
}

/// SortableJS's `swapThreshold: 1`: entering an entry takes its place.
fn moved(position: Point<Pixels>, window: &mut Window, cx: &mut App) {
    cx.set_active_drag_cursor_style(CursorStyle::ClosedHand, window);
    let state = &mut cx.global_mut::<Sessions>().sortable;
    let Some(mut active) = state.active.take() else {
        return;
    };
    let moved = target(state, &active.columns, position)
        .is_some_and(|t| order::step(&mut active.columns, &active.id, &t));
    state.active = Some(active);
    if moved {
        state.moves += 1;
        window.refresh();
    }
}

fn dropped(on_reorder: &OnReorder, window: &mut Window, cx: &mut App) {
    let Some(active) = cx.global_mut::<Sessions>().sortable.active.take() else {
        return;
    };
    let next = order::dropped(&active.start, &active.columns, &active.id);
    if next == active.start {
        log::info!(target: "sessions", "pinned drag of {} dropped in place", active.id);
        return;
    }
    log::info!(target: "sessions", "pinned drag of {} dropped", active.id);
    on_reorder(&next, window, cx);
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

/// A card column; a drag into its empty tail appends there.
pub fn column(index: usize, body: gpui::Div) -> AnyElement {
    let record = move |bounds: Bounds<Pixels>, _: &mut Window, cx: &mut App| {
        let state = &mut cx.global_mut::<Sessions>().sortable;
        state.columns.insert(index, bounds);
    };
    body.relative()
        .child(gpui::canvas(record, |_, _, _, _| {}).absolute().size_full())
        .into_any_element()
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

/// One entry; `ids` are the shown entries in order, laid in `columns`. A disabled entry does
/// not drag.
pub fn item(
    id: &str,
    ids: Rc<Vec<String>>,
    columns: usize,
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
            begin(&key, &ids, columns, window, cx);
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

/// SortableJS's `animateAll`: an entry a move displaced slides from its old place.
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
        let origin = bounds.origin;
        let track = state.tracks.entry(self.id.clone()).or_insert(Track {
            origin,
            from: Point::default(),
            start: now,
            moves,
        });
        let from = track.origin + track.offset(now, length) - origin;
        let far = from.x.abs() >= px(1.0) || from.y.abs() >= px(1.0);
        if track.moves < moves && far && !length.is_zero() {
            track.from = from;
            track.start = now;
        }
        track.origin = origin;
        track.moves = moves;
        let offset = track.offset(now, length);
        if offset != Point::default() {
            window.request_animation_frame();
        }
        window.with_element_offset(offset, |window| self.child.prepaint(window, cx));
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
