//! The main list in `flat` group-by mode (`mainListModel.ts`): unpinned tasks, one row each.

use gpui::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled, div,
};

use super::{
    Sessions,
    group::{Session, for_main_list},
};
use crate::{
    appearance_settings::text_size,
    i18n::{Arg, Translator},
    preferences::Preferences,
    theme::Appearance,
    tip::tip,
    units::css,
};

const GROUP_BY_KEY: &str = "cc-agent.sidebar.filter.groupBy";
pub(super) const SORT_BY_KEY: &str = "cc-agent.sidebar.filter.sortBy";

/// The unpinned tasks in `sortSessionsForMainList`'s order.
pub(super) fn sorted(rows: &[Session], sort_by: Option<&str>) -> Vec<Session> {
    let unpinned = rows
        .iter()
        .filter(|r| r.pinned_at.is_none())
        .cloned()
        .collect();
    for_main_list(unpinned, sort_by == Some("created"))
}

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

/// `SessionItem` in list mode: a 32 px pill, the title, the relative time; `indented` in a group.
pub(super) fn row(row: &Session, indented: bool, now: i64, cx: &App) -> gpui::Stateful<gpui::Div> {
    let appearance = cx.global::<Appearance>();
    let translator = cx.global::<Translator>();
    let t = |key: &str, count: i64| translator.t(key, &[("count", Arg::Number(count))]);
    let (title_size, time_size) = (text_size(14.0, cx), text_size(12.0, cx));
    let time = div()
        .id(gpui::SharedString::from(format!("task-time-{}", row.id)))
        .flex_none()
        .ml_auto()
        .text_size(css(time_size))
        .text_color(appearance.token("cmd-palette-item-meta"))
        .tooltip(tip(absolute(row.activity_at)))
        .child(relative(row.activity_at, now, &t));
    let selector = format!("task-{}", row.id);
    div()
        .id(gpui::SharedString::from(selector.clone()))
        .debug_selector(move || selector)
        .flex()
        .items_center()
        .gap(css(10.0))
        .h(css(ROW_HEIGHT))
        .w_full()
        .pl(css(if indented { 20.0 } else { 12.0 }))
        .pr(css(8.0))
        .rounded(css(9999.0))
        .cursor_pointer()
        .text_size(css(title_size))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(appearance.hsl_token("foreground"))
        .hover(|s| s.bg(appearance.hsl_token("sidebar-item-hover")))
        .child(div().min_w_0().flex_1().truncate().child(row.title.clone()))
        .child(time)
}

pub(super) const ROW_HEIGHT: f32 = 32.0;

/// `ProjectsSection`'s scroll body: `relative flex flex-col gap-1 pt-1 pl-3`.
pub(super) fn scroll(
    id: &'static str,
    children: impl IntoIterator<Item = AnyElement>,
) -> AnyElement {
    let entries = div()
        .flex()
        .flex_col()
        .gap(css(4.0))
        .pt(css(4.0))
        .pl(css(12.0))
        .children(children);
    div()
        .id(id)
        .debug_selector(move || id.into())
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .pb(css(16.0))
        .overflow_y_scroll()
        .child(entries)
        .into_any_element()
}

/// The rows, when the sidebar filter groups the list flat.
pub fn list(cx: &App) -> Option<AnyElement> {
    let prefs = cx.global::<Preferences>();
    if prefs.text(GROUP_BY_KEY).ok().flatten() != Some("flat") {
        return None;
    }
    let rows = &cx.global::<Sessions>().loaded.as_ref()?.sessions;
    let sort_by = prefs.text(SORT_BY_KEY).ok().flatten();
    let now = jiff::Timestamp::now().as_millisecond();
    let items: Vec<AnyElement> = sorted(rows, sort_by)
        .iter()
        .map(|r| row(r, false, now, cx).into_any_element())
        .collect();
    Some(scroll("main-list-flat", items))
}
