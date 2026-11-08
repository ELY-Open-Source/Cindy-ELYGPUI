//! `ProjectsSection.tsx` in `project` group-by mode: project and dialogue groups, loose tasks.

use gpui::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _,
};

use super::{
    Sessions,
    flat::{ROW_HEIGHT, SORT_BY_KEY, row, scroll},
    fold,
    group::{Entry, Order, Session, aliases, entries},
    paths::PLATFORM,
};
use ely_gpui_component::primitives::IconName;

use crate::{
    appearance_settings::text_size,
    i18n::{Arg, Translator},
    lucide::{MESSAGES_SQUARE, icon},
    preferences::Preferences,
    theme::Appearance,
    units::css,
};

const PROJECT_ORDER_KEY: &str = "cc-agent.sidebar.filter.projectOrder";
const GROUP_DIALOGUE_KEY: &str = "cc-agent.sidebar.filter.groupDialogue";
const DIALOGUE_COLLAPSED_KEY: &str = "cc-agent.sidebar.dialogueGroupCollapsed";
const SESSION_LIMIT_KEY: &str = "sidebar.collapse.projectSessionLimit";
const PROJECT_LIMIT_KEY: &str = "sidebar.collapse.projectLimit";
/// `DIALOGUE_GROUP_ALL_KEY`, the one dialogue group of an ungrouped list.
const DIALOGUE_ALL: &str = "all";
const DIALOGUE_FOLD: &str = "dialogue:all";

fn text<'a>(prefs: &'a Preferences, key: &str) -> Option<&'a str> {
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

/// `persistDialogueGroupCollapsedKeys`, keeping other groups' keys.
fn set_dialogue_collapsed(collapsed: bool, cx: &mut App) {
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
    }
}

/// `ProjectNode` and `SessionGroupNode`'s header: icon, name, hover chevron.
fn header(
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

/// The Show all footer of `SessionEntryList` and `ShowAllEntriesButton`.
fn show_all(id: SharedString, count: usize, cx: &App) -> gpui::Stateful<gpui::Div> {
    let appearance = cx.global::<Appearance>();
    let label = cx.global::<Translator>().t(
        "ccAgent.sidebar.showAllSessions",
        &[("count", Arg::Number(count as i64))],
    );
    let selector = id.to_string();
    div()
        .id(id)
        .debug_selector(move || selector)
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
        .child(label)
}

/// A group: its header and, when open, its first tasks (`pt-0.5 pl-2 gap-0.5`, rows indented).
fn group(
    fold_id: String,
    header: gpui::Stateful<gpui::Div>,
    sessions: &[Session],
    collapsed: bool,
    now: i64,
    window: &mut Window,
    cx: &App,
) -> AnyElement {
    let limit = fold::limit(text(cx.global::<Preferences>(), SESSION_LIMIT_KEY), 5);
    let expanded = cx.global::<Sessions>().show_all.contains(&fold_id);
    let (visible, hidden) = fold::view(
        sessions,
        limit,
        expanded,
        Some(fold::recent_since(now)),
        |s| s.activity_at,
    );
    let rows = visible.len() as f32;
    let footer = hidden > 0;
    let height =
        2.0 + rows * ROW_HEIGHT + (rows - 1.0).max(0.0) * 2.0 + if footer { 26.0 } else { 0.0 };
    let id = fold_id.clone();
    let body = || {
        div()
            .flex()
            .flex_col()
            .gap(css(2.0))
            .pt(css(2.0))
            .pl(css(8.0))
            .children(
                visible
                    .iter()
                    .map(|s| row(s, true, now, cx).into_any_element()),
            )
            .when(footer, |el| {
                el.child(
                    show_all(format!("{id}-show-all").into(), sessions.len(), cx).on_click(
                        move |_, _, cx| {
                            cx.global_mut::<Sessions>().show_all.insert(id.clone());
                            cx.refresh_windows();
                        },
                    ),
                )
            })
            .into_any_element()
    };
    let toggle = fold_id.clone();
    let header = header.on_click(move |_, _, cx| {
        let now_collapsed = !collapsed;
        log::info!(target: "sessions", "{toggle} {}", if now_collapsed { "folded" } else { "unfolded" });
        if toggle == DIALOGUE_FOLD {
            set_dialogue_collapsed(now_collapsed, cx);
        } else if now_collapsed {
            cx.global_mut::<Sessions>().collapsed.insert(toggle.clone());
        } else {
            cx.global_mut::<Sessions>().collapsed.remove(&toggle);
        }
        fold::toggle(&toggle, now_collapsed, cx);
        cx.refresh_windows();
    });
    div()
        .relative()
        .flex()
        .flex_col()
        .w_full()
        .child(header)
        .children(fold::section(&fold_id, collapsed, height, body, window, cx))
        .into_any_element()
}

/// The main list unless the filter groups it flat; 8.4c draws the scope header above it.
pub fn list(window: &mut Window, cx: &App) -> Option<AnyElement> {
    let prefs = cx.global::<Preferences>();
    let store = cx.global::<Sessions>();
    let loaded = store.loaded.as_ref()?;
    let order = Order {
        created: text(prefs, SORT_BY_KEY) == Some("created"),
        custom: match text(prefs, PROJECT_ORDER_KEY) {
            Some("custom") => true,
            Some("activity") => false,
            _ => text(prefs, SORT_BY_KEY) == Some("manual"),
        },
        manual: &[],
        group_dialogue: text(prefs, GROUP_DIALOGUE_KEY) != Some("false"),
    };
    let all = entries(
        &loaded.sessions,
        &loaded.recent,
        &aliases(&loaded.aliases, PLATFORM),
        PLATFORM,
        &order,
    );
    let limit = fold::limit(text(prefs, PROJECT_LIMIT_KEY), 20);
    let (visible, hidden) = fold::view(&all, limit, store.show_all_entries, None, |_| 0);
    let now = jiff::Timestamp::now().as_millisecond();
    let mut children: Vec<AnyElement> = visible
        .into_iter()
        .map(|entry| match entry {
            Entry::Project(p) => {
                let fold_id = format!("project:{}", p.key);
                let collapsed = store.collapsed.contains(&fold_id);
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
                group(fold_id, head, &p.sessions, collapsed, now, window, cx)
            }
            Entry::Dialogues(list) => {
                let collapsed = dialogue_collapsed(prefs);
                let name = cx
                    .global::<Translator>()
                    .t("ccAgent.sidebar.dialogues", &[]);
                let head = header(
                    format!("{DIALOGUE_FOLD}-header").into(),
                    MESSAGES_SQUARE,
                    name,
                    collapsed,
                    cx,
                );
                group(DIALOGUE_FOLD.into(), head, list, collapsed, now, window, cx)
            }
            Entry::Loose(s) => row(s, false, now, cx).into_any_element(),
        })
        .collect();
    if hidden > 0 {
        children.push(
            show_all("main-list-show-all".into(), all.len(), cx)
                .on_click(|_, _, cx| {
                    cx.global_mut::<Sessions>().show_all_entries = true;
                    cx.refresh_windows();
                })
                .into_any_element(),
        );
    }
    Some(scroll("main-list", children))
}
