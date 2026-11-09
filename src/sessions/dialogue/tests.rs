//! The dialogues' header: a Tab stop that folds once per key, and its menu.

use gpui::{
    KeyDownEvent, KeyUpEvent, Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent,
    TestAppContext, VisualTestContext,
};

use super::ID;
use crate::sessions::view_tests::open;

fn press(cx: &mut VisualTestContext, button: MouseButton) {
    let at = cx.debug_bounds(ID).unwrap().center();
    let modifiers = Modifiers::none();
    cx.simulate_event(MouseDownEvent {
        position: at,
        modifiers,
        button,
        click_count: 1,
        first_mouse: false,
    });
    cx.simulate_event(MouseUpEvent {
        position: at,
        modifiers,
        button,
        click_count: 1,
    });
    cx.run_until_parked();
}

/// A key down and up, where gpui's keyboard click lands.
fn key(cx: &mut VisualTestContext, name: &'static str) {
    let keystroke = Keystroke::parse(name).unwrap();
    cx.simulate_event(KeyDownEvent {
        keystroke: keystroke.clone(),
        is_held: false,
        prefer_character_input: false,
    });
    cx.simulate_event(KeyUpEvent { keystroke });
    cx.run_until_parked();
}

/// Whether the group shows its task once a fold has run.
fn open_group(cx: &mut VisualTestContext) -> bool {
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(250));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.debug_bounds("task-d0").is_some()
}

const SEARCH: &str = "menu-row-Search This Chat Group";

#[gpui::test]
fn a_click_enter_and_space_each_fold_once(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    assert!(open_group(cx));
    press(cx, MouseButton::Left);
    assert!(!open_group(cx), "a click folds");
    key(cx, "enter");
    assert!(open_group(cx), "Enter unfolds");
    key(cx, "space");
    assert!(!open_group(cx), "Space folds");
}

#[gpui::test]
fn a_right_click_opens_the_menu_without_folding(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    press(cx, MouseButton::Right);
    assert!(open_group(cx));
    assert!(cx.debug_bounds(SEARCH).is_some());
}

#[gpui::test]
fn the_menu_key_opens_the_menu_under_the_left_edge(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    press(cx, MouseButton::Left);
    assert!(!open_group(cx));
    cx.simulate_keystrokes("shift-f10");
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let head = cx.debug_bounds(ID).unwrap();
    let row = cx.debug_bounds(SEARCH).expect("menu open");
    assert!(row.top() >= head.bottom(), "{row:?} under {head:?}");
    assert!(
        row.left() < head.left() + gpui::px(20.0),
        "{row:?} at {head:?}"
    );
    assert!(!open_group(cx), "the menu key does not unfold");
}
