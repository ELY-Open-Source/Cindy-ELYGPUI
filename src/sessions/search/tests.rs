use gpui::{Modifiers, MouseMoveEvent, TestAppContext, VisualTestContext, point, px};

use super::{
    query,
    run::tests::{search, wait},
    trimmed,
};
use crate::localdb::search::conversations::tests::task;
use crate::sessions::view_tests::{click, open};

fn drawn(cx: &mut VisualTestContext, selector: &'static str) -> bool {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.debug_bounds(selector).is_some()
}

fn typed(cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| query(crate::sessions::search::Which::Inline, cx).to_owned())
}

/// A press on the list's empty foot, outside the search surface.
pub(super) fn press_below(cx: &mut VisualTestContext) {
    let list = cx.debug_bounds("main-list").unwrap();
    let at = point(list.center().x, list.bottom() - px(4.0));
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
}

/// The pointer far from the row, so hover does not hold it open.
fn away(cx: &mut VisualTestContext) {
    cx.simulate_event(MouseMoveEvent {
        position: point(px(390.0), px(890.0)),
        modifiers: Modifiers::none(),
        pressed_button: None,
    });
    cx.run_until_parked();
}

#[gpui::test]
fn at_rest_it_is_a_nav_row(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    assert!(drawn(cx, "nav-search-open"));
    assert!(!drawn(cx, "nav-search-clear"));
}

#[gpui::test]
fn a_click_opens_the_field_and_typing_sets_the_query(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    click(cx, "nav-search-open");
    away(cx);
    assert!(!drawn(cx, "nav-search-open"), "the field took the row");
    cx.simulate_input("  plan ");
    assert_eq!(typed(cx), "  plan ");
    assert_eq!(
        cx.update(|_, cx| trimmed(crate::sessions::search::Which::Inline, cx).to_owned()),
        "plan"
    );
    assert!(drawn(cx, "nav-search-clear"));
}

#[gpui::test]
fn clear_empties_the_query_and_lets_the_row_rest(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    click(cx, "nav-search-open");
    cx.simulate_input("plan");
    click(cx, "nav-search-clear");
    assert_eq!(typed(cx), "");
    away(cx);
    assert!(
        drawn(cx, "nav-search-open"),
        "no hover, focus or query holds it"
    );
}

#[gpui::test]
fn a_query_holds_the_field_open_after_focus_leaves(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    click(cx, "nav-search-open");
    cx.simulate_input("plan");
    cx.update(|window, cx| window.blur(cx));
    away(cx);
    assert!(!drawn(cx, "nav-search-open"));
}

#[gpui::test]
fn a_press_outside_clears_a_running_query(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    click(cx, "nav-search-open");
    cx.simulate_input("plan");
    click(cx, "nav-search");
    assert_eq!(typed(cx), "plan", "a press on the row keeps it");
    press_below(cx);
    assert_eq!(typed(cx), "");
}

#[gpui::test]
fn tab_onto_search_lands_in_the_field(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    cx.simulate_keystrokes("tab");
    cx.run_until_parked();
    away(cx);
    assert!(!drawn(cx, "nav-search-open"), "focus within opened it");
    cx.simulate_input("x");
    assert_eq!(typed(cx), "x");
}

#[gpui::test]
fn a_running_query_prunes_the_selection_to_its_results(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = search(cx, dir.path(), Some(plans));
    away(cx);
    cx.update(|window, cx| window.blur(cx));
    cx.run_until_parked();
    let row = cx.debug_bounds("task-a1").unwrap();
    cx.simulate_click(row.center(), Modifiers::control());
    cx.run_until_parked();
    assert!(drawn(cx, "bulk-count"));
    click(cx, "nav-search-open");
    cx.simulate_input("plan");
    assert!(!drawn(cx, "bulk-count"), "the searching line shows no rows");
    wait(cx, 250);
    assert!(drawn(cx, "search-result-a1"));
}

/// Tasks titled `plan`, `a1` among them.
fn plans(db: &rusqlite::Connection) {
    for i in 0..3 {
        task(db, &format!("a{i}"), &format!("plan {i}"), i);
    }
}

#[gpui::test]
fn a_key_press_keeps_the_hover(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    let row = cx.debug_bounds("nav-search").unwrap();
    cx.simulate_event(MouseMoveEvent {
        position: row.center(),
        modifiers: Modifiers::none(),
        pressed_button: None,
    });
    cx.run_until_parked();
    assert!(!drawn(cx, "nav-search-open"), "hover opened it");
    cx.simulate_keystrokes("a");
    cx.run_until_parked();
    assert!(
        !drawn(cx, "nav-search-open"),
        "the pointer still rests on it"
    );
}
