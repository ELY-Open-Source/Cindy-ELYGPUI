//! `ProjectsSection.tsx`: project and dialogue groups and loose tasks, under the Pinned section.

use std::cell::RefCell;

use gpui::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement as _, ScrollHandle,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _,
};

use super::{
    Sessions,
    filter::Filter,
    fold::{self, ShowAll},
    group::{Entry, Order, Project, Session, aliases},
    list_row::{self, list_row},
    lists::{Lists, build},
    paths::PLATFORM,
    pinned,
    row::{ROW_HEIGHT, Scope, row},
    view_mode::{self, Main},
};
use ely_gpui_component::primitives::IconName;

use crate::{
    appearance_settings::text_size,
    i18n::Translator,
    lucide::{MESSAGES_SQUARE, icon},
    preferences::Preferences,
    theme::Appearance,
    units::css,
};

const DIALOGUE_COLLAPSED_KEY: &str = "cc-agent.sidebar.dialogueGroupCollapsed";
const SESSION_LIMIT_KEY: &str = "sidebar.collapse.projectSessionLimit";
const PROJECT_LIMIT_KEY: &str = "sidebar.collapse.projectLimit";
/// `DIALOGUE_GROUP_ALL_KEY`, the one dialogue group of an ungrouped list.
const DIALOGUE_ALL: &str = "all";
const DIALOGUE_FOLD: &str = "dialogue:all";

pub(super) fn text<'a>(prefs: &'a Preferences, key: &str) -> Option<&'a str> {
    prefs.text(key).ok().flatten()
}

/// `loadDialogueGroupCollapsedKeys`: `JSON.parse`'d string keys, as a set; legacy `true` is `all`.
fn dialogue_keys(prefs: &Preferences) -> Vec<String> {
    let raw = match text(prefs, DIALOGUE_COLLAPSED_KEY) {
        None | Some("false") => return Vec::new(),
        Some("true") => return vec![DIALOGUE_ALL.into()],
        Some(raw) => raw,
    };
    let Ok(serde_json::Value::Array(items)) = crate::cindy_json::roundtrip(raw) else {
        return Vec::new();
    };
    let mut keys: Vec<String> = Vec::new();
    for key in items.iter().filter_map(|k| k.as_str()) {
        if !keys.iter().any(|k| k == key) {
            keys.push(key.to_owned());
        }
    }
    keys
}

fn dialogue_collapsed(prefs: &Preferences) -> bool {
    dialogue_keys(prefs).iter().any(|k| k == DIALOGUE_ALL)
}

/// `persistDialogueGroupCollapsedKeys`, keeping other groups' keys; false when unsaved.
fn set_dialogue_collapsed(collapsed: bool, cx: &mut App) -> bool {
    let mut keys = dialogue_keys(cx.global::<Preferences>());
    keys.retain(|k| k != DIALOGUE_ALL);
    if collapsed {
        keys.push(DIALOGUE_ALL.into());
    }
    let value = serde_json::to_string(&keys).expect("strings serialize");
    if let Err(error) = cx
        .global_mut::<Preferences>()
        .set(DIALOGUE_COLLAPSED_KEY, &value)
    {
        log::error!(target: "sessions", "dialogue fold not saved: {error:#}");
        return false;
    }
    true
}

/// Where a group draws: in list rows (inset `pl-2`) or text rows, and the section that owns
/// its Show all, as each `SessionEntryList` owns its own.
pub(super) struct Place<'a> {
    pub list: bool,
    pub section: &'static str,
    /// Show all keys whose lists are drawn this frame; the rest reset.
    pub drawn: &'a RefCell<Vec<String>>,
}

/// `ProjectNode` and `SessionGroupNode`'s header: icon, name, hover chevron.
pub(super) fn header(
    id: SharedString,
    glyph: &[u8],
    name: String,
    collapsed: bool,
    cx: &App,
) -> gpui::Stateful<gpui::Div> {
    let appearance = cx.global::<Appearance>();
    let group = SharedString::from(format!("{id}-group"));
    let chevron = if collapsed {
        IconName::ChevronRight
    } else {
        IconName::ChevronDown
    };
    let muted = appearance.token("sidebar-list-muted");
    let selector = id.to_string();
    div()
        .id(id)
        .debug_selector(move || selector)
        .group(group.clone())
        .flex()
        .items_center()
        .gap(css(10.0))
        .h(css(32.0))
        .w_full()
        .pl(css(12.0))
        .pr(css(8.0))
        .rounded(css(9999.0))
        .cursor_pointer()
        .text_size(css(text_size(14.0, cx)))
        .text_color(muted)
        .hover(|s| s.bg(appearance.hsl_token("sidebar-item-hover")))
        .child(icon(glyph, 15.0, 1.8).text_color(muted))
        .child(
            div()
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
                        .child(name),
                )
                .child(
                    div()
                        .flex_none()
                        .opacity(0.0)
                        .group_hover(group, |s| s.opacity(1.0))
                        .child(
                            icon(chevron.svg(), 13.0, 2.0)
                                .text_color(appearance.token("cmd-palette-item-meta")),
                        ),
                ),
        )
}

