//! `getSessionListCollapseView`'s first N and `SectionCollapse`'s 200 ms fold.

use gpui::{AnyElement, App, IntoElement, ParentElement as _, Styled as _, Window, div};

use super::Sessions;
use crate::{
    motion::{BASE, Tween, length},
    units::css,
};

/// `SESSION_LIST_COLLAPSE_RECENT_WINDOW_MS`.
const RECENT_WINDOW_MS: i64 = 24 * 60 * 60 * 1000;

/// The visible entries and the hidden count. `recent_since` keeps entries active since then,
/// as task lists do; the top-level list passes `None`. Attention and the open task wait for 10.0.
pub fn view<T>(
    entries: &[T],
    limit: usize,
    show_all: bool,
    recent_since: Option<i64>,
    activity: impl Fn(&T) -> i64,
) -> (Vec<&T>, usize) {
    if show_all {
        return (entries.iter().collect(), 0);
    }
    let visible: Vec<&T> = entries
        .iter()
        .enumerate()
        .filter(|(i, e)| *i < limit || recent_since.is_some_and(|since| activity(e) >= since))
        .map(|(_, e)| e)
        .collect();
    let hidden = entries.len() - visible.len();
    (visible, hidden)
}

pub fn recent_since(now_ms: i64) -> i64 {
    now_ms - RECENT_WINDOW_MS
}

/// `readLimit`: `Number.parseInt` of the stored text when it is at least 0, else the default.
pub fn limit(raw: Option<&str>, fallback: usize) -> usize {
    let Some(text) = raw.map(str::trim_start) else {
        return fallback;
    };
    let (negative, rest) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return fallback;
    }
    let zero = digits.bytes().all(|b| b == b'0');
    match (negative && !zero, digits.parse::<usize>()) {
        (true, _) => fallback,
        (false, Ok(n)) => n,
        (false, Err(_)) => usize::MAX,
    }
}

/// Turns a group's fold, animating from where it is; Show all resets once it has closed.
pub fn toggle(id: &str, collapsed: bool, cx: &mut App) {
    let now = cx.background_executor().now();
    let span = length(BASE, cx);
    let store = cx.global_mut::<Sessions>();
    let from = if collapsed { 1.0 } else { 0.0 };
    let tween = store
        .folds
        .entry(id.to_owned())
        .or_insert_with(|| Tween::still(from, now));
    tween.toward(if collapsed { 0.0 } else { 1.0 }, now, span);
    if !collapsed {
        return;
    }
    let id = id.to_owned();
    cx.spawn(async move |cx| {
        cx.background_executor().timer(span).await;
        cx.update(|cx| {
            let now = cx.background_executor().now();
            let store = cx.global_mut::<Sessions>();
            if store.folds.get(&id).is_some_and(|t| t.at(now).0 <= 0.0) {
                store.show_all.remove(&id);
                cx.refresh_windows();
            }
        });
    })
    .detach();
}

/// `SectionCollapse`: the body's height and opacity follow the fold; closed, it unmounts.
pub fn section(
    id: &str,
    collapsed: bool,
    height: f32,
    body: impl FnOnce() -> AnyElement,
    window: &mut Window,
    cx: &App,
) -> Option<AnyElement> {
    let (open, moving) = match cx.global::<Sessions>().folds.get(id) {
        Some(tween) => tween.at(cx.background_executor().now()),
        None => (if collapsed { 0.0 } else { 1.0 }, false),
    };
    if moving {
        window.request_animation_frame();
    } else if open >= 1.0 {
        return Some(body());
    }
    if open <= 0.0 {
        return None;
    }
    Some(
        div()
            .w_full()
            .overflow_hidden()
            .h(css(height * open))
            .opacity(open)
            .child(body())
            .into_any_element(),
    )
}
