//! `SearchFilterMenu`'s options and its trigger in the Search row.

use ely_gpui_component::{
    menus::{Menu, MenuItem, menu_under},
    primitives::IconName,
};
use gpui::{
    AnyElement, App, FocusHandle, InteractiveElement as _, IntoElement as _, ParentElement as _,
    Role, StatefulInteractiveElement as _, Styled as _, Toggled, Window, div,
    prelude::FluentBuilder as _,
};

use super::{Search, projects};
use crate::{
    i18n::{Arg, Translator},
    lucide::icon,
    preferences::Preferences,
    theme::Appearance,
    units::css,
};

/// `SEARCH_SORT_BY_KEY`.
pub const SORT_KEY: &str = "cc-agent.search.sortBy";

type Choice = (&'static str, &'static str);

const SORTS: [Choice; 3] = [
    ("relevance", "ccAgent.search.sort.relevance"),
    ("activityDesc", "ccAgent.search.sort.activityDesc"),
    ("activityAsc", "ccAgent.search.sort.activityAsc"),
];
const STATUSES: [Choice; 3] = [
    ("active", "ccAgent.sidebar.filterStatus.active"),
    ("archived", "ccAgent.sidebar.filterStatus.archived"),
    ("all", "ccAgent.sidebar.filterStatus.all"),
];
const AGENTS: [Choice; 4] = [
    ("all", "ccAgent.sidebar.filterVendor.all"),
    ("cc", "ccAgent.sidebar.filterVendor.cc"),
    ("codex", "ccAgent.sidebar.filterVendor.codex"),
    ("pi", "ccAgent.sidebar.filterVendor.pi"),
];
const ACTIVITIES: [Choice; 5] = [
    ("1d", "ccAgent.sidebar.filterLastActivity.1d"),
    ("3d", "ccAgent.sidebar.filterLastActivity.3d"),
    ("7d", "ccAgent.sidebar.filterLastActivity.7d"),
    ("30d", "ccAgent.sidebar.filterLastActivity.30d"),
    ("all", "ccAgent.sidebar.filterLastActivity.all"),
];

/// The sort shared by every search, and the filters each session starts at `all`.
pub struct Options {
    pub sort: &'static str,
    pub status: &'static str,
    pub agent: &'static str,
    pub last_activity: &'static str,
    /// `projectSelection`; none is `all`.
    pub projects: Option<Vec<String>>,
}

impl Options {
    /// `loadSearchSortBy`: an unknown or missing value is `relevance`.
    pub fn load(prefs: &Preferences) -> Self {
        let raw = match prefs.text(SORT_KEY) {
            Ok(raw) => raw,
            Err(error) => {
                log::warn!(target: "search", "sortBy unread: {error:#}");
                None
            }
        };
        let sort = SORTS
            .iter()
            .find(|(v, _)| Some(*v) == raw)
            .map_or("relevance", |c| c.0);
        Options {
            sort,
            status: "all",
            agent: "all",
            last_activity: "all",
            projects: None,
        }
    }

    /// `activeFilterCount`; the sort never counts.
    pub fn active_count(&self) -> usize {
        let filters = [self.status, self.agent, self.last_activity];
        filters.iter().filter(|v| **v != "all").count() + usize::from(self.projects.is_some())
    }
}

fn options(cx: &mut App) -> &mut Options {
    &mut cx.global_mut::<Search>().options
}

/// A row chosen: the press was the menu's.
fn chosen(cx: &mut App) {
    cx.global_mut::<Search>().chosen = true;
}

/// `setSearchSortBy`: the choice is written, as `persistSearchSortBy` writes it.
fn set_sort(value: &'static str, cx: &mut App) {
    if options(cx).sort == value {
        return;
    }
    options(cx).sort = value;
    log::info!(target: "search", "sort {value}");
    if let Err(error) = cx.global_mut::<Preferences>().set(SORT_KEY, value) {
        log::error!(target: "search", "sortBy not saved: {error:#}");
    }
    super::run::sync(cx);
    cx.refresh_windows();
}

fn set(field: fn(&mut Options) -> &mut &'static str, value: &'static str, cx: &mut App) {
    *field(options(cx)) = value;
    log::info!(target: "search", "filter {value}");
    super::run::sync(cx);
    cx.refresh_windows();
}

/// `resetFilters`.
fn reset(cx: &mut App) {
    let o = options(cx);
    (o.status, o.agent, o.last_activity) = ("all", "all", "all");
    o.projects = None;
    log::info!(target: "search", "filters reset");
    super::run::sync(cx);
    cx.refresh_windows();
}

fn set_projects(value: Option<Vec<String>>, cx: &mut App) {
    log::info!(target: "search", "project filter {:?}", value.as_ref().map(Vec::len));
    options(cx).projects = value;
    super::run::sync(cx);
    cx.refresh_windows();
}

/// `projectValue`.
fn projects_label(o: &Options, t: &Translator) -> String {
    match &o.projects {
        None => t.t("ccAgent.search.filter.allProjects", &[]),
        Some(keys) => t.t(
            "ccAgent.sidebar.filterSelectedProjects",
            &[("count", Arg::Number(keys.len() as i64))],
        ),
    }
}

