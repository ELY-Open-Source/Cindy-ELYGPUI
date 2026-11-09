//! `SidebarTopNav.tsx`'s fixed section over the task list: the New row.

use gpui::{
    App, InteractiveElement as _, IntoElement, ParentElement as _, Role,
    StatefulInteractiveElement as _, Styled as _, Window, div,
};

use crate::{
    appearance_settings::text_size, i18n::Translator, lucide, router, theme::Appearance, units::css,
};

/// `section="pinned"`: `flex flex-col gap-0.5 pt-1 pr-3 pl-3 pb-0.5` around New.
pub fn pinned(window: &mut Window, cx: &mut App) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .flex_none()
        .gap(css(2.0))
        .pt(css(4.0))
        .px(css(12.0))
        .pb(css(2.0))
        .child(new_row(window, cx))
}

/// `handleNew`; the current task's computer rides along with the New draft (chapter 9).
fn new_row(window: &mut Window, cx: &mut App) -> impl IntoElement {
    let label = cx.global::<Translator>().t("ccAgent.layout.new", &[]);
    let focus = window
        .use_keyed_state("nav-new-focus", cx, |_, cx| {
            cx.focus_handle().tab_stop(true)
        })
        .read(cx)
        .clone();
    let appearance = cx.global::<Appearance>();
    let text = appearance.token("sidebar-nav-text");
    let hover = appearance.hsl_token("sidebar-item-hover");
    // `ROW_CLASS`.
    div()
        .id("nav-new")
        .debug_selector(|| "nav-new".into())
        .track_focus(&focus)
        .role(Role::Button)
        .aria_label(label.clone())
        .flex()
        .h(css(32.0))
        .w_full()
        .items_center()
        .gap(css(10.0))
        .rounded(css(9999.0))
        .px(css(12.0))
        .cursor_pointer()
        .text_size(css(text_size(14.0, cx)))
        .line_height(css(text_size(14.0, cx)))
        .text_color(text)
        .hover(|s| s.bg(hover))
        .on_click(|_, _, cx| {
            log::info!(target: "sidebar", "New from the top nav");
            router::navigate("/cc-agent/new", cx);
        })
        .child(lucide::icon(lucide::circle_plus().as_bytes(), 15.0, 1.8).text_color(text))
        .child(label)
}

#[cfg(test)]
mod tests;
