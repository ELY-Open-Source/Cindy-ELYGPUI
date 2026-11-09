//! A task row as `SessionItem.tsx` draws it in text mode.

use std::sync::Arc;

use ely_gpui_component::primitives::IconName;
use gpui::{
    AnyElement, App, ClickEvent, ElementId, FontFeatures, HoverListenerMode,
    InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement as _, Styled, Window, div, prelude::FluentBuilder as _,
};

use super::{
    Sessions,
    group::Session,
    keyboard::{Probe, with_menu},
    marquee, meta, open, rename, select, task_menu, title,
};
use crate::{
    appearance_settings::text_size,
    i18n::{Arg, Translator},
    lucide,
    theme::Appearance,
    tip::tip,
    units::css,
};

/// `formatSidebarTime`: now, minutes, hours, days, weeks, months, years.
pub(super) fn relative(at_ms: i64, now_ms: i64, t: &dyn Fn(&str, i64) -> String) -> String {
    const MINUTE: i64 = 60_000;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;
    let diff = (now_ms - at_ms).max(0);
    let steps = [
        (HOUR, MINUTE, "minute"),
        (DAY, HOUR, "hour"),
        (7 * DAY, DAY, "day"),
        (30 * DAY, 7 * DAY, "week"),
        (365 * DAY, 30 * DAY, "month"),
    ];
    if diff < MINUTE {
        return t("ccAgent.time.relative.now", 0);
    }
    for (below, unit, key) in steps {
        if diff < below {
            return t(
                &format!("ccAgent.time.relative.{key}"),
                (diff / unit).max(1),
            );
        }
    }
    t("ccAgent.time.relative.year", (diff / (365 * DAY)).max(1))
}

