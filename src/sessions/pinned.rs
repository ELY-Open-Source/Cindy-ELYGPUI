//! `PinnedSection.tsx` in text mode: pinned tasks and projects, folded as one section.

use std::cell::RefCell;

use ely_gpui_component::primitives::IconName;
use gpui::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Window, div,
};

use super::{
    fold,
    lists::{Lists, Pinned},
    main_list::{Place, project, text},
    row::{ROW_HEIGHT, row},
};
use crate::{
    appearance_settings::text_size, i18n::Translator, lucide::icon, preferences::Preferences,
    theme::Appearance, tip::tip, units::css,
};

const COLLAPSED_KEY: &str = "cc-agent.sidebar.pinnedSectionCollapsed";
const FOLD: &str = "pinned-section";

/// `persistPinnedSectionCollapsed`: `true` stored, expanding removes it; unsaved, it stays.
fn toggle(collapsed: bool, cx: &mut App) {
    log::info!(target: "sessions", "pinned section {}", if collapsed { "folded" } else { "unfolded" });
    let prefs = cx.global_mut::<Preferences>();
    let saved = if collapsed {
        prefs.set(COLLAPSED_KEY, "true")
    } else {
        prefs.remove(COLLAPSED_KEY)
    };
    if let Err(error) = saved {
        return log::error!(target: "sessions", "pinned section fold not saved: {error:#}");
    }
    fold::toggle(FOLD, collapsed, cx);
    cx.refresh_windows();
}

/// The section, none without pins; the view style menu comes with 8.5's card.
pub fn section(
    lists: &Lists,
    now: i64,
    drawn: &RefCell<Vec<String>>,
    window: &mut Window,
    cx: &App,
) -> Option<AnyElement> {
    let inner = RefCell::new(Vec::new());
    let place = Place {
        inset: 0.0,
        section: "pinned",
        drawn: &inner,
    };
    if lists.pinned.is_empty() {
        return None;
    }
    let appearance = cx.global::<Appearance>();
    let translator = cx.global::<Translator>();
    let collapsed = text(cx.global::<Preferences>(), COLLAPSED_KEY) == Some("true");
    let (muted, hover) = (
        appearance.token("sidebar-list-muted"),
        appearance.token("sidebar-nav-text"),
    );
    let title = div()
        .id("pinned-title")
        .debug_selector(|| "pinned-title".into())
        .cursor_pointer()
        .text_size(css(text_size(14.0, cx)))
        .line_height(css(20.0))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(muted)
        .hover(|s| s.text_color(hover))
        .child(translator.t("ccAgent.sidebar.pinned", &[]))
        .on_click(move |_, _, cx| toggle(!collapsed, cx));
    let (chevron, label) = if collapsed {
        (IconName::ChevronRight, "ccAgent.sidebar.pinnedToggleExpand")
    } else {
        (
            IconName::ChevronDown,
            "ccAgent.sidebar.pinnedToggleCollapse",
        )
    };
    let button = div()
        .id("pinned-toggle")
        .flex()
        .flex_none()
        .size(css(20.0))
        .items_center()
        .justify_center()
        .rounded(css(6.0))
        .cursor_pointer()
        .text_color(muted)
        .hover(|s| s.text_color(hover))
        .opacity(0.0)
        .group_hover("pinned-header", |s| s.opacity(1.0))
        .tooltip(tip(translator.t(label, &[])))
        .child(icon(chevron.svg(), 13.0, 2.0))
        .on_click(move |_, _, cx| toggle(!collapsed, cx));
    let header = div()
        .group("pinned-header")
        .flex()
        .h(css(24.0))
        .items_center()
        .pr(css(12.0))
        .pl(css(24.0))
        .child(
            div()
                .flex()
                .min_w_0()
                .items_center()
                .gap(css(4.0))
                .child(title)
                .child(button),
        );
    let mut height = 6.0;
    let mut rows: Vec<AnyElement> = Vec::new();
    for (i, entry) in lists.pinned.iter().enumerate() {
        let (element, h) = match entry {
            Pinned::Session(s) => {
                let label = lists.labels.get(&s.id).map(String::as_str);
                (row(s, false, label, now, cx).into_any_element(), ROW_HEIGHT)
            }
            Pinned::Project(p) => project(p, &place, window, cx),
        };
        height += h + if i > 0 { 2.0 } else { 0.0 };
        rows.push(element);
    }
    let body = move || {
        div()
            .flex()
            .flex_col()
            .gap(css(2.0))
            .pl(css(12.0))
            .pt(css(6.0))
            .children(rows)
            .into_any_element()
    };
    let body = fold::section(FOLD, collapsed, height, body, window, cx);
    if body.is_some() {
        drawn.borrow_mut().extend(inner.into_inner());
    }
    Some(
        div()
            .flex()
            .flex_col()
            .w_full()
            .child(header)
            .children(body)
            .into_any_element(),
    )
}
