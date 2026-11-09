//! `SidebarTitleMarquee`: an overflowing title scrolls once across while its row is hovered.

use std::time::{Duration, Instant};

use gpui::{
    App, Bounds, Div, InteractiveElement as _, ParentElement as _, Pixels, Stateful,
    StatefulInteractiveElement as _, Styled as _, Window, canvas, div, prelude::FluentBuilder as _,
};

use super::Sessions;
use crate::{
    motion,
    tip::tip,
    units::{self, css},
};

/// `--motion-sidebar-title-marquee-per-viewport`.
const PER_VIEWPORT: Duration = Duration::from_millis(2400);

/// The hovered row's title: its measures and the scroll they started.
#[derive(Default)]
pub struct State {
    row: Option<String>,
    /// What the scroll was started for: title, widths and length.
    key: (String, (f32, f32), Duration),
    track: f32,
    run: Option<Run>,
    /// Painted since the last list render; an unmounted row lets go.
    row_seen: bool,
    title_seen: bool,
    /// The title was painted before the last list render; a remounted one starts again.
    title_was_seen: bool,
}

struct Run {
    started: Instant,
    shift: f32,
    length: Duration,
}

impl Run {
    /// `sidebar-title-marquee`: still to 12%, `--motion-ease-move` to 88%, then held.
    fn at(&self, now: Instant) -> (f32, bool) {
        let t =
            now.saturating_duration_since(self.started).as_secs_f32() / self.length.as_secs_f32();
        let x = match t {
            t if t <= 0.12 => 0.0,
            t if t >= 0.88 => self.shift,
            t => self.shift * motion::ease_move((t - 0.12) / 0.76),
        };
        (x, t < 1.0)
    }
}

/// The row's `mouseenter` and `mouseleave`.
pub(super) fn hover(id: &str, hovered: bool, window: &mut Window, cx: &mut App) {
    let state = &mut cx.global_mut::<Sessions>().marquee;
    if hovered {
        *state = State {
            row: Some(id.to_owned()),
            row_seen: true,
            ..State::default()
        };
    } else if state.row.as_deref() == Some(id) {
        *state = State::default();
    } else {
        return;
    }
    window.refresh();
}

/// Lets go of a hovered row that was not painted since the last list render.
pub(super) fn sweep(cx: &mut App) {
    let state = &mut cx.global_mut::<Sessions>().marquee;
    if state.row.is_some() && !state.row_seen {
        log::debug!(target: "sessions", "title marquee: {:?} unmounted", state.row);
        *state = State::default();
    }
    state.title_was_seen = state.title_seen;
    state.row_seen = false;
    state.title_seen = false;
}

/// Marks the row painted; it goes inside the row that calls `hover`.
pub(super) fn mark(id: &str) -> impl gpui::IntoElement {
    let id = id.to_owned();
    canvas(
        move |_, _, cx| {
            let state = &mut cx.global_mut::<Sessions>().marquee;
            if state.row.as_deref() == Some(id.as_str()) {
                state.row_seen = true;
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
}

/// Measures the hovered title; a remount, a new width, title or motion setting starts again.
fn measure(id: &str, title: &str, container: f32, window: &mut Window, cx: &mut App) {
    let now = cx.background_executor().now();
    let length = motion::length(PER_VIEWPORT, cx);
    let state = &mut cx.global_mut::<Sessions>().marquee;
    if state.row.as_deref() != Some(id) {
        return;
    }
    state.title_seen = true;
    let key = (title.to_owned(), (container, state.track), length);
    if state.title_was_seen && state.key == key {
        if state.run.as_ref().is_some_and(|run| run.at(now).1) {
            window.request_animation_frame();
        }
        return;
    }
    let track = state.track;
    let viewports = (track / container.max(1.0)).ceil().max(1.0);
    state.key = key;
    state.title_was_seen = true;
    state.run = (track > container + 1.0 && !length.is_zero()).then(|| Run {
        started: now,
        shift: container - track,
        length: length * viewports as u32,
    });
    match &state.run {
        Some(run) => {
            log::debug!(target: "sessions", "title marquee: {id} shifts {}px over {:?}", run.shift, run.length)
        }
        None => {
            log::debug!(target: "sessions", "title marquee: {id} still ({track}px in {container}px, {length:?})")
        }
    }
    window.refresh();
}

fn track_width(id: &str, bounds: Bounds<Pixels>, cx: &mut App) {
    let width = units::to_css(bounds.size.width, cx);
    let state = &mut cx.global_mut::<Sessions>().marquee;
    if state.row.as_deref() == Some(id) {
        state.track = width;
    }
}

/// The title: an ellipsis line, and a track drawn over it while it scrolls. Text styles go
/// on the returned container.
pub(super) fn title(id: &str, text: String, cx: &App) -> Stateful<Div> {
    let state = &cx.global::<Sessions>().marquee;
    let still = motion::length(PER_VIEWPORT, cx).is_zero();
    let x = state
        .run
        .as_ref()
        .filter(|_| state.row.as_deref() == Some(id) && !still)
        .map(|run| run.at(cx.background_executor().now()).0);
    let (track_id, container_id) = (id.to_owned(), id.to_owned());
    let label = text.clone();
    let selector = format!("task-title-{id}");
    let track_selector = format!("task-title-track-{id}");
    div()
        .id(gpui::SharedString::from(selector.clone()))
        .debug_selector(move || selector)
        .relative()
        .min_w_0()
        .max_w_full()
        .flex_shrink(1.0)
        .overflow_hidden()
        .tooltip(tip(text.clone()))
        .child(
            div()
                .min_w_0()
                .truncate()
                .when(x.is_some(), |el| el.opacity(0.0))
                .child(text.clone()),
        )
        .child(
            div()
                .debug_selector(move || track_selector)
                .absolute()
                .top_0()
                .left(css(x.unwrap_or(0.0)))
                .whitespace_nowrap()
                .when(x.is_none(), |el| el.invisible())
                .child(text)
                .child(
                    canvas(
                        move |bounds, _, cx| track_width(&track_id, bounds, cx),
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .size_full(),
                ),
        )
        .child(
            canvas(
                move |bounds, window, cx| {
                    let width = units::to_css(bounds.size.width, cx);
                    measure(&container_id, &label, width, window, cx);
                },
                |_, _, _, _| {},
            )
            .absolute()
            .size_full(),
        )
}

#[cfg(test)]
mod tests;
