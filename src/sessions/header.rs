//! `MainListScopeHeader.tsx` and `SidebarFilterPopover.tsx`: the scope title and the menu that
//! edits the filter. Rows carry no summary and a choice closes the menu, as Ely's `Menu` does;
//! the machine switcher comes with chapter 19.

use ely_gpui_component::{
    menus::{Menu, MenuItem, menu_under},
    primitives::IconName,
};
use gpui::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _,
};

use super::filter::{
    self, Filter, GROUP_BY, GROUP_DIALOGUE, LAST_ACTIVITY, PROJECT_ORDER, SORT_BY, STATUS, VENDOR,
};
use super::meta;
use super::view_mode::{self, Main};
use crate::{
    appearance_settings::text_size, i18n::Translator, lucide::icon, preferences::Preferences,
    theme::Appearance, tip::tip, units::css,
};

type Choice = (&'static str, &'static str, IconName);

const STATUSES: [Choice; 3] = [
    (
        "active",
        "ccAgent.sidebar.taskStatus.active",
        IconName::CircleDot,
    ),
    (
        "archived",
        "ccAgent.sidebar.taskStatus.archived",
        IconName::Archive,
    ),
    ("all", "ccAgent.sidebar.taskStatus.all", IconName::Layers),
];
const VENDORS: [Choice; 4] = [
    ("all", "ccAgent.sidebar.filterVendor.all", IconName::Layers),
    ("cc", "ccAgent.sidebar.filterVendor.cc", IconName::Terminal),
    (
        "codex",
        "ccAgent.sidebar.filterVendor.codex",
        IconName::Terminal,
    ),
    ("pi", "ccAgent.sidebar.filterVendor.pi", IconName::Terminal),
];
const LAST_ACTIVITIES: [Choice; 5] = [
    (
        "1d",
        "ccAgent.sidebar.filterLastActivity.1d",
        IconName::CalendarDays,
    ),
    (
        "3d",
        "ccAgent.sidebar.filterLastActivity.3d",
        IconName::CalendarDays,
    ),
    (
        "7d",
        "ccAgent.sidebar.filterLastActivity.7d",
        IconName::CalendarDays,
    ),
    (
        "30d",
        "ccAgent.sidebar.filterLastActivity.30d",
        IconName::CalendarDays,
    ),
    (
        "all",
        "ccAgent.sidebar.filterLastActivity.all",
        IconName::Layers,
    ),
];
const SORTS: [Choice; 3] = [
    (
        "priority",
        "ccAgent.sidebar.sortTaskBy.priority",
        IconName::ArrowDownWideNarrow,
    ),
    (
        "recency",
        "ccAgent.sidebar.sortTaskBy.activity",
        IconName::Clock,
    ),
    (
        "created",
        "ccAgent.sidebar.sortTaskBy.created",
        IconName::Calendar,
    ),
];
const PROJECT_ORDERS: [Choice; 2] = [
    (
        "activity",
        "ccAgent.sidebar.filterProjectOrder.activity",
        IconName::ListOrdered,
    ),
    (
        "custom",
        "ccAgent.sidebar.filterProjectOrder.custom",
        IconName::GripVertical,
    ),
];

/// A submenu of radios that writes `key`.
fn choose(t: &Translator, key: &'static str, choices: &[Choice], current: &str) -> Menu {
    choices
        .iter()
        .fold(Menu::new(), |menu, &(value, label, glyph)| {
            menu.item(
                MenuItem::radio(t.t(label, &[]), value == current)
                    .icon(glyph)
                    .on_click(move |_, cx| filter::set(key, value, cx)),
            )
        })
}

/// `MAIN_VIEW_OPTIONS`; Ely has no `LayoutList`, so `List` stands in.
fn main_icon(mode: Main) -> IconName {
    match mode {
        Main::Text => IconName::AlignJustify,
        Main::List => IconName::List,
    }
}

/// The Display row: its icon is the current mode's.
fn display(current: Main, t: &Translator) -> MenuItem {
    let modes = Main::ALL.iter().fold(Menu::new(), |menu, &mode| {
        let label = match mode {
            Main::Text => "ccAgent.sidebar.viewStyleList",
            Main::List => "ccAgent.sidebar.viewStyleListWide",
        };
        menu.item(
            MenuItem::radio(t.t(label, &[]), mode == current)
                .icon(main_icon(mode))
                .on_click(move |_, cx| view_mode::set_main(mode, cx)),
        )
    });
    MenuItem::submenu(t.t("ccAgent.sidebar.displayHeading", &[]), modes).icon(main_icon(current))
}

