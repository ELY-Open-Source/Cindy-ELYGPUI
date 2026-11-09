use gpui::{
    Modifiers, MouseMoveEvent, ScrollDelta, ScrollEvent, TestAppContext, TouchPhase,
    VisualTestContext, point, px, size,
};

use super::{query, trimmed};
use crate::sessions::view_tests::{click, open};

fn drawn(cx: &mut VisualTestContext, selector: &'static str) -> bool {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.debug_bounds(selector).is_some()
}

fn typed(cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| query(cx).to_owned())
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
    assert_eq!(cx.update(|_, cx| trimmed(cx).to_owned()), "plan");
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
    click(cx, "task-a2");
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
fn a_press_where_the_scrolled_off_row_was_still_clears(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    cx.simulate_resize(size(px(400.0), px(240.0)));
    cx.run_until_parked();
    let row = cx.debug_bounds("task-a1").unwrap();
    cx.simulate_click(row.center(), Modifiers::control());
    cx.run_until_parked();
    click(cx, "nav-search-open");
    cx.simulate_input("plan");
    let list = cx.debug_bounds("main-list").unwrap();
    cx.simulate_event(ScrollEvent {
        position: list.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-40.0))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
        ..Default::default()
    });
    cx.run_until_parked();
    let search = cx.debug_bounds("nav-search").unwrap();
    let bar = cx.debug_bounds("bulk-count").unwrap();
    assert!(search.contains(&bar.center()), "{search:?} under {bar:?}");
    cx.simulate_click(bar.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(typed(cx), "", "the bar took the press, not the hidden row");
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
