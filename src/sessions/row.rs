//! A task row as `SessionItem.tsx` draws it in text mode.

use gpui::{
    App, InteractiveElement as _, ParentElement, StatefulInteractiveElement as _, Styled, div,
    prelude::FluentBuilder as _,
};

use super::group::Session;
use crate::{
    appearance_settings::text_size,
    i18n::{Arg, Translator},
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

/// A 32 px pill: the title, the pinned task's source label, the relative time; `indented`
/// inside a group.
pub(super) fn row(
    row: &Session,
    indented: bool,
    label: Option<&str>,
    now: i64,
    cx: &App,
) -> gpui::Stateful<gpui::Div> {
    let appearance = cx.global::<Appearance>();
    let translator = cx.global::<Translator>();
    let t = |key: &str, count: i64| translator.t(key, &[("count", Arg::Number(count))]);
    let (title_size, time_size) = (text_size(14.0, cx), text_size(12.0, cx));
    let meta = appearance.token("cmd-palette-item-meta");
    let time = div()
        .id(gpui::SharedString::from(format!("task-time-{}", row.id)))
        .flex_none()
        .ml_auto()
        .text_size(css(time_size))
        .text_color(meta)
        .tooltip(tip(absolute(row.activity_at)))
        .child(relative(row.activity_at, now, &t));
    let title = div()
        .flex()
        .min_w_0()
        .flex_1()
        .items_center()
        .gap(css(6.0))
        .child(
            div()
                .min_w_0()
                .max_w_full()
                .flex_shrink(1.0)
                .truncate()
                .child(row.title.clone()),
        )
        .when_some(label, |el, label| {
            el.child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(css(time_size))
                    .font_weight(gpui::FontWeight::NORMAL)
                    .text_color(meta)
                    .child(label.to_owned()),
            )
        });
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
        .child(title)
        .child(time)
}

pub(super) const ROW_HEIGHT: f32 = 32.0;
