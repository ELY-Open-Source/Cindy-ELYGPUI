use gpui::{Modifiers, MouseMoveEvent, TestAppContext, VisualTestContext, point, px};

use super::SORT_KEY;
use crate::{
    preferences::{FILE, Preferences},
    sessions::{
        search::{Search, query},
        view_tests::{click, open},
    },
};

fn drawn(cx: &mut VisualTestContext, selector: &'static str) -> bool {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.debug_bounds(selector).is_some()
}

fn hover(cx: &mut VisualTestContext, selector: &'static str) {
    let position = cx.debug_bounds(selector).unwrap().center();
    cx.simulate_event(MouseMoveEvent {
        position,
        modifiers: Modifiers::none(),
        pressed_button: None,
    });
    cx.run_until_parked();
}

fn away(cx: &mut VisualTestContext) {
    cx.simulate_event(MouseMoveEvent {
        position: point(px(390.0), px(890.0)),
        modifiers: Modifiers::none(),
        pressed_button: None,
    });
    cx.run_until_parked();
}

/// Opens the options from a hovered row and walks `rows`.
fn pick(cx: &mut VisualTestContext, rows: &[&str]) {
    hover(cx, "nav-search");
    click(cx, "search-filter-button");
    for row in rows {
        click(cx, Box::leak(format!("menu-row-{row}").into_boxed_str()));
    }
}

fn sort(cx: &mut VisualTestContext) -> &'static str {
    cx.update(|_, cx| cx.global::<Search>().options.sort)
}

fn with_sort(dir: &std::path::Path, value: &str) {
    let prefs = serde_json::json!({ SORT_KEY: value });
    std::fs::write(dir.join(FILE), prefs.to_string()).unwrap();
}

#[gpui::test]
fn the_sort_is_read_back(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    with_sort(dir.path(), "activityAsc");
    let cx = open(cx, dir.path());
    assert_eq!(sort(cx), "activityAsc");
}

#[gpui::test]
fn an_unknown_sort_is_relevance(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    with_sort(dir.path(), "newest");
    let cx = open(cx, dir.path());
    assert_eq!(sort(cx), "relevance");
}

#[gpui::test]
fn a_chosen_sort_is_saved(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    pick(cx, &["Sort by", "Recently active"]);
    assert_eq!(sort(cx), "activityDesc");
    let saved = Preferences::load(dir.path()).unwrap();
    assert_eq!(saved.text(SORT_KEY).unwrap(), Some("activityDesc"));
    assert!(!drawn(cx, "search-filter-dot"), "the sort never counts");
}

#[gpui::test]
fn a_filter_shows_the_dot_and_reset_clears_it(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    pick(cx, &["Status", "Archived"]);
    let count = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| cx.global::<Search>().options.active_count())
    };
    assert_eq!(count(cx), 1);
    assert!(drawn(cx, "search-filter-dot"));
    pick(cx, &["Last activity", "7d"]);
    assert_eq!(count(cx), 2);
    pick(cx, &["Reset"]);
    assert_eq!(count(cx), 0);
    assert!(!drawn(cx, "search-filter-dot"));
    assert_eq!(sort(cx), "relevance", "reset leaves the sort");
}

#[gpui::test]
fn the_open_menu_holds_the_row_open(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    pick(cx, &[]);
    assert!(drawn(cx, "menu-panel-0"));
    away(cx);
    assert!(!drawn(cx, "nav-search-open"), "menuOpen keeps it expanded");
}

#[gpui::test]
fn a_press_on_the_menu_keeps_the_query(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    click(cx, "nav-search-open");
    cx.simulate_input("plan");
    pick(cx, &["Status"]);
    click(cx, "menu-row-Archived");
    assert_eq!(cx.update(|_, cx| query(cx).to_owned()), "plan");
    click(cx, "search-filter-button");
    assert!(drawn(cx, "menu-panel-0"));
    let panel = cx.debug_bounds("menu-panel-0").unwrap();
    let foot = cx.debug_bounds("main-list").unwrap().bottom();
    assert!(panel.bottom() < foot - px(4.0), "{panel:?}");
    crate::sessions::search::tests::press_below(cx);
    assert_eq!(
        cx.update(|_, cx| query(cx).to_owned()),
        "",
        "a press past the open menu clears it"
    );
}

#[gpui::test]
fn a_project_choice_counts_and_names_its_projects(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    pick(cx, &["Projects", "app · 7"]);
    let label = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| {
            let t = cx.global::<crate::i18n::Translator>();
            super::projects_label(&cx.global::<Search>().options, t)
        })
    };
    assert_eq!(label(cx), "1 selected");
    assert!(drawn(cx, "search-filter-dot"));
    pick(cx, &["Reset"]);
    assert_eq!(label(cx), "All projects");
    assert!(!drawn(cx, "search-filter-dot"));
}
