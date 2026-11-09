use gpui::{
    Bounds, KeyUpEvent, Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Pixels,
    TestAppContext, VisualTestContext,
};

use super::menu_key;
use crate::{
    router,
    sessions::{
        Sessions,
        view_mode::{self, Main, Pinned},
        view_tests::open,
    },
};

/// A key pressed and released.
fn key(cx: &mut VisualTestContext, key: &'static str) {
    cx.simulate_keystrokes(key);
    let keystroke = Keystroke::parse(key).unwrap();
    cx.simulate_event(KeyUpEvent { keystroke });
    cx.run_until_parked();
}

fn tabs(cx: &mut VisualTestContext, n: usize) {
    for _ in 0..n {
        key(cx, "tab");
    }
}

fn here(cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| router::path(cx).to_owned())
}

fn bounds(cx: &mut VisualTestContext, selector: &str) -> Option<Bounds<Pixels>> {
    cx.debug_bounds(Box::leak(selector.to_owned().into_boxed_str()))
}

/// The task menu's top left corner.
fn menu_at(cx: &mut VisualTestContext) -> gpui::Point<Pixels> {
    bounds(cx, "menu-panel-0").expect("the menu opened").origin
}

/// The menu opened at `at`, moved in as `snap_to_window` keeps it in the window.
fn opened_at(cx: &mut VisualTestContext, at: gpui::Point<Pixels>) -> bool {
    let panel = bounds(cx, "menu-panel-0").expect("the menu opened");
    let window = cx.update(|window, _| window.viewport_size());
    let x = at.x.min(window.width - panel.size.width);
    let y = at.y.min(window.height - panel.size.height);
    (panel.origin.x - x).abs() <= gpui::px(1.0) && (panel.origin.y - y).abs() <= gpui::px(1.0)
}

#[test]
fn the_menu_keys_follow_chromium() {
    let k = |s: &'static str| Keystroke::parse(s).unwrap();
    let windows = cfg!(target_os = "windows");
    assert_eq!(menu_key(&k("menu"), false), !windows);
    assert_eq!(menu_key(&k("menu"), true), windows);
    assert!(!menu_key(&k("ctrl-menu"), windows));
    assert!(!menu_key(&k("shift-menu"), windows));
    assert_eq!(menu_key(&k("shift-f10"), false), !cfg!(target_os = "macos"));
    assert!(!menu_key(&k("shift-f10"), true));
    assert!(!menu_key(&k("f10"), false));
    assert!(!menu_key(&k("ctrl-shift-f10"), false));
}

#[gpui::test]
fn tab_reaches_each_row_and_enter_or_space_opens_it(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    assert_eq!(here(cx), "/cc-agent/a0");
    // Each row, then its More button.
    tabs(cx, 3);
    key(cx, "enter");
    assert_eq!(here(cx), "/cc-agent/a1");
    tabs(cx, 2);
    key(cx, "space");
    assert_eq!(here(cx), "/cc-agent/a2");
}

#[gpui::test]
fn enter_on_more_opens_its_menu_not_the_task(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    tabs(cx, 4);
    key(cx, "enter");
    assert!(
        bounds(cx, "menu-row-Rename").is_some(),
        "More's menu opened"
    );
    assert_eq!(here(cx), "/cc-agent/a0", "the row did not open");
}

#[gpui::test]
fn the_menu_key_opens_the_task_menu_at_the_row_centre(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    tabs(cx, 3);
    key(cx, "menu");
    let row = bounds(cx, "task-a1").unwrap();
    assert!(opened_at(cx, row.center()), "{:?} for {row:?}", menu_at(cx));
    assert_eq!(here(cx), "/cc-agent/a0", "the menu key opens no task");
}

#[gpui::test]
fn the_menu_key_on_more_opens_the_task_menu_at_more(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    tabs(cx, 2);
    let more = bounds(cx, "task-more-a0").unwrap();
    key(cx, "menu");
    assert!(
        opened_at(cx, more.center()),
        "{:?} for {more:?}",
        menu_at(cx)
    );
}

#[gpui::test]
fn shift_f10_opens_the_task_menu_off_macos(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    tabs(cx, 1);
    key(cx, "shift-f10");
    let opened = bounds(cx, "menu-row-Rename").is_some();
    assert_eq!(opened, !cfg!(target_os = "macos"));
}

#[gpui::test]
fn a_right_click_opens_the_task_menu_at_the_pointer(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    let row = bounds(cx, "task-a1").unwrap();
    let at = gpui::point(row.origin.x + gpui::px(40.0), row.center().y);
    cx.simulate_event(MouseDownEvent {
        position: at,
        modifiers: Modifiers::none(),
        button: MouseButton::Right,
        click_count: 1,
        first_mouse: false,
    });
    cx.simulate_event(MouseUpEvent {
        position: at,
        modifiers: Modifiers::none(),
        button: MouseButton::Right,
        click_count: 1,
    });
    cx.run_until_parked();
    assert!(opened_at(cx, at), "{:?}", menu_at(cx));
}

#[gpui::test]
fn a_text_row_takes_the_menu_key(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    cx.update(|_, cx| view_mode::set_main(Main::Text, cx));
    cx.run_until_parked();
    tabs(cx, 1);
    key(cx, "menu");
    let row = bounds(cx, "task-a0").unwrap();
    assert!(opened_at(cx, row.center()), "{:?} for {row:?}", menu_at(cx));
}

#[gpui::test]
fn space_on_a_pinned_card_opens_it(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    cx.update(|window, cx| {
        let loaded = cx.global_mut::<Sessions>().loaded.as_mut().unwrap();
        loaded.sessions[2].pinned_at = Some(1);
        view_mode::set_pinned(Pinned::Card, cx);
        window.refresh();
    });
    cx.run_until_parked();
    // The view style button, then the card.
    tabs(cx, 2);
    key(cx, "space");
    assert_eq!(here(cx), "/cc-agent/a2");
}
