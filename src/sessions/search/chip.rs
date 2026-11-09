//! The inline row's chip for the locked scope.

use ely_gpui_component::primitives::IconName;
use gpui::{
    AnyElement, App, FocusHandle, HoverListenerMode, InteractiveElement as _, IntoElement as _,
    ParentElement as _, Role, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _,
};

use super::{Which, lock};
use crate::{
    appearance_settings::text_size, i18n::Translator, lucide::icon, theme::Appearance, tip::tip,
    units::css,
};

/// The locked scope's chip; its X clears the lock, as `clearLock` does.
pub(super) fn view(
    label: String,
    focus: &FocusHandle,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let hovered = window.use_keyed_state("nav-search-unlock-hover", cx, |_, _| false);
    let lit = *hovered.read(cx);
    let appearance = cx.global::<Appearance>();
    let reset = cx
        .global::<Translator>()
        .t("ccAgent.search.filter.reset", &[]);
    let (tertiary, strong) = (
        appearance.token("text-tertiary"),
        appearance.token("text-primary"),
    );
    let xs = text_size(12.0, cx);
    let unlock = div()
        .id("nav-search-unlock")
        .debug_selector(|| "nav-search-unlock".into())
        .track_focus(focus)
        .role(Role::Button)
        .aria_label(reset.clone())
        .relative()
        .flex()
        .flex_none()
        .size(css(16.0))
        .items_center()
        .justify_center()
        .rounded(css(9999.0))
        .cursor_pointer()
        .hover_listener_mode(HoverListenerMode::InputModalityIndependent)
        .on_hover(move |h, _, cx| {
            hovered.update(cx, |hovered, cx| {
                *hovered = *h;
                cx.notify();
            })
        })
        .tooltip(tip(reset))
        .on_click(|_, window, cx| {
            lock::clear(Which::Inline, cx);
            window.blur(cx);
        })
        .when(lit, |el| {
            el.child(
                div()
                    .debug_selector(|| "nav-search-unlock-hover".into())
                    .absolute()
                    .inset_0()
                    .rounded(css(9999.0))
                    .bg(appearance.hsl_token("sidebar-item-hover")),
            )
        })
        .child(
            icon(IconName::X.svg(), 11.0, 2.0)
                .relative()
                .text_color(if lit { strong } else { tertiary }),
        );
    div()
        .debug_selector(|| "nav-search-chip".into())
        .flex()
        .min_w_0()
        .max_w(gpui::relative(0.48))
        .items_center()
        .gap(css(4.0))
        .rounded(css(9999.0))
        .bg(appearance.token("surface-chip"))
        .py(css(2.0))
        .pl(css(8.0))
        .pr(css(2.0))
        .text_size(css(xs))
        .line_height(css(16.0))
        .text_color(appearance.token("text-secondary"))
        .child(div().min_w_0().truncate().child(label))
        .child(unlock)
        .into_any_element()
}