/// A group: its header and, when open, its first tasks (`pt-0.5 gap-0.5`, text rows indented),
/// with its drawn height.
fn group(
    fold_id: String,
    header: gpui::Stateful<gpui::Div>,
    sessions: &[Session],
    collapsed: bool,
    place: &Place,
    window: &mut Window,
    cx: &App,
) -> (AnyElement, f32) {
    let now = jiff::Timestamp::now().as_millisecond();
    let limit = fold::limit(text(cx.global::<Preferences>(), SESSION_LIMIT_KEY), 5);
    let show_key = format!("{}/{fold_id}", place.section);
    let expanded = cx.global::<Sessions>().show_all.contains(&show_key);
    let (visible, hidden) = fold::view(
        sessions,
        limit,
        expanded,
        Some(fold::recent_since(now)),
        |s| s.activity_at,
    );
    let rows = visible.len() as f32;
    let footer = hidden > 0;
    let list = place.list;
    let row_height = if list {
        list_row::height(cx)
    } else {
        ROW_HEIGHT
    };
    let height =
        2.0 + rows * row_height + (rows - 1.0).max(0.0) * 2.0 + if footer { 26.0 } else { 0.0 };
    let id = fold_id.clone();
    let shown = show_key.clone();
    let body = || {
        div()
            .flex()
            .flex_col()
            .gap(css(2.0))
            .pt(css(2.0))
            .when(list, |el| el.pl(css(8.0)))
            // `data-no-drag`: a press on a task does not drag its project.
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .children(visible.iter().enumerate().map(|(i, s)| {
                if list {
                    list_row(s, i == 0, None, now, cx)
                } else {
                    row(s, true, None, now, Scope::Sidebar, cx)
                }
            }))
            .when(footer, |el| {
                el.child(ShowAll::new(
                    format!("{id}-show-all"),
                    sessions.len(),
                    move |cx| {
                        cx.global_mut::<Sessions>().show_all.insert(shown.clone());
                    },
                ))
            })
            .into_any_element()
    };
    let toggle = fold_id.clone();
    let header = header.on_click(move |_, _, cx| {
        let now_collapsed = !collapsed;
        log::info!(target: "sessions", "{toggle} {}", if now_collapsed { "folded" } else { "unfolded" });
        if toggle == DIALOGUE_FOLD {
            if !set_dialogue_collapsed(now_collapsed, cx) {
                return;
            }
        } else if now_collapsed {
            cx.global_mut::<Sessions>().collapsed.insert(toggle.clone());
        } else {
            cx.global_mut::<Sessions>().collapsed.remove(&toggle);
        }
        fold::toggle(&toggle, now_collapsed, cx);
        cx.refresh_windows();
    });
    let open = fold::openness(&fold_id, collapsed, cx);
    let body = fold::section(&fold_id, collapsed, height, body, window, cx);
    if body.is_some() {
        place.drawn.borrow_mut().push(show_key);
    }
    let element = div()
        .relative()
        .flex()
        .flex_col()
        .w_full()
        .child(header)
        .children(body)
        .into_any_element();
    (element, 32.0 + height * open)
}

/// `ProjectNode`, folded per project key.
pub(super) fn project(
    p: &Project,
    place: &Place,
    window: &mut Window,
    cx: &App,
) -> (AnyElement, f32) {
    let fold_id = format!("project:{}", p.key);
    let collapsed = cx.global::<Sessions>().collapsed.contains(&fold_id);
    let glyph = if collapsed {
        IconName::Folder
    } else {
        IconName::FolderOpen
    };
    let head = header(
        SharedString::from(format!("{fold_id}-header")),
        glyph.svg(),
        p.name.clone(),
        collapsed,
        cx,
    );
    group(fold_id, head, &p.sessions, collapsed, place, window, cx)
}

/// `CCAgentSidebarUpper`'s scroll body: the Pinned section above the main list; 8.4c draws the
/// scope header between them. Show all resets for lists no longer drawn.
pub fn list(window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    super::marquee::sweep(cx);
    super::sortable::sweep(window, cx);
    super::select::begin(cx);
    super::reveal::step(window, cx);
    let drawn = RefCell::new(Vec::new());
    let view_style = pinned::view_style(window, cx);
    // The header stays while the list loads or fails, as `MainListScopeHeader` does.
    let (pinned, main) = match draw(&drawn, view_style, window, cx) {
        Some((pinned, main)) => (pinned, Some(main)),
        None => (None, None),
    };
    let drawn = drawn.into_inner();
    let store = cx.global_mut::<Sessions>();
    store.masonry.mounted &= drawn.iter().any(|k| k == super::masonry::DRAWN);
    let before = store.show_all.len();
    store.show_all.retain(|k| drawn.contains(k));
    if store.show_all.len() < before {
        log::debug!(target: "sessions", "Show all reset for {} undrawn lists", before - store.show_all.len());
    }
    // `ProjectsSection`'s root: the scope header over the list body.
    let section = div()
        .flex()
        .flex_col()
        .w_full()
        .gap(css(2.0))
        .child(super::header::header(window, cx))
        .children(main);
    // The list stays built while results show, as `hidden` keeps it mounted.
    let handle = cx.global::<super::reveal::Reveal>().handle.clone();
    super::search::scroll::sync(&handle, cx);
    let results = super::search::results::body(super::search::Which::Inline, window, cx);
    let pin_search = !super::search::trimmed(super::search::Which::Inline, cx).is_empty();
    let list = div()
        .flex()
        .flex_col()
        .gap(css(8.0))
        .when(results.is_some(), |el| el.hidden())
        .children(pinned)
        .child(section);
    let scroll = div()
        .id("main-list")
        .debug_selector(|| "main-list".into())
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .gap(css(8.0))
        .pb(css(16.0))
        .overflow_y_scroll()
        .track_scroll(&handle)
        .child(super::search::scroll::pointer(handle.clone()))
        .children(nav(&handle, pin_search, window, cx))
        .children(results.map(super::search::stick::after))
        .child(super::search::stick::after(list.into_any_element()))
        .child(super::select::end());
    Some(
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .children(super::select::bar(window, cx))
            .child(scroll)
            .into_any_element(),
    )
}

