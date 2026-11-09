use gpui::{Bounds, Modifiers, Pixels, TestAppContext, VisualTestContext, px};

use super::super::{
    Shell,
    tests::{away, drawn, enter, key_open, open, wait},
};
use crate::sessions::{
    Sessions,
    rail::{
        pinned::tests::{hover, rail},
        sections::{Panels, Section},
    },
    view_tests::task,
};

const APP: &str = "rail-project-local:/work/app";

/// The open project's key.
fn project(cx: &mut VisualTestContext) -> Option<String> {
    cx.update(|_, cx| {
        cx.default_global::<Panels>()
            .project
            .as_ref()
            .map(|o| o.key.clone())
    })
}

/// Adds `n` more tasks to /work/app, each older than a6, and a one-task project /work/b.
fn add(cx: &mut VisualTestContext, n: usize) {
    cx.update(|_, cx| {
        let old = jiff::Timestamp::now().as_millisecond() - 9 * 24 * 60 * 60 * 1000;
        let loaded = cx.global_mut::<Sessions>().loaded.as_mut().unwrap();
        for i in 0..n {
            let at = old - 60_000 * i as i64;
            loaded
                .sessions
                .push(task(&format!("m{i}"), Some("/work/app"), false, at));
        }
        loaded
            .sessions
            .push(task("b0", Some("/work/b"), false, old));
    });
}

/// Opens Projects by hover, then the app project's level by hovering its row.
fn show(cx: &mut VisualTestContext) -> Bounds<Pixels> {
    let tile = drawn(cx, "rail-projects").unwrap();
    hover(cx, tile.center());
    let row = drawn(cx, APP).expect("the project row");
    hover(cx, row.center());
    assert_eq!(project(cx).as_deref(), Some("local:/work/app"));
    row
}

#[gpui::test]
fn a_row_hover_opens_its_tasks_beside_it(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    let row = show(cx);
    let level = drawn(cx, "rail-project-panel").expect("the level");
    let gap = level.left() - row.right();
    assert!(
        (gap - px(8.0)).abs() < px(1.0),
        "8px right of the row: {gap:?}"
    );
    let rise = row.top() - level.top();
    assert!(
        (rise - px(6.0)).abs() < px(1.0),
        "6px above the row's top: {rise:?}"
    );
    assert!(drawn(cx, "task-rail-a3").is_some(), "its unpinned task");
    assert!(drawn(cx, "task-rail-a0").is_none(), "pins stay out");
}

#[gpui::test]
fn the_level_keeps_while_the_pointer_is_on_it_and_closes_alone_off_it(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    show(cx);
    let level = drawn(cx, "rail-project-panel").unwrap();
    hover(cx, level.center());
    wait(cx, 300);
    assert!(project(cx).is_some(), "on the level");
    // The head of the first level: inside the panel, off every project row.
    let panel = drawn(cx, "rail-panel").unwrap();
    hover(cx, panel.origin + gpui::point(px(20.0), px(12.0)));
    wait(cx, 119);
    assert!(project(cx).is_some(), "within the grace");
    wait(cx, 1);
    assert_eq!(project(cx), None);
    assert_eq!(open(cx), Some(Section::Projects), "the first level stays");
    assert!(drawn(cx, "rail-project-panel").is_none());
}

#[gpui::test]
fn another_row_switches_the_level_and_resets_its_fold(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    add(cx, 3);
    show(cx);
    // a3 to a6, then m0: five, then Show all.
    assert!(drawn(cx, "task-rail-m0").is_some() && drawn(cx, "task-rail-m1").is_none());
    let more = drawn(cx, "rail-project-show-all").unwrap();
    cx.simulate_click(more.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(drawn(cx, "task-rail-m2").is_some(), "all shown");
    let b = drawn(cx, "rail-project-local:/work/b").unwrap();
    hover(cx, b.center());
    assert_eq!(project(cx).as_deref(), Some("local:/work/b"));
    assert!(drawn(cx, "task-rail-b0").is_some());
    let app = drawn(cx, APP).unwrap();
    hover(cx, app.center());
    assert!(drawn(cx, "task-rail-m1").is_none(), "a switch folds again");
}

#[gpui::test]
fn a_press_on_the_level_keeps_both_and_outside_closes_both(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    show(cx);
    let task = drawn(cx, "task-rail-a3").unwrap();
    cx.simulate_click(task.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(project(cx).is_some());
    cx.simulate_click(away(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!((open(cx), project(cx)), (None, None));
}

#[gpui::test]
fn a_keyboard_open_moves_focus_in_and_takes_no_grace(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    key_open(cx, "rail-projects");
    enter(cx);
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert_eq!(project(cx).as_deref(), Some("local:/work/app"));
    let inside = cx.update(|window, cx| {
        let level = cx.global::<Shell>().focus[1].clone().unwrap();
        level.contains_focused(window, cx) && !level.is_focused(window)
    });
    assert!(inside, "focus moved to the level's first task");
    hover(cx, away());
    wait(cx, 300);
    assert!(project(cx).is_some(), "no hover close");
    cx.simulate_keystrokes("escape");
    assert_eq!((open(cx), project(cx)), (None, None));
}

/// Opens Projects by hover, then the app project's level from the keyboard on its row.
fn key_level(cx: &mut VisualTestContext) -> Bounds<Pixels> {
    let row = show(cx);
    cx.simulate_click(row.center(), Modifiers::none());
    cx.run_until_parked();
    enter(cx);
    let keyboard = cx.update(|_, cx| cx.global::<Panels>().project.as_ref().unwrap().via_keyboard);
    assert!(keyboard, "Enter on the row");
    row
}

#[gpui::test]
fn a_move_within_the_row_keeps_a_keyboard_level(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    let row = key_level(cx);
    hover(cx, row.center() + gpui::point(px(3.0), px(1.0)));
    let panel = drawn(cx, "rail-panel").unwrap();
    hover(cx, panel.origin + gpui::point(px(20.0), px(12.0)));
    wait(cx, 300);
    assert!(
        project(cx).is_some(),
        "the row's hover took no key press as a leave"
    );
}

#[gpui::test]
fn a_tile_click_resets_a_keyboard_level_and_hands_focus_to_its_row(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    key_level(cx);
    let opener = cx.update(|_, cx| cx.global::<Panels>().project_opener.clone().unwrap());
    let tile = drawn(cx, "rail-projects").unwrap();
    cx.simulate_click(tile.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!((open(cx), project(cx)), (Some(Section::Projects), None));
    assert!(
        cx.update(|window, _| opener.is_focused(window)),
        "back on the row"
    );
}
