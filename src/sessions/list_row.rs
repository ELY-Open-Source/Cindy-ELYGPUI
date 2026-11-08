//! `SessionCard.tsx`'s `list` variant: a flat row, the title line over one preview line.

use gpui::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _,
};

use super::{
    Sessions,
    group::Session,
    rename,
    row::{ROW_GROUP, Slot, rename_on_double, source_label, status_glyph, with_menu},
    title,
};
use crate::{
    appearance_settings::text_size, i18n::Translator, theme::Appearance,
    theme_import::color::js_trim, units::css,
};

/// `SIDEBAR_LIST_ROW_HEIGHT_CLASS`: 8 + 22 + 4 + 8 and one preview line.
pub(super) fn height(cx: &App) -> f32 {
    42.0 + 1.45 * text_size(12.0, cx)
}

/// A row; `first` adds the top divider, as `isFirst` does. A right click opens the task menu.
pub(super) fn list_row(
    s: &Session,
    first: bool,
    label: Option<&str>,
    now: i64,
    cx: &App,
) -> AnyElement {
    let appearance = cx.global::<Appearance>();
    let xs = text_size(12.0, cx);
    let editing = cx
        .global::<Sessions>()
        .editing
        .as_ref()
        .filter(|e| e.id == s.id);
    // `bg-[color-mix(in_srgb,hsl(var(--sidebar-border))_50%,transparent)]`.
    let line = appearance.hsl_token("sidebar-border").opacity(0.5);
    let divider = move |top: bool| {
        div()
            .absolute()
            .left(css(10.0))
            .right(css(10.0))
            .h(css(1.0))
            .map(|el| if top { el.top_0() } else { el.bottom_0() })
            .bg(line)
            .group_hover(ROW_GROUP, |st| st.opacity(0.0))
    };
    let prefix = div()
        .flex()
        .flex_none()
        .items_center()
        .child(
            div()
                .flex()
                .w(css(12.0))
                .items_center()
                .justify_center()
                .child(status_glyph(s, cx)),
        )
        .child(div().w(css(6.0)));
    let title = div()
        .flex()
        .min_w_0()
        .flex_1()
        .items_center()
        .gap(css(4.0))
        .child(
            div()
                .min_w_0()
                .flex_shrink(1.0)
                .truncate()
                .text_size(css(text_size(14.0, cx)))
                .line_height(gpui::relative(1.3))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(appearance.hsl_token("foreground"))
                .child(title::display(s, cx.global::<Translator>())),
        )
        .when_some(label, |el, label| el.child(source_label(&s.id, label, cx)));
    let slot = div()
        .relative()
        .flex()
        .h(css(22.0))
        .min_w_0()
        .flex_1()
        .items_center()
        .child(prefix);
    let slot = match editing {
        Some(editing) => slot.child(rename::field(editing, cx)),
        None => slot.child(title),
    };
    let line_one =
        div()
            .flex()
            .items_center()
            .gap(css(6.0))
            .child(slot)
            .when(editing.is_none(), |el| {
                el.child(Slot {
                    session: s.clone(),
                    now,
                    height: 22.0,
                })
            });
    // `resolveSessionCardBody`: the latest message, trimmed; the line stays when empty.
    let body = s
        .preview
        .as_deref()
        .map(js_trim)
        .filter(|p| !p.is_empty())
        .map(str::to_owned);
    let preview = div()
        .mt(css(4.0))
        .h(css(1.45 * xs))
        .overflow_hidden()
        .truncate()
        .text_size(css(xs))
        .line_height(css(1.45 * xs))
        .text_color(appearance.token("text-secondary"))
        .children(body);
    let selector = format!("task-{}", s.id);
    let id = s.id.clone();
    let row = div()
        .id(gpui::SharedString::from(selector.clone()))
        .debug_selector(move || selector)
        .group(ROW_GROUP)
        .relative()
        .w_full()
        .h(css(height(cx)))
        .px(css(10.0))
        .py(css(8.0))
        .overflow_hidden()
        .rounded(css(8.0))
        .when(editing.is_none(), |el| el.cursor_pointer())
        .hover(|st| st.bg(appearance.hsl_token("sidebar-item-hover")))
        .on_click(move |event, window, cx| rename_on_double(&id, event, window, cx))
        .child(divider(false))
        .when(first, |el| el.child(divider(true)))
        .child(line_one)
        .child(preview);
    with_menu(s, editing.is_some(), row.into_any_element(), cx)
}
