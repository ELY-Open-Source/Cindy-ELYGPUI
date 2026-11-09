//! `PinnedSection.tsx` in text, list and card mode: pinned tasks and projects, folded as one
//! section, with the `ViewStyleMenu` that picks the section's display mode.

use std::{cell::RefCell, rc::Rc};

use ely_gpui_component::{
    menus::{Menu, MenuItem, menu_under},
    primitives::IconName,
};
use gpui::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement as _, Role,
    StatefulInteractiveElement as _, Styled as _, Window, div,
};

use super::{
    Sessions,
    card::Card,
    fold,
    list_row::{self, list_row},
    lists::{Lists, Pinned},
    main_list::{Place, project, text},
    masonry, reorder,
    row::{ROW_HEIGHT, row},
    select, sortable,
    view_mode::{self, Pinned as Mode},
};
use crate::{
    appearance_settings::text_size, i18n::Translator, lucide::icon, preferences::Preferences,
    theme::Appearance, tip::tip, units::css,
};

const COLLAPSED_KEY: &str = "cc-agent.sidebar.pinnedSectionCollapsed";
const FOLD: &str = "pinned-section";
/// The section's sortable list.
const LIST: &str = "pinned-sortable";

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
/// A pinned task's source label.
fn label_of(entry: &Pinned, lists: &Lists) -> Option<String> {
    match entry {
        Pinned::Session(s) => lists.labels.get(&s.id).cloned(),
        Pinned::Project(_) => None,
    }
}

/// An entry in card mode: sessions as cards, projects framed.
fn card_element(
    entry: &Pinned,
    now: i64,
    place: &Place,
    window: &mut Window,
    cx: &App,
) -> AnyElement {
    match entry {
        Pinned::Session(s) => Card {
            session: (**s).clone(),
            now,
        }
        .into_any_element(),
        Pinned::Project(p) => {
            let appearance = cx.global::<Appearance>();
            div()
                .rounded(css(12.0))
                .border(css(1.0))
                .border_color(appearance.token("cmd-palette-border"))
                .bg(appearance.token("surface-elevated"))
                .p(css(4.0))
                .child(project(p, place, window, cx).0)
                .into_any_element()
        }
    }
}

/// An entry in text or list mode, with its drawn height.
fn entry_element(
    entry: &Pinned,
    first: bool,
    label: Option<&str>,
    now: i64,
    place: &Place,
    window: &mut Window,
    cx: &App,
) -> (AnyElement, f32) {
    match entry {
        Pinned::Session(s) if place.list => {
            (list_row(s, first, label, now, cx), list_row::height(cx))
        }
        Pinned::Session(s) => (row(s, false, label, now, cx), ROW_HEIGHT),
        // `border-b border-[var(--cmd-palette-border)] py-0.5`.
        Pinned::Project(p) if place.list => {
            let (element, h) = project(p, place, window, cx);
            let framed = div()
                .py(css(2.0))
                .border_b(css(1.0))
                .border_color(cx.global::<Appearance>().token("cmd-palette-border"))
                .child(element)
                .into_any_element();
            (framed, h + 5.0)
        }
        Pinned::Project(p) => project(p, place, window, cx),
    }
}

pub fn section(
    lists: &Lists,
    view_style: AnyElement,
    now: i64,
    drawn: &RefCell<Vec<String>>,
    window: &mut Window,
    cx: &App,
) -> Option<AnyElement> {
    let inner = RefCell::new(Vec::new());
    let mode = view_mode::pinned(cx);
    let list = mode == Mode::List;
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
    let top = match mode {
        Mode::Text => 6.0,
        Mode::List => 4.0,
        Mode::Card => 2.0,
    };
    let mut height = top;
    let card = mode == Mode::Card;
    let columns = if card { masonry::columns(cx) } else { 1 };
    let among = Rc::new(sortable::Among {
        list: LIST,
        ids: Rc::new(lists.pinned.iter().map(Pinned::id).collect()),
        columns,
    });
    let buckets = sortable::arrange(LIST, lists.pinned.clone(), columns, Pinned::id, cx);
    let editing = cx
        .global::<Sessions>()
        .editing
        .as_ref()
        .map(|e| e.id.clone());
    let mut shown: Vec<Vec<AnyElement>> = Vec::new();
    for bucket in &buckets {
        let mut column = Vec::new();
        let c = shown.len();
        for entry in bucket {
            let first = shown.is_empty() && column.is_empty();
            let label = label_of(entry, lists);
            let element = if card {
                // `rowMajorRank` for `getVisibleSidebarSessionIds`.
                div()
                    .relative()
                    .w_full()
                    .child(select::rank(Some(column.len() * columns + c)))
                    .child(card_element(entry, now, &place, window, cx))
                    .child(select::rank(None))
                    .into_any_element()
            } else {
                let (element, h) =
                    entry_element(entry, first, label.as_deref(), now, &place, window, cx);
                height += h + if first { 0.0 } else { 2.0 };
                element
            };
            let draw: sortable::Draw = {
                let entry = entry.clone();
                Rc::new(move |window, cx| {
                    let drawn = RefCell::new(Vec::new());
                    let place = Place {
                        list,
                        section: "pinned",
                        drawn: &drawn,
                    };
                    match card {
                        true => card_element(&entry, now, &place, window, cx),
                        false => {
                            entry_element(&entry, false, label.as_deref(), now, &place, window, cx)
                                .0
                        }
                    }
                })
            };
            let enabled = editing.as_deref() != Some(entry.id().as_str());
            let id = entry.id();
            column.push(sortable::item(
                &id,
                among.clone(),
                enabled,
                element,
                draw,
                cx,
            ));
        }
        shown.push(column);
    }
    // `CardMasonry`: its height is measured.
    let rows = match card {
        true => {
            height = top + cx.global::<super::Sessions>().masonry.height;
            vec![masonry::layout(shown, &inner, window, cx)]
        }
        false => shown.into_iter().flatten().collect(),
    };
    let on_reorder: sortable::OnReorder =
        Rc::new(|order, window, cx| reorder::reorder(order, window, cx));
    let body = move || {
        sortable::list(LIST, rows, on_reorder)
            .flex()
            .flex_col()
            .gap(css(2.0))
            .pl(css(12.0))
            .pt(css(top))
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