/// `formatSidebarTimeAbsolute`: `YYYY-MM-DD HH:mm`, local time.
fn absolute(at_ms: i64) -> String {
    jiff::Timestamp::from_millisecond(at_ms)
        .map(|t| {
            t.to_zoned(jiff::tz::TimeZone::system())
                .strftime("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_default()
}

/// `SessionStatusIcon` at rest: the archive box, the Orca lead's people, else the harness mark;
/// running, attention and drafts come with the agent runtime (10.0). `size` overrides each; the
/// active row draws it in its foreground.
pub(super) fn status_glyph(s: &Session, size: Option<f32>, active: bool, cx: &App) -> AnyElement {
    let appearance = cx.global::<Appearance>();
    let active_fg = appearance.token("sidebar-item-active-foreground");
    let meta = match active {
        true => active_fg,
        false => appearance.token("cmd-palette-item-meta"),
    };
    if s.status == "archived" {
        lucide::icon(IconName::Archive.svg(), size.unwrap_or(12.0), 1.75)
            .text_color(meta)
            .into_any_element()
    } else if s.orca_lead {
        lucide::icon(lucide::users_round().as_bytes(), size.unwrap_or(13.0), 1.75)
            .text_color(meta)
            .into_any_element()
    } else {
        // `VendorIcon`: Claude Code at 13 px, Codex and Pi at 12.
        let muted = match active {
            true => active_fg,
            false => appearance.hsl_token("sidebar-muted"),
        };
        match s.agent_kind.as_str() {
            "codex" => lucide::mark(
                &lucide::codex_mark(size.unwrap_or(12.0)),
                size.unwrap_or(12.0),
            )
            .text_color(muted)
            .into_any_element(),
            "pi" => div()
                .flex()
                .size(css(size.unwrap_or(12.0)))
                .items_center()
                .justify_center()
                .text_size(css(size.unwrap_or(12.0) * 0.86))
                .line_height(css(size.unwrap_or(12.0)))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(muted)
                .child("π")
                .into_any_element(),
            _ => lucide::mark(&lucide::claude_mark(), size.unwrap_or(13.0))
                .text_color(muted)
                .into_any_element(),
        }
    }
}

/// How `SessionInfoMeta` is sized and colored where it sits.
pub(super) struct InfoStyle {
    pub size: f32,
    pub color: gpui::Hsla,
    /// The slot's height; none is `leading-none`.
    pub height: Option<f32>,
}

/// `SessionInfoMeta`: the ticked fields, `·` between them.
pub(super) fn info(s: &Session, now: i64, style: InfoStyle, cx: &App) -> Option<AnyElement> {
    let translator = cx.global::<Translator>();
    let t = |key: &str, count: i64| translator.t(key, &[("count", Arg::Number(count))]);
    let fields = cx.global::<super::Sessions>().task_info.clone();
    let pieces = meta::pieces(
        s,
        &fields,
        |at| (relative(at, now, &t), absolute(at)),
        |key| translator.t(key, &[]),
    );
    if pieces.is_empty() {
        return None;
    }
    let children = pieces.into_iter().enumerate().map(|(i, piece)| {
        let selector = format!("task-{}-{}", piece.field, s.id);
        div()
            .id(gpui::SharedString::from(selector.clone()))
            .debug_selector(move || selector)
            .flex()
            .flex_none()
            .items_center()
            .gap(css(4.0))
            .tooltip(tip(piece.tip))
            .when(i > 0, |el| el.child(div().opacity(0.5).child("·")))
            .child(piece.text)
    });
    Some(
        div()
            .flex()
            .flex_none()
            .map(|el| match style.height {
                Some(h) => el.h(css(h)),
                None => el.line_height(css(style.size)),
            })
            .items_center()
            .gap(css(4.0))
            .text_size(css(style.size))
            .font_weight(gpui::FontWeight::MEDIUM)
            .font_features(FontFeatures(Arc::new(vec![("tnum".into(), 1)])))
            .text_color(style.color)
            .children(children)
            .into_any_element(),
    )
}

/// The hover group of one row.
pub(super) const ROW_GROUP: &str = "task-row";

/// The right slot: the info, which gives way on hover to More (`SessionAction`).
/// The info slot; a hover, or More holding focus as `group-focus-within/slot`, swaps in More.
#[derive(IntoElement)]
pub(super) struct Slot {
    pub session: Session,
    pub now: i64,
    /// 24 in the text row, 22 in the list row's title line.
    pub height: f32,
    pub active: bool,
    pub more: Probe,
}

impl RenderOnce for Slot {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let s = &self.session;
        let focus = window
            .use_keyed_state(
                ElementId::Name(format!("task-more-focus-{}", s.id).into()),
                cx,
                |_, cx| cx.focus_handle().tab_stop(true),
            )
            .read(cx)
            .clone();
        let focused = focus.is_focused(window);
        let menu = task_menu::menu(s, cx.global::<Translator>());
        div()
            .relative()
            .flex()
            .flex_none()
            .h(css(self.height))
            .ml_auto()
            .items_center()
            .justify_end()
            .children(
                info(
                    s,
                    self.now,
                    InfoStyle {
                        size: text_size(12.0, cx),
                        color: match self.active {
                            true => cx
                                .global::<Appearance>()
                                .token("sidebar-item-active-foreground"),
                            false => cx.global::<Appearance>().hsl_token("sidebar-action-icon"),
                        },
                        height: Some(self.height),
                    },
                    cx,
                )
                .map(|info| {
                    div()
                        .flex()
                        .overflow_hidden()
                        .when(focused, |el| el.w(css(0.0)).opacity(0.0))
                        .group_hover(ROW_GROUP, |st| st.w(css(0.0)).opacity(0.0))
                        .child(info)
                }),
            )
            .child(
                div()
                    .hidden()
                    .size(css(20.0))
                    .when(focused, |el| el.flex())
                    .group_hover(ROW_GROUP, |st| st.flex()),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .flex()
                    .h(css(self.height))
                    .items_center()
                    .gap(css(2.0))
                    .opacity(if focused { 1.0 } else { 0.0 })
                    .group_hover(ROW_GROUP, |st| st.opacity(1.0))
                    .child(task_menu::More {
                        id: s.id.clone(),
                        menu,
                        focus,
                        card: false,
                        active: self.active,
                        probe: self.more,
                    }),
            )
    }
}

/// A 32 px pill: the status icon, the title, a pinned task's source label, the info slot, or
/// the rename field; `indented` inside a group. A right click opens the task menu.
pub(super) fn row(
    row: &Session,
    indented: bool,
    label: Option<&str>,
    now: i64,
    cx: &App,
) -> AnyElement {
    let appearance = cx.global::<Appearance>();
    let title_size = text_size(14.0, cx);
    let active = open::is_active(&row.id, cx);
    let selected = select::is_selected(&row.id, cx);
    let editing = cx
        .global::<Sessions>()
        .editing
        .as_ref()
        .filter(|e| e.id == row.id);
    let title = div()
        .flex()
        .min_w_0()
        .flex_1()
        .items_center()
        .gap(css(6.0))
        .child(marquee::title(
            &row.id,
            title::display(row, cx.global::<Translator>()),
            cx,
        ))
        .when_some(label, |el, label| {
            el.child(source_label(&row.id, label, active, cx))
        });
    let selector = format!("task-{}", row.id);
    let (id, hovered) = (row.id.clone(), row.id.clone());
    let body = div()
        .id(gpui::SharedString::from(selector.clone()))
        .debug_selector(move || selector)
        .group(ROW_GROUP)
        .flex()
        .items_center()
        .gap(css(10.0))
        .h(css(ROW_HEIGHT))
        .w_full()
        .pl(css(if indented { 20.0 } else { 12.0 }))
        .pr(css(8.0))
        .rounded(css(9999.0))
        .when(editing.is_none(), |el| el.cursor_pointer())
        .text_size(css(title_size))
        .font_weight(gpui::FontWeight::MEDIUM)
        .map(|el| match (active, selected) {
            (true, _) => el
                .bg(appearance.hsl_token("sidebar-item-active"))
                .text_color(appearance.token("sidebar-item-active-foreground"))
                .relative()
                .child(ring(&row.id, 9999.0, cx)),
            (false, true) => el
                .bg(appearance.token("chat-input-chip-bg"))
                .text_color(appearance.hsl_token("foreground")),
            (false, false) => el
                .text_color(appearance.hsl_token("foreground"))
                .hover(|s| s.bg(appearance.hsl_token("sidebar-item-hover"))),
        })
        .when(selected, |el| {
            el.aria_toggled(gpui::Toggled::True)
                .child(selected_ring(&row.id, 9999.0, cx))
        })
        .on_click(move |event, window, cx| open::clicked(&id, event, window, cx))
        .hover_listener_mode(HoverListenerMode::InputModalityIndependent)
        .on_hover(move |h, window, cx| marquee::hover(&hovered, *h, window, cx))
        .child(marquee::mark(&row.id))
        .child(
            div()
                .flex()
                .flex_none()
                .w(css(15.0))
                .items_center()
                .justify_center()
                .child(status_glyph(row, None, active, cx)),
        );
    let more = Probe::default();
    let body = match editing {
        Some(editing) => body.child(rename::field(editing, rename::TEXT, active, cx)),
        None => body.child(title).child(Slot {
            session: row.clone(),
            now,
            height: ROW_SLOT,
            active,
            more: more.clone(),
        }),
    };
    with_menu(row, body, more).into_any_element()
}

/// `sourceLabel`, its full text in `title`; 70% of the active foreground on the active row.
pub(super) fn source_label(id: &str, label: &str, active: bool, cx: &App) -> AnyElement {
    let appearance = cx.global::<Appearance>();
    let color = match active {
        true => appearance
            .token("sidebar-item-active-foreground")
            .opacity(0.7),
        false => appearance.token("cmd-palette-item-meta"),
    };
    let selector = format!("task-label-{id}");
    div()
        .id(gpui::SharedString::from(selector.clone()))
        .debug_selector(move || selector)
        .min_w_0()
        .truncate()
        .text_size(css(text_size(12.0, cx)))
        .font_weight(gpui::FontWeight::NORMAL)
        .text_color(color)
        .tooltip(tip(label.to_owned()))
        .child(label.to_owned())
        .into_any_element()
}

/// `shadow-[inset_0_0_0_1px_var(--sidebar-item-active-border)]`: a ring inside the box, out
/// of layout.
pub(super) fn ring(id: &str, radius: f32, cx: &App) -> gpui::Div {
    let selector = format!("task-active-{id}");
    div()
        .debug_selector(move || selector)
        .absolute()
        .inset_0()
        .rounded(css(radius))
        .border(css(1.0))
        .border_color(
            cx.global::<Appearance>()
                .token("sidebar-item-active-border"),
        )
}

/// `ring-1 ring-inset ring-[var(--focus-ring-soft)]` on a selected task, over the active ring.
pub(super) fn selected_ring(id: &str, radius: f32, cx: &App) -> gpui::Div {
    let selector = format!("task-selected-{id}");
    div()
        .debug_selector(move || selector)
        .absolute()
        .inset_0()
        .rounded(css(radius))
        .border(css(1.0))
        .border_color(cx.global::<Appearance>().token("focus-ring-soft"))
}

/// A double click renames, as `handleDoubleClick` does.
pub(super) fn rename_on_double(id: &str, event: &ClickEvent, window: &mut Window, cx: &mut App) {
    if event.click_count() != 2 {
        return;
    }
    let found = cx
        .global::<Sessions>()
        .loaded
        .as_ref()
        .and_then(|l| l.sessions.iter().find(|s| s.id == id).cloned());
    if let Some(s) = found {
        rename::start(&s, window, cx);
    }
}

pub(super) const ROW_HEIGHT: f32 = 32.0;
const ROW_SLOT: f32 = 24.0;
