//! `CCAgentSessionView`'s column: the message area and the composer dock, at the hook's widths.

use gpui::{
    AnyElement, App, AvailableSpace, InteractiveElement as _, IntoElement as _, ParentElement as _,
    Styled as _, Window, canvas, div, linear_color_stop, linear_gradient, size,
};

use super::width::{Widths, widths};
use crate::{
    theme::Appearance,
    units::{self, css},
};

/// The view's container, measured as the hook's `ResizeObserver` measures it.
pub fn element() -> AnyElement {
    canvas(
        |bounds, window, cx| {
            let widths = widths(units::to_css(bounds.size.width, cx))?;
            let mut section = section(widths, window, cx);
            let space = size(
                AvailableSpace::Definite(bounds.size.width),
                AvailableSpace::Definite(bounds.size.height),
            );
            section.prepaint_as_root(bounds.origin, space, window, cx);
            Some(section)
        },
        |_, section, window, cx| {
            if let Some(mut section) = section {
                section.paint(window, cx);
            }
        },
    )
    .flex_1()
    .min_h_0()
    .w_full()
    .into_any_element()
}

/// `section`: the message viewport, and the dock over its foot.
fn section(widths: Widths, window: &mut Window, cx: &mut App) -> AnyElement {
    let area = cx.global::<Appearance>().hsl_token("content-area");
    let area = crate::wallpaper::clear(area, cx);
    let messages = super::stream::element(widths, window, cx);
    let viewport = div().relative().min_h_0().flex_1().child(messages);
    let pill = super::handoff_pill::element(widths.message, window, cx);
    // `bg-gradient-to-t from-content-area to-transparent`, `h-8`.
    let fade = div()
        .debug_selector(|| "session-composer-fade".into())
        .h(css(32.0))
        .w_full()
        .bg(linear_gradient(
            0.0,
            linear_color_stop(area, 0.0),
            linear_color_stop(area.opacity(0.0), 1.0),
        ));
    // The solid zone, `pb-5`; 9.15 fills its row at `inputWidth`.
    let zone = div()
        .debug_selector(|| "session-composer-zone".into())
        .flex()
        .flex_col()
        .items_center()
        .w_full()
        .pb(css(20.0))
        .bg(area)
        .child(
            div()
                .debug_selector(|| "session-composer-slot".into())
                .mx_auto()
                .w(css(widths.input)),
        );
    let dock = div()
        .absolute()
        .bottom_0()
        .left(css(widths.input_pad))
        .right(css(widths.input_pad))
        .flex()
        .flex_col()
        .items_center()
        .child(fade)
        .child(zone);
    div()
        .debug_selector(|| "session-view".into())
        .relative()
        .flex()
        .flex_col()
        .size_full()
        .bg(area)
        .children(pill)
        .child(viewport)
        .child(dock)
        .into_any_element()
}

#[cfg(test)]
mod tests;
