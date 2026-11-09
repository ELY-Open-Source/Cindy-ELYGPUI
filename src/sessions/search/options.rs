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

use super::{
    Search, Which,
    lock::{self, Lock},
    of, of_mut, projects, run,
};
use crate::{
    i18n::{Arg, Translator},
    lucide::icon,
    preferences::Preferences,
    sessions::group::Project,
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

/// A search's filters, each starting at `all`; the sort is shared, on `Search`.
pub struct Options {
    pub status: &'static str,
    pub agent: &'static str,
    pub last_activity: &'static str,
    /// `projectSelection`; none is `all`.
    pub projects: Option<Vec<String>>,
    pub lock: Option<Lock>,
}

/// `loadSearchSortBy`: an unknown or missing value is `relevance`.
pub fn load_sort(prefs: &Preferences) -> &'static str {
    let raw = match prefs.text(SORT_KEY) {
        Ok(raw) => raw,
        Err(error) => {
            log::warn!(target: "search", "sortBy unread: {error:#}");
            None
        }
    };
    SORTS
        .iter()
        .find(|(v, _)| Some(*v) == raw)
        .map_or("relevance", |c| c.0)
}

impl Default for Options {
    fn default() -> Self {
        Options {
            status: "all",
            agent: "all",
            last_activity: "all",
            projects: None,
            lock: None,
        }
    }
}

impl Options {
    /// `activeFilterCount`; the sort never counts.
    pub fn active_count(&self) -> usize {
        let filters = [self.status, self.agent, self.last_activity];
        let group = matches!(self.lock, Some(Lock::Group { .. }));
        filters.iter().filter(|v| **v != "all").count()
            + usize::from(self.projects.is_some() || group)
    }
}

fn options(w: Which, cx: &mut App) -> &mut Options {
    &mut of_mut(w, cx).options
}

/// A row chosen: the press was the menu's.
fn chosen(cx: &mut App) {
    cx.global_mut::<Search>().chosen = true;
}

/// `setSearchSortBy`: written as `persistSearchSortBy` writes it; both searches follow.
fn set_sort(value: &'static str, cx: &mut App) {
    if cx.global::<Search>().sort == value {
        return;
    }
    cx.global_mut::<Search>().sort = value;
    log::info!(target: "search", "sort {value}");
    if let Err(error) = cx.global_mut::<Preferences>().set(SORT_KEY, value) {
        log::error!(target: "search", "sortBy not saved: {error:#}");
    }
    run::sync(Which::Inline, cx);
    run::sync(Which::Rail, cx);
    cx.refresh_windows();
}

type Field = fn(&mut Options) -> &mut &'static str;

fn set(w: Which, field: Field, value: &'static str, cx: &mut App) {
    *field(options(w, cx)) = value;
    log::info!(target: "search", "{w:?} filter {value}");
    run::sync(w, cx);
    cx.refresh_windows();
}

/// `resetFilters`: a locked project stays selected.
fn reset(w: Which, cx: &mut App) {
    let o = options(w, cx);
    let lock = o.lock.take();
    *o = Options::default();
    if let Some(Lock::Project { key, .. }) = &lock {
        o.projects = Some(vec![key.clone()]);
    }
    o.lock = lock;
    log::info!(target: "search", "{w:?} filters reset");
    run::sync(w, cx);
    cx.refresh_windows();
}

fn set_projects(w: Which, value: Option<Vec<String>>, cx: &mut App) {
    log::info!(target: "search", "{w:?} project filter {:?}", value.as_ref().map(Vec::len));
    options(w, cx).projects = value;
    run::sync(w, cx);
    cx.refresh_windows();
}

/// `projectValue`.
fn projects_label(o: &Options, t: &Translator, cx: &App) -> String {
    if let Some(lock) = &o.lock {
        return lock::label(lock, false, cx);
    }
    match &o.projects {
        None => t.t("ccAgent.search.filter.allProjects", &[]),
        Some(keys) => t.t(
            "ccAgent.sidebar.filterSelectedProjects",
            &[("count", Arg::Number(keys.len() as i64))],
        ),
    }
}

