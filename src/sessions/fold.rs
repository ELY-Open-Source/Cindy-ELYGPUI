//! `getSessionListCollapseView`'s first N and `SectionCollapse`'s 200 ms fold.

use gpui::{
    AnyElement, App, BoxShadow, InteractiveElement as _, IntoElement, ParentElement as _,
    RenderOnce, Role, StatefulInteractiveElement as _, Styled as _, Window, div, point, px,
};

use super::Sessions;
use crate::{
    appearance_settings::text_size,
    i18n::{Arg, Translator},
    motion::{BASE, Tween, length},
    theme::Appearance,
    units::{css, window_px},
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

/// Turns a group's fold, animating from where it is.
pub fn toggle(id: &str, collapsed: bool, cx: &mut App) {
    let now = cx.background_executor().now();
    let span = length(BASE, cx);
    let from = if collapsed { 1.0 } else { 0.0 };
    let tween = cx
        .global_mut::<Sessions>()
        .folds
        .entry(id.to_owned())
        .or_insert_with(|| Tween::still(from, now));
    tween.toward(if collapsed { 0.0 } else { 1.0 }, now, span);
}

/// How open a fold is now, 0 to 1.
pub fn openness(id: &str, collapsed: bool, cx: &App) -> f32 {
    match cx.global::<Sessions>().folds.get(id) {
        Some(tween) => tween.at(cx.background_executor().now()).0,
        None if collapsed => 0.0,
        None => 1.0,
    }
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

/// The Show all footer of `SessionEntryList` and `ShowAllEntriesButton`, a button.
#[derive(IntoElement)]
pub struct ShowAll {
    id: String,
    count: usize,
    show: Box<dyn Fn(&mut App)>,
}

impl ShowAll {
    pub fn new(id: String, count: usize, show: impl Fn(&mut App) + 'static) -> Self {
        Self {
            id,
            count,
            show: Box::new(show),
        }
    }
}

impl RenderOnce for ShowAll {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = window
            .use_keyed_state(format!("{}-focus", self.id), cx, |_, cx| {
                cx.focus_handle().tab_stop(true)
            })
            .read(cx)
            .clone();
        let appearance = cx.global::<Appearance>();
        let label = cx.global::<Translator>().t(
            "ccAgent.sidebar.showAllSessions",
            &[("count", Arg::Number(self.count as i64))],
        );
        // `focus-visible:ring-1 ring-[var(--focus-ring)]`.
        let ring = BoxShadow {
            color: appearance.token("focus-ring"),
            offset: point(px(0.0), px(0.0)),
            blur_radius: px(0.0),
            spread_radius: window_px(1.0, cx),
            inset: false,
        };
        let (id, show) = (self.id, self.show);
        let selector = id.clone();
        div()
            .id(gpui::SharedString::from(id.clone()))
            .debug_selector(move || selector)
            .track_focus(&focus)
            .role(Role::Button)
            .flex()
            .h(css(24.0))
            .w_full()
            .items_center()
            .justify_center()
            .rounded(css(9999.0))
            .px(css(8.0))
            .cursor_pointer()
            .text_size(css(text_size(12.0, cx)))
            .text_color(appearance.token("cmd-palette-item-meta"))
            .hover(|s| {
                s.bg(appearance.hsl_token("sidebar-item-hover"))
                    .text_color(appearance.hsl_token("foreground"))
            })
            .focus_visible(|s| s.shadow(vec![ring]))
            .on_click(move |_, _, cx| {
                log::info!(target: "sessions", "{id}: all shown");
                show(cx);
                cx.refresh_windows();
            })
            .child(label)
    }
}
