//! The task menu and rename driven by real mouse and key events. No database is open here,
//! so every write fails and rolls back, as `handleRename` does when `patchMeta` throws.

use gpui::{
    KeyUpEvent, Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, TestAppContext,
    VisualTestContext,
};

use super::{
    Sessions,
    view_tests::{click, open},
};
use crate::toast::Toasts;

fn press(cx: &mut VisualTestContext, selector: &'static str, button: MouseButton, count: usize) {
    let at = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} not drawn"))
        .center();
    let modifiers = Modifiers::none();
    cx.simulate_event(MouseDownEvent {
        position: at,
        modifiers,
        button,
        click_count: count,
        first_mouse: false,
    });
    cx.simulate_event(MouseUpEvent {
        position: at,
        modifiers,
        button,
        click_count: count,
    });
    cx.run_until_parked();
}

fn toasts(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|window, cx| {
        cx.global::<Toasts>()
            .items(window.window_handle().window_id())
            .iter()
            .map(|item| item.message.to_string())
            .collect()
    })
}

fn row(cx: &mut VisualTestContext, id: &str) -> (String, bool) {
    cx.update(|_, cx| {
        let s = cx.global::<Sessions>().loaded.as_ref().unwrap();
        let s = s.sessions.iter().find(|s| s.id == id).unwrap();
        (s.title.clone(), s.muted)
    })
}

fn editing(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.debug_bounds("task-rename").is_some()
}

#[gpui::test]
fn a_double_click_renames_and_escape_drops_it(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    press(cx, "task-a0", MouseButton::Left, 1);
    assert!(!editing(cx), "one click does not rename");
    press(cx, "task-a0", MouseButton::Left, 2);
    assert!(editing(cx));
    cx.simulate_input("Other");
    cx.simulate_keystrokes("escape");
    assert!(!editing(cx));
    assert_eq!(row(cx, "a0").0, "a0");
    assert!(toasts(cx).is_empty(), "nothing was written");
}

#[gpui::test]
fn a_failed_rename_rolls_back_with_a_toast(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    press(cx, "task-a0", MouseButton::Right, 1);
    click(cx, "menu-row-Rename");
    assert!(editing(cx));
    cx.simulate_input("  Renamed  ");
    cx.simulate_keystrokes("enter");
    assert!(!editing(cx));
    assert_eq!(row(cx, "a0").0, "a0", "rolled back");
    assert_eq!(toasts(cx), ["Rename failed"]);
}

#[gpui::test]
fn an_unchanged_title_saves_nothing(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    press(cx, "task-a1", MouseButton::Left, 2);
    cx.simulate_keystrokes("enter");
    assert!(!editing(cx));
    assert!(toasts(cx).is_empty());
}

#[gpui::test]
fn more_copies_the_link_and_a_failed_mute_rolls_back(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    let at = cx.debug_bounds("task-a0").unwrap().center();
    cx.simulate_mouse_move(at, None, Modifiers::none());
    click(cx, "task-more-a0");
    click(cx, "menu-row-Copy session link");
    let copied = cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(copied.as_deref(), Some("cindy://session/a0"));
    assert_eq!(toasts(cx), ["Link copied"]);
    press(cx, "task-a0", MouseButton::Right, 1);
    click(cx, "menu-row-Mute notifications");
    assert!(!row(cx, "a0").1, "rolled back");
    assert_eq!(
        toasts(cx),
        ["Failed to update notification settings", "Link copied"],
        "newest first"
    );
}

#[gpui::test]
fn an_edit_ends_when_its_task_leaves_the_list(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    press(cx, "task-a0", MouseButton::Left, 2);
    assert!(editing(cx));
    cx.update(|_, cx| {
        let store = cx.global_mut::<Sessions>();
        let mut loaded = store.loaded.take().unwrap();
        loaded.sessions.retain(|s| s.id != "a0");
        store.show(loaded);
    });
    assert!(!editing(cx));
    press(cx, "task-a1", MouseButton::Left, 2);
    assert!(editing(cx), "another row still renames");
}

#[gpui::test]
fn a_rename_keeps_what_javascript_trim_keeps(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    press(cx, "task-a0", MouseButton::Left, 2);
    cx.simulate_input("\u{85}");
    cx.simulate_keystrokes("enter");
    assert_eq!(toasts(cx), ["Rename failed"], "U+0085 is not blank");
}

/// A key pressed and released, as gpui's keyboard click waits for the release.
fn key(cx: &mut VisualTestContext, key: &'static str) {
    cx.simulate_keystrokes(key);
    let keystroke = Keystroke::parse(key).unwrap();
    cx.simulate_event(KeyUpEvent { keystroke });
    cx.run_until_parked();
}

#[gpui::test]
fn tab_reaches_more_and_enter_opens_its_menu(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    key(cx, "tab");
    key(cx, "enter");
    assert!(
        cx.debug_bounds("menu-row-Rename").is_some(),
        "the menu opened"
    );
}
