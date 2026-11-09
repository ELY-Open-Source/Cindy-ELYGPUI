//! `SessionCard.tsx`'s `card` variant: title, body and a meta row in a bordered card.

use gpui::{
    AnyElement, App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _,
    RenderOnce, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _,
};

use super::{
    Sessions,
    group::Session,
    open, rename,
    row::{InfoStyle, ROW_GROUP, info, status_glyph, with_menu},
    task_menu, title,
};
use crate::{
    appearance_settings::text_size, i18n::Translator, theme::Appearance,
    theme_import::color::js_trim, units::css,
};

/// `resolveSessionCardBody`: a pinned card's summary, else the latest message.
pub(super) fn body(s: &Session) -> Option<&str> {
    let summary = s.summary.as_deref().map(js_trim).filter(|t| !t.is_empty());
    let summary = summary.filter(|_| s.pinned_at.is_some());
    summary.or_else(|| s.preview.as_deref().map(js_trim).filter(|t| !t.is_empty()))
}

/// `cardPreviewLineClamp`; a running task's two lines come with 10.0.
pub(super) fn clamp(s: &Session) -> usize {
    let summary = s.summary.as_deref().is_some_and(|t| !t.is_empty());
    let automation = s.source == "scheduler" || s.title.starts_with(title::SCHEDULE_PREFIX);
    match () {
        _ if s.pinned_at.is_some() && summary => 3,
        _ if automation => 1,
        _ => 2,
    }
}

/// One card; a right click opens the task menu.
#[derive(IntoElement)]
pub(super) struct Card {
    pub session: Session,
    pub now: i64,
}

impl RenderOnce for Card {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        card(&self.session, self.now, window, cx)
    }
}

fn card(s: &Session, now: i64, window: &mut Window, cx: &mut App) -> AnyElement {
    let focus = window
        .use_keyed_state(
            ElementId::Name(format!("task-more-focus-{}", s.id).into()),
            cx,
            |_, cx| cx.focus_handle().tab_stop(true),
        )
        .read(cx)
        .clone();
    let focused = focus.is_focused(window);
    let appearance = cx.global::<Appearance>();
    let editing = cx
        .global::<Sessions>()
        .editing
        .as_ref()
        .filter(|e| e.id == s.id);
    let small = text_size(11.0, cx);
    let active = open::is_active(&s.id, cx);
    let active_fg = appearance.token("sidebar-item-active-foreground");
    // The active card: its own surface and border, its foreground for text and meta.
    let (foreground, secondary, tertiary, meta) = match active {
        true => (active_fg, active_fg.opacity(0.8), active_fg, active_fg),
        false => (
            appearance.hsl_token("foreground"),
            appearance.token("text-secondary"),
            appearance.token("text-tertiary"),
            appearance.token("cmd-palette-item-meta"),
        ),
    };
    let (surface, border, hover) = match active {
        true => (
            appearance.hsl_token("sidebar-item-active"),
            appearance.token("sidebar-item-active-border"),
            None,
        ),
        false => (
            appearance.token("surface-elevated"),
            appearance.hsl_token("sidebar-border"),
            Some(appearance.hsl_token("sidebar-item-hover")),
        ),
    };
    let actions = editing.is_none().then(|| {
        div()
            .absolute()
            .top(css(6.0))
            .right(css(6.0))
            .flex()
            .items_center()
            .gap(css(2.0))
            .opacity(if focused { 1.0 } else { 0.0 })
            .group_hover(ROW_GROUP, |st| st.opacity(1.0))
            .child(task_menu::More {
                id: s.id.clone(),
                menu: task_menu::menu(s, cx.global::<Translator>()),
                focus,
                card: true,
                active: false,
            })
    });
    let title_size = text_size(12.0, cx);
    let heading = div()
        .relative()
        .flex()
        .items_start()
        .child(
            div()
                .min_w_0()
                .text_size(css(title_size))
                .line_height(gpui::relative(1.22))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(foreground)
                .line_clamp(2)
                .when(editing.is_some(), |el| el.invisible())
                .child(title::display(s, cx.global::<Translator>())),
        )
        .when_some(editing, |el, editing| {
            el.child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .child(rename::field(
                        editing,
                        (12.0, gpui::FontWeight::SEMIBOLD),
                        active,
                        cx,
                    )),
            )
        });
    let preview = body(s).map(|text| {
        div()
            .mt(css(4.0))
            .text_size(css(small))
            .line_height(gpui::relative(1.4))
            .text_color(secondary)
            .line_clamp(clamp(s))
            .child(text.to_owned())
    });
    let style = InfoStyle {
        size: small,
        color: meta,
        height: None,
    };
    let info = info(s, now, style, cx).map(|info| div().ml_auto().flex_none().child(info));
    let meta_row = div()
        .mt(css(6.0))
        .flex()
        .items_center()
        .gap(css(6.0))
        .text_size(css(small))
        .line_height(css(small))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(tertiary)
        .child(status_glyph(s, Some(11.0), active, cx))
        .children(info);
    let selector = format!("task-{}", s.id);
    let id = s.id.clone();
    let card = div()
        .id(gpui::SharedString::from(selector.clone()))
        .debug_selector(move || selector)
        .group(ROW_GROUP)
        .relative()
        .w_full()
        .overflow_hidden()
        .rounded(css(12.0))
        .bg(surface)
        .border(css(1.0))
        .border_color(border)
        .when(editing.is_none(), |el| el.cursor_pointer())
        .when_some(hover, |el, hover| el.hover(|st| st.bg(hover)))
        .on_click(move |event, window, cx| open::clicked(&id, event, window, cx))
        .child(
            div()
                .relative()
                .flex()
                .flex_col()
                .px(css(10.0))
                .py(css(8.0))
                .child(heading)
                .children(preview)
                .child(meta_row)
                .children(actions),
        );
    with_menu(s, card.into_any_element(), cx)
}

#[cfg(test)]
mod tests;