/// The menu as `SidebarFilterPopover` lays it out, without the project and creator filters
/// (13.0).
pub fn menu(f: &Filter, main: Main, info: &[&'static str], t: &Translator) -> Menu {
    let (flat, group_dialogue) = (f.flat, f.group_dialogue);
    let group_by = Menu::new()
        .item(
            MenuItem::check(t.t("ccAgent.sidebar.filterGroupBy.project", &[]), !flat)
                .icon(IconName::Folder)
                .on_click(move |_, cx| {
                    filter::set(GROUP_BY, if flat { "project" } else { "flat" }, cx)
                }),
        )
        .item(
            MenuItem::check(
                t.t("ccAgent.sidebar.filterGroupBy.dialogue", &[]),
                group_dialogue,
            )
            .icon(IconName::MessageSquare)
            .on_click(move |_, cx| {
                filter::set(
                    GROUP_DIALOGUE,
                    if group_dialogue { "false" } else { "true" },
                    cx,
                )
            }),
        );
    let narrow = Menu::new()
        .item(
            MenuItem::submenu(
                t.t("ccAgent.sidebar.filterAgentHeading", &[]),
                choose(t, VENDOR, &VENDORS, f.vendor),
            )
            .icon(IconName::Bot),
        )
        .item(
            MenuItem::submenu(
                t.t("ccAgent.sidebar.filterLastActivityHeading", &[]),
                choose(t, LAST_ACTIVITY, &LAST_ACTIVITIES, f.last_activity),
            )
            .icon(IconName::CalendarDays),
        )
        .separator()
        .item(
            MenuItem::new(t.t("ccAgent.sidebar.filterReset", &[]))
                .icon(IconName::RotateCcw)
                .disabled(f.active_count() == 0)
                .on_click(|_, cx| filter::reset(cx)),
        );
    let mut menu = Menu::new()
        .item(
            MenuItem::submenu(t.t("ccAgent.sidebar.filterGroupByHeading", &[]), group_by)
                .icon(IconName::Rows2),
        )
        .item(
            MenuItem::submenu(
                t.t("ccAgent.sidebar.filterTaskSortHeading", &[]),
                choose(t, SORT_BY, &SORTS, f.sort_by),
            )
            .icon(IconName::ArrowDownWideNarrow),
        );
    if !flat {
        menu = menu.item(
            MenuItem::submenu(
                t.t("ccAgent.sidebar.filterProjectOrderHeading", &[]),
                choose(t, PROJECT_ORDER, &PROJECT_ORDERS, f.project_order),
            )
            .icon(IconName::ListOrdered),
        );
    }
    menu.separator()
        .item(
            MenuItem::submenu(
                t.t("ccAgent.sidebar.taskStatusHeading", &[]),
                choose(t, STATUS, &STATUSES, f.status),
            )
            .icon(IconName::ListChecks),
        )
        .item(
            MenuItem::submenu(t.t("ccAgent.sidebar.filterHeading", &[]), narrow)
                .icon(IconName::Filter),
        )
        .separator()
        .item(display(main, t))
        .item(
            MenuItem::submenu(
                t.t("ccAgent.sidebar.taskInfoHeading", &[]),
                task_info(info, t),
            )
            .icon(IconName::Info),
        )
}

/// `TASK_INFO_OPTIONS`: checks in a fixed order; ticking appends, so the row shows them in
/// the order they were ticked.
fn task_info(ticked: &[&'static str], t: &Translator) -> Menu {
    let glyph = |field| match field {
        "time" => IconName::Clock,
        "pr" => IconName::GitPullRequest,
        "worktree" => IconName::Folder,
        "tokens" => IconName::Hash,
        _ => IconName::Wallet,
    };
    meta::FIELDS.iter().fold(Menu::new(), |menu, &field| {
        menu.item(
            MenuItem::check(
                t.t(&format!("ccAgent.sidebar.taskInfo.{field}"), &[]),
                ticked.contains(&field),
            )
            .icon(glyph(field))
            .on_click(move |_, cx| meta::toggle(field, cx)),
        )
    })
}

/// The menu for the saved filter, view mode and task info.
pub fn current(cx: &App) -> Menu {
    let f = Filter::read(cx.global::<Preferences>());
    let info = &cx.global::<super::Sessions>().task_info;
    menu(&f, view_mode::main(cx), info, cx.global::<Translator>())
}

/// `group/sidebar-header flex h-6 justify-between pl-6`: the scope title, and the settings
/// button that shows on hover, or always with a dot while archived tasks are listed.
pub fn header(window: &mut Window, cx: &mut App) -> AnyElement {
    let f = Filter::read(cx.global::<Preferences>());
    let translator = cx.global::<Translator>();
    let menu = current(cx);
    let (title, label) = (
        translator.t("ccAgent.sidebar.allSessions", &[]),
        translator.t("ccAgent.sidebar.organizeSidebar", &[]),
    );
    let appearance = cx.global::<Appearance>();
    let (muted, hover) = (
        appearance.token("sidebar-list-muted"),
        appearance.token("sidebar-nav-text"),
    );
    let accent = appearance.token("status-bar-accent");
    let archived = f.status == "archived";
    let title = div()
        .min_w_0()
        .truncate()
        .text_size(css(text_size(14.0, cx)))
        .line_height(css(20.0))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(muted)
        .child(title);
    let button = menu_under(
        "sidebar-filter".into(),
        &menu,
        move |click| {
            div()
                .id("sidebar-filter-button")
                .debug_selector(|| "sidebar-filter-button".into())
                .relative()
                .flex()
                .size(css(28.0))
                .items_center()
                .justify_center()
                .rounded(css(6.0))
                .cursor_pointer()
                .text_color(muted)
                .hover(|s| s.text_color(hover))
                .tooltip(tip(label))
                .group("sidebar-filter-button")
                .child(
                    icon(IconName::SlidersHorizontal.svg(), 14.0, 2.0)
                        .text_color(muted)
                        .group_hover("sidebar-filter-button", |s| s.text_color(hover)),
                )
                .when(archived, |el| {
                    el.child(
                        div()
                            .absolute()
                            .top(css(2.0))
                            .right(css(2.0))
                            .size(css(6.0))
                            .rounded_full()
                            .bg(accent),
                    )
                })
                .when(!archived, |el| {
                    el.opacity(0.0)
                        .group_hover("scope-header", |s| s.opacity(1.0))
                })
                .on_click(move |event, window, cx| click(event, window, cx))
                .into_any_element()
        },
        window,
        cx,
    );
    div()
        .group("scope-header")
        .flex()
        .h(css(24.0))
        .items_center()
        .justify_between()
        .pl(css(24.0))
        .child(title)
        .child(button)
        .into_any_element()
}
