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
    // Search, its options, More, the project and its More, the row, then its More.
    key(cx, "tab");
    key(cx, "tab");
    key(cx, "tab");
    key(cx, "tab");
    key(cx, "tab");
    key(cx, "tab");
    key(cx, "tab");
    key(cx, "enter");
    assert!(
        cx.debug_bounds("menu-row-Rename").is_some(),
        "the menu opened"
    );
}

#[gpui::test]
fn pin_promotes_the_order_and_a_failed_write_rolls_back(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    let owner = dir.path().join("owner");
    cx.update(|_, cx| cx.global_mut::<Sessions>().owner_dir = Some(owner.clone()));
    press(cx, "task-a0", MouseButton::Right, 1);
    click(cx, "menu-row-Pin");
    cx.executor().run_until_parked();
    let pinned = cx.update(|_, cx| {
        let loaded = cx.global::<Sessions>().loaded.as_ref().unwrap();
        let row = loaded.sessions.iter().find(|s| s.id == "a0").unwrap();
        (row.pinned_at, loaded.settings.pinned_order.clone())
    });
    assert_eq!(
        pinned,
        (None, vec!["a0".to_string()]),
        "pinnedAt rolled back, the order kept"
    );
    let saved = std::fs::read_to_string(owner.join(super::settings::FILE)).unwrap();
    assert!(saved.contains("\"a0\""), "{saved}");
    assert_eq!(toasts(cx), ["Operation failed"]);
}

#[gpui::test]
fn a_failed_order_save_puts_the_order_back(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    press(cx, "task-a0", MouseButton::Right, 1);
    click(cx, "menu-row-Pin");
    cx.executor().run_until_parked();
    let order = cx.update(|_, cx| {
        let loaded = cx.global::<Sessions>().loaded.as_ref().unwrap();
        loaded.settings.pinned_order.clone()
    });
    assert!(order.is_empty(), "no owner, so the save failed");
    assert_eq!(toasts(cx), ["Operation failed", "Operation failed"]);
}

#[gpui::test]
fn tab_reaches_the_pinned_view_style_and_space_opens_it(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    cx.update(|window, cx| {
        let loaded = cx.global_mut::<Sessions>().loaded.as_mut().unwrap();
        loaded.sessions[0].pinned_at = Some(1);
        window.refresh();
    });
    cx.run_until_parked();
    key(cx, "tab");
    key(cx, "tab");
    key(cx, "tab");
    key(cx, "tab");
    key(cx, "space");
    assert!(
        cx.debug_bounds("menu-row-Cards").is_some(),
        "the menu opened"
    );
}

#[gpui::test]
fn a_right_click_while_renaming_opens_no_task_menu(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    press(cx, "task-a0", MouseButton::Left, 2);
    assert!(editing(cx));
    press(cx, "task-rename", MouseButton::Right, 1);
    assert!(cx.debug_bounds("menu-row-Rename").is_none(), "no task menu");
    assert!(editing(cx), "still renaming");
}

#[gpui::test]
fn flat_loose_tasks_carry_their_source_label(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    let label = |cx: &mut VisualTestContext, id: &str| {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        cx.debug_bounds(Box::leak(format!("task-label-{id}").into_boxed_str()))
            .is_some()
    };
    assert!(!label(cx, "a1"), "a project's own rows carry none");
    cx.update(|_, cx| {
        let prefs = cx.global_mut::<crate::preferences::Preferences>();
        prefs
            .set("cc-agent.sidebar.filter.groupBy", "flat")
            .unwrap();
    });
    assert!(label(cx, "a1"), "flat list row");
    cx.update(|_, cx| super::view_mode::set_main(super::view_mode::Main::Text, cx));
    assert!(label(cx, "a1"), "flat text row");
}

#[gpui::test]
fn a_failed_unpin_puts_the_summary_back(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    let summary = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| {
            let loaded = cx.global::<Sessions>().loaded.as_ref().unwrap();
            let row = loaded.sessions.iter().find(|s| s.id == "a0").unwrap();
            (row.pinned_at, row.summary.clone())
        })
    };
    cx.update(|window, cx| {
        let loaded = cx.global_mut::<Sessions>().loaded.as_mut().unwrap();
        loaded.sessions[0].pinned_at = Some(1);
        loaded.sessions[0].summary = Some("done".into());
        super::write::apply("a0", super::write::Meta::Pinned(None), window, cx);
    });
    cx.run_until_parked();
    assert_eq!(summary(cx), (Some(1), Some("done".into())), "both restored");
    assert_eq!(toasts(cx), ["Operation failed"]);
}

#[gpui::test]
fn a_right_click_in_the_rename_field_keeps_the_task_menu_shut(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    press(cx, "task-a0", MouseButton::Left, 2);
    assert!(editing(cx));
    press(cx, "task-rename", MouseButton::Right, 1);
    assert!(editing(cx), "still renaming");
    assert!(cx.debug_bounds("menu-row-Rename").is_none(), "no task menu");
}
