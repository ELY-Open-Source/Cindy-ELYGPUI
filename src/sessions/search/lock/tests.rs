//! Search in a project or the dialogues: the lock, its chip, the scope and the frozen list.

use gpui::{
    Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, ScrollDelta, ScrollEvent, TestAppContext,
    TouchPhase, VisualTestContext, point, px, size,
};

use super::Lock;
use crate::sessions::{
    Sessions,
    reveal::Reveal,
    search::{Which, field, of, projects},
    view_tests::{click, open},
};

const APP: &str = "local:/work/app";

fn right_click(cx: &mut VisualTestContext, selector: &'static str) {
    let at = cx.debug_bounds(selector).unwrap().center();
    cx.simulate_event(MouseDownEvent {
        position: at,
        modifiers: Modifiers::none(),
        button: MouseButton::Right,
        click_count: 1,
        first_mouse: false,
    });
    cx.run_until_parked();
}

fn in_project(cx: &mut VisualTestContext) {
    right_click(cx, "project:local:/work/app-header");
    click(cx, "menu-row-Search sessions in this project");
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

fn lock(cx: &mut VisualTestContext) -> Option<Lock> {
    cx.update(|_, cx| of(Which::Inline, cx).options.lock.clone())
}

fn selection(cx: &mut VisualTestContext) -> Option<Vec<String>> {
    cx.update(|_, cx| of(Which::Inline, cx).options.projects.clone())
}

fn scope(cx: &mut VisualTestContext) -> Option<Vec<String>> {
    cx.update(|_, cx| projects::scope(&of(Which::Inline, cx).options, cx))
}

fn drawn(cx: &mut VisualTestContext, selector: &'static str) -> bool {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.debug_bounds(selector).is_some()
}

fn app_ids() -> Vec<String> {
    (0..7).map(|i| format!("a{i}")).collect()
}

#[gpui::test]
fn the_project_menu_locks_the_search_to_it(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    in_project(cx);
    let Some(Lock::Project { key, name, ids }) = lock(cx) else {
        panic!("no project lock");
    };
    assert_eq!((key.as_str(), name.as_str(), ids), (APP, "app", app_ids()));
    assert_eq!(selection(cx), Some(vec![APP.to_owned()]));
    assert_eq!(scope(cx), Some(app_ids()));
    let focused = cx.update(|window, cx| {
        let input = field(Which::Inline, window, cx);
        input.read(cx).focus().is_focused(window)
    });
    assert!(focused, "the field takes focus");
    assert!(drawn(cx, "nav-search-chip"));
    let count = cx.update(|_, cx| of(Which::Inline, cx).options.active_count());
    assert_eq!(count, 1);
}

#[gpui::test]
fn the_chip_clears_the_lock(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    in_project(cx);
    click(cx, "nav-search-unlock");
    assert_eq!(lock(cx), None);
    assert_eq!(selection(cx), None);
    assert!(!drawn(cx, "nav-search-chip"));
}

#[gpui::test]
fn the_dialogues_search_keeps_to_their_tasks(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    right_click(cx, "dialogue:all-header");
    click(cx, "menu-row-Search This Chat Group");
    let group = Lock::Group {
        name: "Chat".into(),
        ids: vec!["d0".into()],
    };
    assert_eq!(lock(cx), Some(group));
    assert_eq!(selection(cx), None, "a group selects no project");
    assert_eq!(scope(cx), Some(vec!["d0".to_owned()]));
    let count = cx.update(|_, cx| of(Which::Inline, cx).options.active_count());
    assert_eq!(count, 1, "the group counts as a filter");
}

#[gpui::test]
fn locked_project_rows_do_not_choose(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    in_project(cx);
    click(cx, "search-filter-button");
    click(cx, "menu-row-Projects");
    click(cx, "menu-row-All projects");
    assert_eq!(selection(cx), Some(vec![APP.to_owned()]));
}

#[gpui::test]
fn reset_keeps_the_locked_project(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    in_project(cx);
    click(cx, "search-filter-button");
    click(cx, "menu-row-Status");
    click(cx, "menu-row-Archived");
    click(cx, "search-filter-button");
    click(cx, "menu-row-Reset");
    assert_eq!(selection(cx), Some(vec![APP.to_owned()]));
    assert!(lock(cx).is_some());
    let status = cx.update(|_, cx| of(Which::Inline, cx).options.status);
    assert_eq!(status, "all");
}

#[gpui::test]
fn a_hidden_project_lets_the_lock_go(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    in_project(cx);
    cx.update(|_, cx| {
        let loaded = cx.global_mut::<Sessions>().loaded.as_mut().unwrap();
        loaded.settings.hidden = vec!["/work/app".into()];
        projects::loaded(cx);
    });
    assert_eq!(lock(cx), None);
    assert_eq!(selection(cx), None);
}

#[gpui::test]
fn an_unknown_project_keeps_its_own_ids_until_the_catalogue_loads(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    let ghost = || Lock::Project {
        key: "local:/gone".into(),
        name: "gone".into(),
        ids: vec!["z".into(), "y".into(), "z".into()],
    };
    cx.update(|window, cx| {
        cx.global_mut::<Sessions>().loaded.as_mut().unwrap().known = Vec::new();
        super::request(ghost(), window, cx);
    });
    assert_eq!(scope(cx), Some(vec!["y".to_owned(), "z".to_owned()]));
    cx.update(|_, cx| {
        let loaded = cx.global_mut::<Sessions>().loaded.as_mut().unwrap();
        loaded.known = projects::known(&loaded.sessions, &[], &[], &[]);
        projects::loaded(cx);
    });
    assert_eq!(lock(cx), None, "a known catalogue without it releases it");
}

fn offset(cx: &mut VisualTestContext) -> f32 {
    cx.update(|_, cx| f32::from(cx.global::<Reveal>().handle.offset().y))
}

/// The list in a short window, scrolled down 60.
fn scrolled<'a>(cx: &'a mut TestAppContext, dir: &std::path::Path) -> &'a mut VisualTestContext {
    let cx = open(cx, dir);
    cx.simulate_resize(size(px(400.0), px(200.0)));
    cx.run_until_parked();
    let list = cx.debug_bounds("main-list").unwrap();
    cx.simulate_event(ScrollEvent {
        position: list.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-60.0))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
        ..Default::default()
    });
    cx.run_until_parked();
    assert_eq!(offset(cx), -60.0);
    cx
}

