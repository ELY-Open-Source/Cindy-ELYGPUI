//! `CardMasonry` and `DraggableCardColumns`' layout: cards in one to three round-robin columns.

use std::{cell::RefCell, time::Instant};

use gpui::{
    AnyElement, App, Bounds, IntoElement, ParentElement as _, Pixels, Styled as _, Window, canvas,
    div,
};

use super::{Sessions, sortable};
use crate::{
    motion::{self, BASE},
    units::{self, css},
};

/// `TWO_COL_MIN_WIDTH` and `THREE_COL_MIN_WIDTH`, the container's width in CSS pixels.
pub(super) fn columns_for(width: f32) -> usize {
    match width {
        w if w >= 390.0 => 3,
        w if w >= 246.0 => 2,
        _ => 1,
    }
}

/// The measured layout: Cindy's `ResizeObserver` and the `card-col-fade` remount.
#[derive(Default)]
pub struct State {
    columns: usize,
    /// When the columns last mounted; the fade runs from here.
    shown_at: Option<Instant>,
    /// The laid-out height, for the section's fold.
    pub height: f32,
    pub mounted: bool,
}

/// The `drawn` key that keeps the layout mounted.
pub const DRAWN: &str = "pinned-cards";

/// The column count the last layout measured.
pub fn columns(cx: &App) -> usize {
    cx.global::<Sessions>().masonry.columns.max(1)
}

/// Measures in prepaint; a mount or a new column count repaints with the fade from 0.
fn measure(bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
    let (width, height) = (
        units::to_css(bounds.size.width, cx),
        units::to_css(bounds.size.height, cx),
    );
    let now = cx.background_executor().now();
    let state = &mut cx.global_mut::<Sessions>().masonry;
    state.height = height;
    let columns = columns_for(width);
    if state.mounted && state.columns == columns {
        return;
    }
    log::debug!(target: "sessions", "pinned cards: {columns} columns at {width}px");
    *state = State {
        columns,
        shown_at: Some(now),
        height,
        mounted: true,
    };
    window.refresh();
}

/// `card-col-fade`: 0.2s `ease` from transparent, none under reduced motion.
fn opacity(state: &State, window: &mut Window, cx: &App) -> f32 {
    let span = motion::length(BASE, cx);
    if span.is_zero() {
        return 1.0;
    }
    let Some(shown) = state.shown_at.filter(|_| state.mounted) else {
        return 0.0;
    };
    let t = cx
        .background_executor()
        .now()
        .saturating_duration_since(shown)
        .as_secs_f32()
        / span.as_secs_f32();
    if t < 1.0 {
        window.request_animation_frame();
    }
    motion::ease(t)
}

/// The cards in `columns(cx)` buckets, as `sortable::arrange` lays them.
pub fn layout(
    buckets: Vec<Vec<AnyElement>>,
    drawn: &RefCell<Vec<String>>,
    window: &mut Window,
    cx: &App,
) -> AnyElement {
    drawn.borrow_mut().push(DRAWN.into());
    let state = &cx.global::<Sessions>().masonry;
    let opacity = opacity(state, window, cx);
    let body = match <[_; 1]>::try_from(buckets) {
        Ok([cards]) => div().flex().flex_col().gap(css(7.0)).children(cards),
        Err(buckets) => {
            div()
                .flex()
                .w_full()
                .gap(css(7.0))
                .children(buckets.into_iter().enumerate().map(|(c, bucket)| {
                    let column = div().flex().flex_1().min_w_0().flex_col().gap(css(7.0));
                    sortable::column(super::pinned::LIST, c, column.children(bucket))
                }))
        }
    };
    div()
        .relative()
        .w_full()
        .opacity(opacity)
        .child(body)
        .child(canvas(measure, |_, _, _, _| {}).absolute().size_full())
        .into_any_element()
}