/// The Projects submenu; the task count joins the label.
fn projects_menu(o: &Options, known: &[crate::sessions::group::Project], t: &Translator) -> Menu {
    let all = MenuItem::check(
        t.t("ccAgent.search.filter.allProjects", &[]),
        o.projects.is_none(),
    )
    .on_click(|_, cx| {
        chosen(cx);
        set_projects(None, cx)
    });
    let menu = Menu::new().item(all);
    let menu = match known.is_empty() {
        true => menu,
        false => menu.separator(),
    };
    known.iter().fold(menu, |menu, p| {
        let key = p.key.clone();
        let text = format!("{} · {}", p.name, p.sessions.len());
        let item = MenuItem::check(text, projects::selected(o.projects.as_deref(), &p.key))
            .on_click(move |_, cx| {
                chosen(cx);
                let next = projects::next(options(cx).projects.as_deref(), &key);
                set_projects(next, cx)
            });
        menu.item(match p.key.starts_with("remote:") {
            true => item.icon(IconName::Globe),
            false => item,
        })
    })
}

fn label(choices: &[Choice], value: &str, t: &Translator) -> String {
    let key = choices.iter().find(|c| c.0 == value).map_or("", |c| c.1);
    t.t(key, &[])
}

/// A submenu of `SelectMenuItem`s.
fn radios(
    t: &Translator,
    choices: &'static [Choice],
    current: &str,
    pick: impl Fn(&'static str, &mut App) + Copy + 'static,
) -> Menu {
    choices.iter().fold(Menu::new(), |menu, &(value, key)| {
        menu.item(
            MenuItem::radio(t.t(key, &[]), value == current).on_click(move |_, cx| {
                chosen(cx);
                pick(value, cx)
            }),
        )
    })
}

fn menu(o: &Options, known: &[crate::sessions::group::Project], t: &Translator) -> Menu {
    let reset_row = (o.active_count() > 0).then(|| {
        MenuItem::new(t.t("ccAgent.search.filter.reset", &[])).on_click(|_, cx| {
            chosen(cx);
            reset(cx)
        })
    });
    let sub = |heading: &str, menu: Menu| MenuItem::submenu(t.t(heading, &[]), menu);
    Menu::new()
        .group(t.t("ccAgent.search.filter.label", &[]), reset_row)
        .item(sub(
            "ccAgent.sidebar.filterSortByHeading",
            radios(t, &SORTS, o.sort, set_sort),
        ))
        .item(sub(
            "ccAgent.sidebar.filterStatusHeading",
            radios(t, &STATUSES, o.status, |v, cx| {
                set(|o| &mut o.status, v, cx)
            }),
        ))
        .item(sub(
            "ccAgent.sidebar.filterProjectsHeading",
            projects_menu(o, known, t),
        ))
        .item(sub(
            "ccAgent.sidebar.filterAgentHeading",
            radios(t, &AGENTS, o.agent, |v, cx| set(|o| &mut o.agent, v, cx)),
        ))
        .item(sub(
            "ccAgent.sidebar.filterLastActivityHeading",
            radios(t, &ACTIVITIES, o.last_activity, |v, cx| {
                set(|o| &mut o.last_activity, v, cx)
            }),
        ))
}

/// The compact trigger (`SEARCH_TOOL_ICON_CLASS`) and its menu; `focus` is its Tab stop.
pub fn button(focus: &FocusHandle, window: &mut Window, cx: &mut App) -> AnyElement {
    let t = cx.global::<Translator>();
    let o = &cx.global::<Search>().options;
    let active = o.active_count();
    let values = [
        label(&SORTS, o.sort, t),
        label(&STATUSES, o.status, t),
        label(&AGENTS, o.agent, t),
        label(&ACTIVITIES, o.last_activity, t),
        projects_label(o, t),
    ];
    let names = ["sort", "status", "agent", "lastActivity", "projects"];
    let args: Vec<_> = names
        .iter()
        .zip(&values)
        .map(|(n, v)| (*n, Arg::Text(v)))
        .collect();
    let aria = t.t("ccAgent.search.filterAria", &args);
    let menu = menu(o, projects::known_of(cx), t);
    let appearance = cx.global::<Appearance>();
    let (tertiary, strong) = (
        appearance.token("text-tertiary"),
        appearance.token("text-primary"),
    );
    let hover = appearance.hsl_token("sidebar-item-hover");
    let accent = appearance.token("status-bar-accent");
    let focus = focus.clone();
    menu_under(
        "search-filter".into(),
        &menu,
        move |click| {
            div()
                .id("search-filter-button")
                .debug_selector(|| "search-filter-button".into())
                .track_focus(&focus)
                .role(Role::Button)
                .aria_label(aria)
                .aria_toggled(if active > 0 {
                    Toggled::True
                } else {
                    Toggled::False
                })
                .group("search-filter-button")
                .relative()
                .flex()
                .flex_none()
                .size(css(24.0))
                .items_center()
                .justify_center()
                .rounded(css(9999.0))
                .cursor_pointer()
                .hover(|s| s.bg(hover))
                .child(
                    icon(IconName::SlidersHorizontal.svg(), 14.0, 2.0)
                        .text_color(if active > 0 { strong } else { tertiary })
                        .group_hover("search-filter-button", |s| s.text_color(strong)),
                )
                // `FilterReminderDot`.
                .when(active > 0, |el| {
                    el.child(
                        div()
                            .debug_selector(|| "search-filter-dot".into())
                            .absolute()
                            .top(css(2.0))
                            .right(css(2.0))
                            .size(css(6.0))
                            .rounded(css(9999.0))
                            .bg(accent),
                    )
                })
                .on_click(move |event, window, cx| click(event, window, cx))
                .into_any_element()
        },
        window,
        cx,
    )
    .into_any_element()
}

#[cfg(test)]
mod tests;