fn ask(cx: &mut VisualTestContext) {
    let scope = Lock::Project {
        key: APP.into(),
        name: "app".into(),
        ids: app_ids(),
    };
    cx.update(|window, cx| super::request(scope, window, cx));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

#[gpui::test]
fn the_frozen_list_comes_back_after_a_press_outside(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = scrolled(cx, dir.path());
    ask(cx);
    assert_eq!(offset(cx), 0.0, "the search row shows");
    let at = cx.debug_bounds("main-list").unwrap().bottom_left() + point(px(4.0), px(-4.0));
    let press = MouseDownEvent {
        position: at,
        modifiers: Modifiers::none(),
        button: MouseButton::Left,
        click_count: 1,
        first_mouse: false,
    };
    cx.simulate_event(press);
    cx.run_until_parked();
    assert_eq!(offset(cx), 0.0, "held until the pointer lifts");
    cx.simulate_event(MouseUpEvent {
        position: at,
        modifiers: Modifiers::none(),
        button: MouseButton::Left,
        click_count: 1,
    });
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert_eq!(offset(cx), -60.0);
}

#[gpui::test]
fn the_frozen_list_comes_back_when_focus_leaves(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = scrolled(cx, dir.path());
    ask(cx);
    click(cx, "search-filter-button");
    click(cx, "search-filter-button");
    assert_eq!(offset(cx), 0.0, "the row's own stops keep it");
    cx.update(|window, cx| {
        let elsewhere = cx.focus_handle().tab_stop(true);
        window.focus(&elsewhere, cx);
        std::mem::forget(elsewhere);
    });
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert_eq!(offset(cx), -60.0);
}

#[gpui::test]
fn a_press_in_the_row_keeps_the_list_frozen(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = scrolled(cx, dir.path());
    ask(cx);
    click(cx, "nav-search");
    assert_eq!(offset(cx), 0.0);
}

#[gpui::test]
fn the_menu_freezes_the_list_with_the_field_focused(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = scrolled(cx, dir.path());
    in_project(cx);
    assert_eq!(offset(cx), 0.0);
    cx.simulate_input("a");
    cx.simulate_keystrokes("backspace");
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert_eq!(offset(cx), -60.0, "a cleared query puts the list back");
}

fn press_below(cx: &mut VisualTestContext) {
    let at = cx.debug_bounds("main-list").unwrap().bottom_left() + point(px(4.0), px(-4.0));
    cx.simulate_event(MouseDownEvent {
        position: at,
        modifiers: Modifiers::none(),
        button: MouseButton::Left,
        click_count: 1,
        first_mouse: false,
    });
    cx.run_until_parked();
    assert_eq!(offset(cx), 0.0, "held until the pointer lifts");
    cx.simulate_event(MouseUpEvent {
        position: at,
        modifiers: Modifiers::none(),
        button: MouseButton::Left,
        click_count: 1,
    });
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

#[gpui::test]
fn a_press_past_the_open_menu_puts_the_list_back(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = scrolled(cx, dir.path());
    ask(cx);
    click(cx, "search-filter-button");
    assert!(drawn(cx, "menu-panel-0"));
    press_below(cx);
    assert!(!drawn(cx, "menu-panel-0"), "the press closed the menu");
    assert_eq!(offset(cx), -60.0);
}

#[gpui::test]
fn a_menu_choice_keeps_the_list_frozen(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = scrolled(cx, dir.path());
    ask(cx);
    click(cx, "search-filter-button");
    click(cx, "menu-row-Status");
    click(cx, "menu-row-Archived");
    assert_eq!(offset(cx), 0.0);
}

#[gpui::test]
fn the_chip_x_stays_lit_while_typing(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    in_project(cx);
    let at = cx.debug_bounds("nav-search-unlock").unwrap().center();
    cx.simulate_mouse_move(at, None, Modifiers::none());
    assert!(drawn(cx, "nav-search-unlock-hover"));
    cx.simulate_input("a");
    assert!(
        drawn(cx, "nav-search-unlock-hover"),
        "a key press keeps the hover"
    );
}
