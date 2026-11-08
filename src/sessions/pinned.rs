//! `PinnedSection.tsx` in text and list mode: pinned tasks and projects, folded as one
//! section, with the `ViewStyleMenu` that picks the section's display mode.

use std::cell::RefCell;

use ely_gpui_component::{
    menus::{Menu, MenuItem, menu_under},
    primitives::IconName,
};
use gpui::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement as _, Role,
    StatefulInteractiveElement as _, Styled as _, Window, div,
};

use super::{
    fold,
    list_row::{self, list_row},
    lists::{Lists, Pinned},
    main_list::{Place, project, text},
    row::{ROW_HEIGHT, row},
    view_mode::{self, Pinned as Mode},
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

/// `VIEW_STYLE_OPTIONS`; Ely has no `LayoutList`, so `List` stands in.
fn style(mode: Mode) -> (&'static str, IconName) {
    match mode {
        Mode::Text => ("ccAgent.sidebar.viewStyleList", IconName::AlignJustify),
        Mode::Card => ("ccAgent.sidebar.viewStyleCard", IconName::LayoutGrid),
        Mode::List => ("ccAgent.sidebar.viewStyleListWide", IconName::List),
    }
}

/// The header's right-hand trigger, showing the current mode's icon, and its menu.
pub fn view_style(window: &mut Window, cx: &mut App) -> AnyElement {
    let current = view_mode::pinned(cx);
    let translator = cx.global::<Translator>();
    let menu = Mode::ALL.iter().fold(Menu::new(), |menu, &mode| {
        let (label, glyph) = style(mode);
        menu.item(
            MenuItem::radio(translator.t(label, &[]), mode == current)
                .icon(glyph)
                .on_click(move |_, cx| view_mode::set_pinned(mode, cx)),
        )
    });
    let label = translator.t("ccAgent.sidebar.viewStyle", &[]);
    let appearance = cx.global::<Appearance>();
    let (muted, hover) = (
        appearance.token("sidebar-list-muted"),
        appearance.token("sidebar-nav-text"),
    );
    let glyph = style(current).1;
    let focus = window
        .use_keyed_state("pinned-view-style-focus", cx, |_, cx| {
            cx.focus_handle().tab_stop(true)
        })
        .read(cx)
        .clone();
    let focused = focus.is_focused(window);
    menu_under(
        "pinned-view-style".into(),
        &menu,
        move |click| {
            div()
                .id("pinned-view-style-button")
                .debug_selector(|| "pinned-view-style-button".into())
                .track_focus(&focus)
                .role(Role::Button)
                .aria_label(label.clone())
                .flex()
                .flex_none()
                .size(css(20.0))
                .items_center()
                .justify_center()
                .rounded(css(6.0))
                .cursor_pointer()
                .opacity(if focused { 1.0 } else { 0.0 })
                .group_hover("pinned-header", |s| s.opacity(1.0))
                .tooltip(tip(label))
                .group("pinned-view-style")
                .child(
                    icon(glyph.svg(), 13.0, 2.0)
                        .text_color(muted)
                        .group_hover("pinned-view-style", |s| s.text_color(hover)),
                )
                .on_click(move |event, window, cx| click(event, window, cx))
                .into_any_element()
        },
        window,
        cx,
    )
    .into_any_element()
}

/// The section, none without pins.
pub fn section(
    lists: &Lists,
    view_style: AnyElement,
    now: i64,
    drawn: &RefCell<Vec<String>>,
    window: &mut Window,
    cx: &App,
) -> Option<AnyElement> {
    let inner = RefCell::new(Vec::new());
    let list = view_mode::pinned(cx) == Mode::List;
    let place = Place {
        list,
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
        .group("pinned-toggle")
        .child(
            icon(chevron.svg(), 13.0, 2.0)
                .text_color(muted)
                .group_hover("pinned-toggle", |s| s.text_color(hover)),
        )
        .on_click(move |_, _, cx| toggle(!collapsed, cx));
    let header = div()
        .group("pinned-header")
        .flex()
        .h(css(24.0))
        .items_center()
        .justify_between()
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
        )
        .child(view_style);
    let top = if list { 4.0 } else { 6.0 };
    let mut height = top;
    let mut rows: Vec<AnyElement> = Vec::new();
    for (i, entry) in lists.pinned.iter().enumerate() {
        let label = |s: &super::group::Session| lists.labels.get(&s.id).map(String::as_str);
        let (element, h) = match entry {
            Pinned::Session(s) if list => {
                (list_row(s, i == 0, label(s), now, cx), list_row::height(cx))
            }
            Pinned::Session(s) => (row(s, false, label(s), now, cx), ROW_HEIGHT),
            // `border-b border-[var(--cmd-palette-border)] py-0.5`.
            Pinned::Project(p) if list => {
                let (element, h) = project(p, &place, window, cx);
                let framed = div()
                    .py(css(2.0))
                    .border_b(css(1.0))
                    .border_color(appearance.token("cmd-palette-border"))
                    .child(element)
                    .into_any_element();
                (framed, h + 5.0)
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
            .pt(css(top))
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