/// The Search row and More, or Customize in their place, which no query pins.
fn nav(handle: &ScrollHandle, pin: bool, window: &mut Window, cx: &mut App) -> Vec<AnyElement> {
    use super::search::stick;
    if let Some(panel) = crate::sidebar::top_nav::customize::panel(window, cx) {
        let row = stick::row(panel, handle.clone(), false).into_any_element();
        return [Some(row), super::search::listen(cx)]
            .into_iter()
            .flatten()
            .collect();
    }
    let search = stick::row(super::search::section(window, cx), handle.clone(), pin);
    let more = stick::after(crate::sidebar::top_nav::more::section(window, cx));
    vec![search.into_any_element(), more.into_any_element()]
}

/// The sidebar's lists from the loaded bucket under the filter, as `ExpandedView` derives them.
pub(super) fn lists(cx: &App) -> Option<Lists> {
    let prefs = cx.global::<Preferences>();
    let loaded = cx.global::<Sessions>().loaded.as_ref()?;
    let filter = Filter::read(prefs);
    let order = Order {
        flat: filter.flat,
        created: filter.sort_by == "created",
        custom: filter.project_order == "custom",
        manual: &[],
        group_dialogue: filter.group_dialogue,
        vendor: (filter.vendor != "all").then_some(filter.vendor),
        cutoff: filter.cutoff(jiff::Timestamp::now().as_millisecond()),
    };
    let dialogues = cx
        .global::<Translator>()
        .t("ccAgent.sidebar.dialogues", &[]);
    Some(build(
        &loaded.sessions,
        &loaded.recent,
        &aliases(&loaded.aliases, PLATFORM),
        &loaded.settings,
        PLATFORM,
        &order,
        &dialogues,
    ))
}

/// The Pinned section and the main list's body.
fn draw(
    drawn: &RefCell<Vec<String>>,
    view_style: AnyElement,
    window: &mut Window,
    cx: &App,
) -> Option<(Option<AnyElement>, gpui::Div)> {
    let list = view_mode::main(cx) == Main::List;
    let main_place = Place {
        list,
        section: "main",
        drawn,
    };
    let prefs = cx.global::<Preferences>();
    let store = cx.global::<Sessions>();
    let lists = lists(cx)?;
    let limit = fold::limit(text(prefs, PROJECT_LIMIT_KEY), 20);
    let dialogues = cx
        .global::<Translator>()
        .t("ccAgent.sidebar.dialogues", &[]);
    let (visible, hidden) = fold::view(&lists.main, limit, store.show_all_entries, None, |_| 0);
    let now = jiff::Timestamp::now().as_millisecond();
    let mut children: Vec<AnyElement> = visible
        .into_iter()
        .map(|entry| match entry {
            Entry::Project(p) => project(p, &main_place, window, cx).0,
            Entry::Dialogues(list) => {
                let collapsed = dialogue_collapsed(prefs);
                let head = header(
                    format!("{DIALOGUE_FOLD}-header").into(),
                    MESSAGES_SQUARE,
                    dialogues.clone(),
                    collapsed,
                    cx,
                );
                group(
                    DIALOGUE_FOLD.into(),
                    head,
                    list,
                    collapsed,
                    &main_place,
                    window,
                    cx,
                )
                .0
            }
            // A loose task is its own list, with no top divider.
            Entry::Loose(s) => {
                let label = lists.main_labels.get(&s.id).map(String::as_str);
                if list {
                    list_row(s, false, label, now, cx)
                } else {
                    row(s, false, label, now, Scope::Sidebar, cx)
                }
            }
        })
        .collect();
    if hidden > 0 {
        children.push(
            ShowAll::new("main-list-show-all".into(), lists.main.len(), |cx| {
                cx.global_mut::<Sessions>().show_all_entries = true;
            })
            .into_any_element(),
        );
    }
    let main = div()
        .flex()
        .flex_col()
        .gap(css(4.0))
        .pt(css(4.0))
        .pl(css(12.0))
        .children(children);
    Some((
        pinned::section(&lists, view_style, now, drawn, window, cx),
        main,
    ))
}
