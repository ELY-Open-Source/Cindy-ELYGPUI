//! The main list in `flat` group-by mode (`mainListModel.ts`): unpinned tasks, one row each.

use gpui::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled, div,
};

use super::{Sessions, query::Row};
use crate::{
    appearance_settings::text_size,
    i18n::{Arg, Translator},
    preferences::Preferences,
    theme::Appearance,
    tip::tip,
    units::css,
};

const GROUP_BY_KEY: &str = "cc-agent.sidebar.filter.groupBy";
const SORT_BY_KEY: &str = "cc-agent.sidebar.filter.sortBy";

/// `sortSessionsForMainList`: `created` by creation, else by activity; ties keep list order.
/// `priority` first ranks waiting and running tasks, which the agent runtime reports (10.0).
pub(super) fn sorted<'a>(rows: &'a [Row], sort_by: Option<&str>) -> Vec<&'a Row> {
    let mut rows: Vec<&Row> = rows.iter().filter(|r| !r.pinned).collect();
    if sort_by == Some("created") {
        rows.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| a.id.cmp(&b.id))
        });
    } else {
        rows.sort_by_key(|r| std::cmp::Reverse(r.activity_at));
    }
    rows
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

/// The rows, when the sidebar filter groups the list flat; project grouping arrives with 8.4.
pub fn list(cx: &App) -> Option<AnyElement> {
    let prefs = cx.global::<Preferences>();
    if prefs.text(GROUP_BY_KEY).ok().flatten() != Some("flat") {
        return None;
    }
    let rows = cx.global::<Sessions>().rows.as_ref()?;
    let sort_by = prefs.text(SORT_BY_KEY).ok().flatten();
    let appearance = cx.global::<Appearance>();
    let translator = cx.global::<Translator>();
    let t = |key: &str, count: i64| translator.t(key, &[("count", Arg::Number(count))]);
    let now = jiff::Timestamp::now().as_millisecond();
    let (title_size, time_size) = (text_size(14.0, cx), text_size(12.0, cx));
    let items = sorted(rows, sort_by).into_iter().map(|row| {
        let time = div()
            .id(gpui::SharedString::from(format!("task-time-{}", row.id)))
            .flex_none()
            .ml_auto()
            .text_size(css(time_size))
            .text_color(appearance.token("cmd-palette-item-meta"))
            .tooltip(tip(absolute(row.activity_at)))
            .child(relative(row.activity_at, now, &t));
        div()
            .id(gpui::SharedString::from(format!("task-{}", row.id)))
            .flex()
            .items_center()
            .gap(css(10.0))
            .h(css(32.0))
            .w_full()
            .pl(css(12.0))
            .pr(css(8.0))
            .rounded(css(9999.0))
            .cursor_pointer()
            .text_size(css(title_size))
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(appearance.hsl_token("foreground"))
            .hover(|s| s.bg(appearance.hsl_token("sidebar-item-hover")))
            .child(div().min_w_0().flex_1().truncate().child(row.title.clone()))
            .child(time)
    });
    let entries = div()
        .flex()
        .flex_col()
        .gap(css(4.0))
        .pt(css(4.0))
        .pl(css(12.0))
        .children(items);
    Some(
        div()
            .id("main-list-flat")
            .debug_selector(|| "main-list-flat".into())
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .pb(css(16.0))
            .overflow_y_scroll()
            .child(entries)
            .into_any_element(),
    )
}
