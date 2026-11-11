//! A programmatic smooth scroll on a `ScrollHandle`, along Chromium's curve.

use std::time::{Duration, Instant};

use gpui::{Pixels, ScrollHandle, point, px};

use crate::motion::{self, Tween};

/// A scroll on its way, and the offset it last set.
pub struct Run {
    tween: Tween,
    set: Pixels,
}

/// `ScrollOffsetAnimationCurve::EaseInOutSegmentDuration`, delta based, in device pixels.
pub fn duration(delta: f32) -> Duration {
    Duration::from_secs_f32(delta.abs().sqrt().min(1.5 * 60.0) / 60.0)
}

/// Starts the scroll `delta` window pixels down the content: how far it goes and for how long.
pub fn start(
    handle: &ScrollHandle,
    run: &mut Option<Run>,
    delta: f32,
    (scale, reduced): (f32, bool),
    now: Instant,
) -> Option<(f32, Duration)> {
    let from = handle.offset().y;
    let to = (from - px(delta)).clamp(-handle.max_offset().y, px(0.0));
    if to == from {
        // `CancelProgrammaticScrollAnimation` on a no-op offset.
        *run = None;
        return None;
    }
    let length = match reduced {
        true => Duration::ZERO,
        false => duration(f32::from(to - from) * scale),
    };
    let mut tween = Tween::still(from.into(), now);
    tween.toward_along(to.into(), now, length, motion::programmatic_scroll);
    *run = Some(Run { tween, set: from });
    Some(((to - from).into(), length))
}

/// The next step, before the scroller lays out; whether another frame is owed.
pub fn step(handle: &ScrollHandle, run: &mut Option<Run>, now: Instant) -> bool {
    let Some(going) = run.as_mut() else {
        return false;
    };
    if handle.offset().y != going.set {
        log::debug!(target: "scroll", "a programmatic scroll stopped by a scroll");
        *run = None;
        return false;
    }
    let (at, more) = going.tween.at(now);
    going.set = px(at);
    handle.set_offset(point(handle.offset().x, going.set));
    if !more {
        *run = None;
    }
    more
}
