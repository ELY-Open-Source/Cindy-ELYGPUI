//! An in-document anchor: the ids the stream painted, and `scrollIntoView` to one.

use std::collections::HashMap;

use gpui::{App, Bounds, Pixels, ScrollHandle, Window};

use super::SessionView;
use crate::{
    motion::ReducedMotion,
    smooth_scroll::{self, Run},
};

/// The stream's scroller, where each id was last painted, and the scroll on its way.
#[derive(Default)]
pub struct Jump {
    pub handle: ScrollHandle,
    ids: HashMap<String, Bounds<Pixels>>,
    run: Option<Run>,
}

fn held(cx: &mut App) -> Option<&mut Jump> {
    let shown = cx.global_mut::<SessionView>().shown.as_mut();
    shown?.content.as_mut().map(|content| &mut content.jump)
}

/// Before the rows draw: the ids are painted anew, and a scroll on its way takes its step.
pub fn begin(window: &mut Window, cx: &mut App) -> Option<ScrollHandle> {
    let now = cx.background_executor().now();
    let jump = held(cx)?;
    jump.ids.clear();
    if smooth_scroll::step(&jump.handle, &mut jump.run, now) {
        window.request_animation_frame();
    }
    Some(jump.handle.clone())
}

/// An element with `id` was painted; the first in the document keeps the id.
pub fn painted(id: &str, bounds: Bounds<Pixels>, cx: &mut App) {
    if let Some(jump) = held(cx) {
        jump.ids.entry(id.to_owned()).or_insert(bounds);
    }
}

/// `scrollIntoView({ behavior: 'smooth', block: 'start' })`; false when no element has the id.
pub fn to(id: &str, window: &mut Window, cx: &mut App) -> bool {
    let now = cx.background_executor().now();
    let held = (window.scale_factor(), cx.global::<ReducedMotion>().0);
    let Some(jump) = self::held(cx) else {
        return false;
    };
    let Some(found) = jump.ids.get(id) else {
        return false;
    };
    let delta = f32::from(found.top() - jump.handle.bounds().top());
    match smooth_scroll::start(&jump.handle, &mut jump.run, delta, held, now) {
        Some((by, length)) => {
            log::info!(target: "sessionView", "anchor {id}: {by:.1}px over {}ms", length.as_millis());
            window.refresh();
        }
        None => log::debug!(target: "sessionView", "anchor {id} already at the top"),
    }
    true
}
