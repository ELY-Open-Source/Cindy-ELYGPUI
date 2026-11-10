//! `scrollIntoNearestView` for the active task: `block: 'nearest'`, smooth unless motion is
//! reduced, along Chromium's programmatic scroll curve.

use std::time::Duration;

use gpui::{App, Bounds, Global, Pixels, ScrollHandle, Window};

use crate::motion::{self, Tween};

/// The list's scroller, the task waiting to be revealed and the scroll on its way.
#[derive(Default)]
pub struct Reveal {
    pub handle: ScrollHandle,
    wanted: Option<String>,
    /// A project asked for, and whether a frame has drawn since: it waits one frame, as a RAF.
    project: Option<(String, bool)>,
    run: Option<Run>,
}

impl Global for Reveal {}

struct Run {
    tween: Tween,
    set: Pixels,
}

/// A task that became active, or mounted active, as the `isActive` effect sees it.
pub fn activated(id: &str, cx: &mut App) {
    log::debug!(target: "sessions", "task {id} active: reveal asked");
    cx.global_mut::<Reveal>().wanted = Some(id.to_owned());
}

/// The CSSOM `nearest` block delta for a box against the scrollport.
fn nearest(top: f32, bottom: f32, port_top: f32, port_bottom: f32) -> f32 {
    let (height, port) = (bottom - top, port_bottom - port_top);
    let (above, below) = (top < port_top, bottom > port_bottom);
    if above && below {
        0.0
    } else if (above && height <= port) || (below && height >= port) {
        top - port_top
    } else if (above && height > port) || (below && height < port) {
        bottom - port_bottom
    } else {
        0.0
    }
}

/// `ScrollOffsetAnimationCurve::EaseInOutSegmentDuration`, delta based, in device pixels.
fn duration(delta: f32) -> Duration {
    Duration::from_secs_f32(delta.abs().sqrt().min(1.5 * 60.0) / 60.0)
}

/// The focused project's node, as the effect's `scrollIntoView` in the next frame finds it.
pub fn project(key: &str, cx: &mut App) {
    log::debug!(target: "sessions", "project {key} focused: reveal asked");
    cx.global_mut::<Reveal>().project = Some((key.to_owned(), false));
}

/// The row's prepaint: a wanted row starts its scroll from where the list sits now.
pub fn painted(id: &str, row: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
    if cx.global::<Reveal>().wanted.as_deref() != Some(id) {
        return;
    }
    cx.global_mut::<Reveal>().wanted = None;
    scroll(id, row, window, cx);
}

/// The project node's prepaint.
pub fn project_painted(key: &str, node: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
    let wanted = cx.global::<Reveal>().project.as_ref();
    if wanted.is_none_or(|(wanted, _)| wanted != key) {
        return;
    }
    cx.global_mut::<Reveal>().project = None;
    scroll(key, node, window, cx);
}

/// `scrollIntoView({ block: 'nearest' })` for the box drawn at `row`.
fn scroll(id: &str, row: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
    let now = cx.background_executor().now();
    let reveal = cx.global_mut::<Reveal>();
    let port = reveal.handle.bounds();
    let delta = nearest(
        row.top().into(),
        row.bottom().into(),
        port.top().into(),
        port.bottom().into(),
    );
    let from = reveal.handle.offset().y;
    let max = reveal.handle.max_offset().y;
    let to = (from - gpui::px(delta)).clamp(-max, gpui::px(0.0));
    if to == from {
        // `CancelProgrammaticScrollAnimation` on a no-op offset.
        reveal.run = None;
        return log::debug!(target: "sessions", "{id} already in view");
    }
    let length = duration(f32::from(to - from) * window.scale_factor());
    let length = motion::length(length, cx);
    let mut tween = Tween::still(from.into(), now);
    tween.toward_along(to.into(), now, length, motion::programmatic_scroll);
    log::info!(target: "sessions", "{id} revealed: {:.1}px over {}ms", f32::from(to - from), length.as_millis());
    cx.global_mut::<Reveal>().run = Some(Run { tween, set: from });
    window.request_animation_frame();
}

/// Before the list lays out: the next step of the scroll; a user scroll takes over.
pub fn step(window: &mut Window, cx: &mut App) {
    let now = cx.background_executor().now();
    let reveal = cx.global_mut::<Reveal>();
    // A project whose node no frame drew is let go, as the one `querySelector` misses it.
    reveal.project = match reveal.project.take() {
        Some((key, false)) => Some((key, true)),
        _ => None,
    };
    let Some(run) = reveal.run.as_mut() else {
        return;
    };
    if reveal.handle.offset().y != run.set {
        log::debug!(target: "sessions", "reveal stopped by a scroll");
        reveal.run = None;
        return;
    }
    let (at, going) = run.tween.at(now);
    run.set = gpui::px(at);
    reveal
        .handle
        .set_offset(gpui::point(reveal.handle.offset().x, run.set));
    if going {
        window.request_animation_frame();
    } else {
        reveal.run = None;
    }
}

#[cfg(test)]
mod tests;
