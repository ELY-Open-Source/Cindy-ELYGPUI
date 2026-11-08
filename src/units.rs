//! Cindy's CSS pixels. Chromium's zoom scales them all, so they are rems at 16 per zoom unit.

use gpui::{App, Pixels, Rems, rems};

/// The window zoom from `appearance-settings.json`.
#[derive(Clone, Copy, Debug)]
pub struct Zoom(pub f32);

impl gpui::Global for Zoom {}

/// One length in Cindy's CSS pixels.
pub fn css(value: f32) -> Rems {
    rems(value / 16.0)
}

/// The rem size that makes `css` follow the zoom.
pub fn rem_size(cx: &App) -> Pixels {
    gpui::px(16.0 * cx.global::<Zoom>().0)
}

/// Window pixels back in Cindy's CSS pixels.
pub fn to_css(value: Pixels, cx: &App) -> f32 {
    f32::from(value) / cx.global::<Zoom>().0
}

/// Cindy's CSS pixels in window pixels, for comparing with painted bounds and the pointer.
pub fn window_px(value: f32, cx: &App) -> Pixels {
    gpui::px(value * cx.global::<Zoom>().0)
}
