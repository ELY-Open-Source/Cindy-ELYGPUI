//! Cindy's frameless window chrome: `WindowControls.tsx`, drag bands and Linux resize edges.

pub mod actions;
mod history;

use ely_gpui_component::{
    primitives::{Icon, IconName},
    theme::{ActiveTheme as _, IconSize},
};
use gpui::{
    AnyElement, App, CursorStyle, Decorations, InteractiveElement, IntoElement, MouseButton,
    ParentElement, ResizeEdge, SharedString, StatefulInteractiveElement, Styled, Window,
    WindowControlArea, div, prelude::FluentBuilder, rgb,
};

use crate::units::css;
use crate::{i18n::Translator, theme::Appearance};

/// Cindy's 46px top row: traffic lights sit at y 16, centered on it.
pub const TOP_ROW: f32 = 46.0;
/// `WIN_CONTROLS_WIDTH`: what the header leaves for the controls.
pub const CONTROLS_WIDTH: f32 = 138.0;
/// `pl-[78px]`: room for the mac traffic lights.
pub const TRAFFIC_LIGHT_INSET: f32 = 78.0;

#[derive(Clone, Copy, Debug)]
enum Control {
    Minimize,
    Maximize,
    Close,
}

/// `WindowControls`: minimize, maximize or restore, close; Windows and Linux only. `close` runs
/// what the window's own close runs.
pub fn window_controls(
    close: impl Fn(&(), &mut Window, &mut App) + 'static,
    cx: &App,
) -> AnyElement {
    let close = std::rc::Rc::new(close);
    let appearance = cx.global::<Appearance>();
    let t = |key: &str| SharedString::from(cx.global::<Translator>().t(key, &[]));
    let icon = appearance.hsl_token("titlebar-icon");
    let hover = appearance.hsl_token("titlebar-control-hover");
    let buttons = [
        (
            Control::Minimize,
            IconName::Minus,
            WindowControlArea::Min,
            t("titleBar.minimize"),
        ),
        (
            Control::Maximize,
            IconName::Square,
            WindowControlArea::Max,
            t("titleBar.maximizeOrRestore"),
        ),
        (
            Control::Close,
            IconName::X,
            WindowControlArea::Close,
            t("titleBar.close"),
        ),
    ];
    div()
        .flex()
        .gap(css(2.0))
        .children(buttons.map(|(control, glyph, area, label)| {
            let is_close = matches!(control, Control::Close);
            let close = close.clone();
            let group = SharedString::from(format!("window-control-{control:?}"));
            div()
                .id(group.clone())
                .group(group.clone())
                .debug_selector(move || format!("window-control-{control:?}"))
                .flex()
                .items_center()
                .justify_center()
                .size(css(28.0))
                .window_control_area(area)
                .aria_label(label)
                .when(is_close, |el| el.hover(|s| s.bg(rgb(0xE81123))))
                .when(!is_close, |el| el.hover(|s| s.bg(hover)))
                .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
                .on_click(move |_, window, cx| {
                    log::info!(target: "window", "window control {control:?}");
                    match control {
                        Control::Minimize => window.minimize_window(),
                        Control::Maximize => window.zoom_window(),
                        Control::Close => close(&(), window, cx),
                    }
                })
                .child({
                    let glyph = Icon::new(glyph).size(IconSize::Sm).color(icon);
                    if is_close {
                        glyph.group_hover_color(group, gpui::white())
                    } else {
                        glyph
                    }
                })
                .into_any_element()
        }))
        .into_any_element()
}

/// Edges a frameless Linux window resizes from; none while tiled or server-drawn.
pub fn resize_edges(window: &Window, cx: &App) -> Option<AnyElement> {
    let Decorations::Client { tiling } = window.window_decorations() else {
        return None;
    };
    if tiling.is_tiled() {
        return None;
    }
    let inset = cx.theme().window_inset().to_pixels(window.rem_size());
    let grip = |edge: ResizeEdge, cursor: CursorStyle| {
        div()
            .id(SharedString::from(format!("resize-{edge:?}")))
            .absolute()
            .cursor(cursor)
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                log::info!(target: "window", "resize from {edge:?}");
                cx.stop_propagation();
                window.start_window_resize(edge);
            })
    };
    let side = inset;
    let corner = inset;
    Some(
        div()
            .absolute()
            .size_full()
            .child(
                grip(ResizeEdge::Top, CursorStyle::ResizeUpDown)
                    .top_0()
                    .left(corner)
                    .right(corner)
                    .h(side),
            )
            .child(
                grip(ResizeEdge::Bottom, CursorStyle::ResizeUpDown)
                    .bottom_0()
                    .left(corner)
                    .right(corner)
                    .h(side),
            )
            .child(
                grip(ResizeEdge::Left, CursorStyle::ResizeLeftRight)
                    .left_0()
                    .top(corner)
                    .bottom(corner)
                    .w(side),
            )
            .child(
                grip(ResizeEdge::Right, CursorStyle::ResizeLeftRight)
                    .right_0()
                    .top(corner)
                    .bottom(corner)
                    .w(side),
            )
            .child(
                grip(ResizeEdge::TopLeft, CursorStyle::ResizeUpLeftDownRight)
                    .top_0()
                    .left_0()
                    .size(corner),
            )
            .child(
                grip(ResizeEdge::TopRight, CursorStyle::ResizeUpRightDownLeft)
                    .top_0()
                    .right_0()
                    .size(corner),
            )
            .child(
                grip(ResizeEdge::BottomLeft, CursorStyle::ResizeUpRightDownLeft)
                    .bottom_0()
                    .left_0()
                    .size(corner),
            )
            .child(
                grip(ResizeEdge::BottomRight, CursorStyle::ResizeUpLeftDownRight)
                    .bottom_0()
                    .right_0()
                    .size(corner),
            )
            .into_any_element(),
    )
}