/// The Projects submenu; the task count joins the label.
fn projects_menu(w: Which, o: &Options, known: &[Project], t: &Translator) -> Menu {
    let locked = o.lock.is_some();
    let all = MenuItem::check(
        t.t("ccAgent.search.filter.allProjects", &[]),
        o.projects.is_none(),
    )
    .disabled(locked)
    .on_click(move |_, cx| {
        chosen(cx);
        set_projects(w, None, cx)
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
            .disabled(locked)
            .on_click(move |_, cx| {
                chosen(cx);
                let next = projects::next(options(w, cx).projects.as_deref(), &key);
                set_projects(w, next, cx)
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

fn menu(w: Which, o: &Options, sort: &str, known: &[Project], t: &Translator) -> Menu {
    let reset_row = (o.active_count() > 0).then(|| {
        MenuItem::new(t.t("ccAgent.search.filter.reset", &[])).on_click(move |_, cx| {
            chosen(cx);
            reset(w, cx)
        })
    });
    let sub = |heading: &str, menu: Menu| MenuItem::submenu(t.t(heading, &[]), menu);
    Menu::new()
        .group(t.t("ccAgent.search.filter.label", &[]), reset_row)
        .item(sub(
            "ccAgent.sidebar.filterSortByHeading",
            radios(t, &SORTS, sort, set_sort),
        ))
        .item(sub(
            "ccAgent.sidebar.filterStatusHeading",
            radios(t, &STATUSES, o.status, move |v, cx| {
                set(w, |o| &mut o.status, v, cx)
            }),
        ))
        .item(sub(
            "ccAgent.sidebar.filterProjectsHeading",
            projects_menu(w, o, known, t),
        ))
        .item(sub(
            "ccAgent.sidebar.filterAgentHeading",
            radios(t, &AGENTS, o.agent, move |v, cx| {
                set(w, |o| &mut o.agent, v, cx)
            }),
        ))
        .item(sub(
            "ccAgent.sidebar.filterLastActivityHeading",
            radios(t, &ACTIVITIES, o.last_activity, move |v, cx| {
                set(w, |o| &mut o.last_activity, v, cx)
            }),
        ))
}

/// The compact trigger (`SEARCH_TOOL_ICON_CLASS`) and its menu; `focus` is its Tab stop.
pub fn button(w: Which, focus: &FocusHandle, window: &mut Window, cx: &mut App) -> AnyElement {
    let t = cx.global::<Translator>();
    let (o, sort) = (&of(w, cx).options, cx.global::<Search>().sort);
    let active = o.active_count();
    let values = [
        label(&SORTS, sort, t),
        label(&STATUSES, o.status, t),
        label(&AGENTS, o.agent, t),
        label(&ACTIVITIES, o.last_activity, t),
        projects_label(o, t, cx),
    ];
    let names = ["sort", "status", "agent", "lastActivity", "projects"];
    let args: Vec<_> = names
        .iter()
        .zip(&values)
        .map(|(n, v)| (*n, Arg::Text(v)))
        .collect();
    let aria = t.t("ccAgent.search.filterAria", &args);
    let menu = menu(w, o, sort, projects::known_of(cx), t);
    let appearance = cx.global::<Appearance>();
    let (tertiary, strong) = (
        appearance.token("text-tertiary"),
        appearance.token("text-primary"),
    );
    let hover = appearance.hsl_token("sidebar-item-hover");
    let accent = appearance.token("status-bar-accent");
    let focus = focus.clone();
    if w == Which::Rail {
        let text = match active {
            0 => t.t("ccAgent.search.filter.label", &[]),
            n => t.t(
                "ccAgent.search.filter.active",
                &[("count", Arg::Number(n as i64))],
            ),
        };
        let look = labelled(text, aria, active > 0, &focus, cx);
        let menu = menu_under(
            "rail-search-filter".into(),
            &menu,
            move |click| {
                look.on_click(move |event, window, cx| click(event, window, cx))
                    .into_any_element()
            },
            window,
            cx,
        );
        return menu.into_any_element();
    }
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

/// `SEARCH_TOOL_BUTTON_CLASS` with its label and chevron, the popover's trigger.
fn labelled(
    text: String,
    aria: String,
    active: bool,
    focus: &FocusHandle,
    cx: &App,
) -> gpui::Stateful<gpui::Div> {
    let appearance = cx.global::<Appearance>();
    let size = crate::appearance_settings::text_size(12.0, cx);
    let (hover, strong, secondary) = (
        appearance.token("cmd-palette-item-hover"),
        appearance.token("text-primary"),
        appearance.token("text-secondary"),
    );
    let group = "rail-search-filter-button";
    div()
        .id(group)
        .group(group)
        .debug_selector(|| "rail-search-filter-button".into())
        .track_focus(focus)
        .role(Role::Button)
        .aria_label(aria)
        .aria_toggled(if active {
            Toggled::True
        } else {
            Toggled::False
        })
        .flex()
        .h(css(28.0))
        .min_w_0()
        .items_center()
        .gap(css(6.0))
        .rounded(css(9999.0))
        .border(css(1.0))
        .border_color(appearance.token("border-default"))
        .bg(match active {
            true => appearance.token("surface-chip"),
            false => appearance.token("surface-elevated"),
        })
        .px(css(10.0))
        .cursor_pointer()
        .text_size(css(size))
        .text_color(secondary)
        .hover(|s| s.bg(hover).text_color(strong))
        .child(
            icon(IconName::SlidersHorizontal.svg(), 13.0, 2.0)
                .text_color(secondary)
                .group_hover(group, |s| s.text_color(strong)),
        )
        .child(div().truncate().child(text))
        .child(
            icon(IconName::ChevronDown.svg(), 13.0, 2.0)
                .text_color(appearance.token("cmd-palette-item-meta")),
        )
}

#[cfg(test)]
mod tests;
