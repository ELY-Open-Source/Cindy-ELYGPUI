//! `CCAgentSidebarUpper`'s two views, crossfading, and `CollapsedView`, the rail's column.

use gpui::{
    AnyElement, App, ElementId, FocusHandle, HoverListenerMode, InteractiveElement as _,
    IntoElement, ParentElement as _, Role, StatefulInteractiveElement as _, Styled as _, Svg,
    Window, div, prelude::FluentBuilder as _,
};

pub mod pinned;
pub mod search;
pub mod sections;

use crate::{
    i18n::Translator, lucide, motion::ReducedMotion, router, theme::Appearance, tip::tip,
    units::css,
};

/// Both views stay drawn; `rail` is the rail's opacity, and `on` picks the one taking the pointer.
pub fn upper(rail: f32, on: bool, window: &mut Window, cx: &mut App) -> AnyElement {
    let layer = |id: &'static str, opacity: f32| {
        div()
            .id(id)
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .min_w_0()
            .overflow_hidden()
            .opacity(opacity)
    };
    let expanded = layer("sidebar-expanded", 1.0 - rail).children(super::list(window, cx));
    let collapsed = layer("sidebar-collapsed", rail).child(view(window, cx));
    // `pointer-events-none` on the hidden one: the shown one paints last and occludes it.
    let (under, over) = match on {
        true => (expanded, collapsed),
        false => (collapsed, expanded),
    };
    div()
        .relative()
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .overflow_hidden()
        .child(under)
        .child(over.occlude())
        .into_any_element()
}

/// `CollapsedView`'s scroller, no scrollbar; Teammates come with 14.0.
fn view(window: &mut Window, cx: &mut App) -> AnyElement {
    let label = cx.global::<Translator>().t("ccAgent.layout.new", &[]);
    let new = tile(
        "rail-new",
        label,
        lucide::circle_plus().as_bytes(),
        false,
        window,
        cx,
    )
    .on_click(|_, _, cx| {
        log::info!(target: "sidebar", "New from the rail");
        router::navigate("/cc-agent/new", cx);
    });
    let border = cx.global::<Appearance>().hsl_token("sidebar-border");
    div()
        .id("rail")
        .debug_selector(|| "rail".into())
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .items_center()
        .gap(css(3.0))
        .overflow_y_scroll()
        .px(css(8.0))
        .pt(css(12.0))
        .pb(css(8.0))
        .child(new)
        .child(search::tile(window, cx))
        .child(
            div()
                .flex_none()
                .my(css(7.0))
                .h(css(1.0))
                .w(css(22.0))
                .bg(border),
        )
        .children(pinned::section(window, cx))
        .children(sections::tiles(window, cx))
        .into_any_element()
}

/// A tile's Tab stop, the same handle each frame.
fn tab_stop(id: &str, window: &mut Window, cx: &mut App) -> FocusHandle {
    let key = ElementId::Name(format!("{id}-focus").into());
    window
        .use_keyed_state(key, cx, |_, cx| cx.focus_handle().tab_stop(true))
        .read(cx)
        .clone()
}

/// `SidebarIconButton`, 36px round; `open` is `data-state=open`.
fn tile(
    id: &'static str,
    label: String,
    source: &[u8],
    open: bool,
    window: &mut Window,
    cx: &mut App,
) -> gpui::Stateful<gpui::Div> {
    let focus = tab_stop(id, window, cx);
    let hovered =
        window.use_keyed_state(ElementId::Name(format!("{id}-hover").into()), cx, |_, _| {
            false
        });
    let lit = *hovered.read(cx);
    // `cindy-button-frame`: no press inset under reduced motion.
    let inset = if cx.global::<ReducedMotion>().0 {
        0.0
    } else {
        1.0
    };
    let appearance = cx.global::<Appearance>();
    let hover = appearance.hsl_token("sidebar-item-hover");
    let (fg, chip) = match open {
        true => (
            appearance.token("msg-assistant-text"),
            Some(appearance.token("chat-input-chip-bg")),
        ),
        false => (appearance.hsl_token("titlebar-icon"), None),
    };
    let svg: Svg = lucide::icon(source, 18.0, 2.0).text_color(fg);
    div()
        .id(id)
        .debug_selector(move || id.into())
        .track_focus(&focus)
        .role(Role::Button)
        .aria_label(label.clone())
        .group(id)
        .relative()
        .flex()
        .flex_none()
        .size(css(36.0))
        .items_center()
        .justify_center()
        .rounded(css(9999.0))
        .cursor_pointer()
        .tooltip(tip(label))
        .hover_listener_mode(HoverListenerMode::InputModalityIndependent)
        .on_hover(move |h, _, cx| {
            hovered.update(cx, |hovered, cx| {
                *hovered = *h;
                cx.notify();
            })
        })
        .child(
            div()
                .id("face")
                .debug_selector(move || format!("{id}-face"))
                // A cursor gives the face the hitbox `group_active` needs.
                .cursor_pointer()
                .absolute()
                .inset_0()
                .rounded(css(9999.0))
                .when(lit, |el| el.bg(hover))
                .when_some(chip, |el, chip| el.bg(chip))
                .group_active(id, |s| s.bg(hover).inset(css(inset))),
        )
        .child(svg)
}
