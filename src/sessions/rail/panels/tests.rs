use std::time::Duration;

use gpui::{
    Bounds, KeyUpEvent, Keystroke, Modifiers, MouseButton, Pixels, Point, TestAppContext,
    VisualTestContext, point, px, size,
};

use super::Shell;
use crate::{
    router,
    sessions::{
        Sessions,
        rail::{
            pinned::tests::{hover, rail},
            sections::{Panels, Section},
        },
        rename,
        row::Scope,
        view_tests::task,
    },
};

pub(super) fn drawn(cx: &mut VisualTestContext, selector: &str) -> Option<Bounds<Pixels>> {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.debug_bounds(Box::leak(selector.to_owned().into_boxed_str()))
}

pub(super) fn open(cx: &mut VisualTestContext) -> Option<Section> {
    cx.update(|_, cx| cx.default_global::<Panels>().open)
}

pub(super) fn wait(cx: &mut VisualTestContext, ms: u64) {
    cx.executor().advance_clock(Duration::from_millis(ms));
    cx.run_until_parked();
}

/// Twelve more dialogues, each a minute older than the last and older than d0.
fn dialogues(cx: &mut VisualTestContext) {
    cx.update(|_, cx| {
        let old = jiff::Timestamp::now().as_millisecond() - 8 * 24 * 60 * 60 * 1000;
        let loaded = cx.global_mut::<Sessions>().loaded.as_mut().unwrap();
        for i in 1..=12 {
            loaded
                .sessions
                .push(task(&format!("d{i}"), None, true, old - 60_000 * i as i64));
        }
    });
}

/// Hovers the Dialogues tile; the panel opens.
fn show(cx: &mut VisualTestContext) -> Bounds<Pixels> {
    let tile = drawn(cx, "rail-dialogues").unwrap();
    hover(cx, tile.center());
    assert_eq!(open(cx), Some(Section::Dialogues));
    tile
}

pub(super) fn away() -> Point<Pixels> {
    point(px(700.0), px(550.0))
}

#[gpui::test]
fn the_panel_sits_at_the_tile_with_its_head_and_rows(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    let tile = show(cx);
    let panel = drawn(cx, "rail-panel").expect("the panel");
    assert_eq!(panel.left(), tile.right() + px(12.0));
    assert_eq!(panel.top(), tile.top() - px(6.0));
    assert_eq!(panel.size.width, px(264.0));
    assert!(drawn(cx, "task-rail-d0").is_some(), "its own row");
    assert!(drawn(cx, "task-rail-a3").is_none(), "only dialogues");
}

#[gpui::test]
fn ten_rows_then_show_all_which_resets_with_the_panel(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    dialogues(cx);
    show(cx);
    let top = |cx: &mut VisualTestContext, id: &str| drawn(cx, &format!("task-rail-{id}"));
    assert!(
        top(cx, "d0").unwrap().top() < top(cx, "d1").unwrap().top(),
        "by recency"
    );
    assert!(
        top(cx, "d9").is_some() && top(cx, "d10").is_none(),
        "ten shown"
    );
    let more = drawn(cx, "rail-panel-show-all").unwrap();
    cx.simulate_click(more.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(top(cx, "d12").is_some(), "all shown");
    cx.simulate_keystrokes("escape");
    show(cx);
    assert!(top(cx, "d10").is_none(), "a new panel folds again");
}

#[gpui::test]
fn the_pointer_keeps_it_over_the_panel_and_leaving_closes_it(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    show(cx);
    let panel = drawn(cx, "rail-panel").unwrap();
    hover(cx, panel.center());
    wait(cx, 300);
    assert_eq!(open(cx), Some(Section::Dialogues), "inside the panel");
    hover(cx, away());
    wait(cx, 120);
    assert_eq!(open(cx), None);
}

#[gpui::test]
fn a_press_outside_closes_and_a_row_press_keeps_it(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    show(cx);
    let row = drawn(cx, "task-rail-d0").unwrap();
    cx.simulate_click(row.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        open(cx),
        Some(Section::Dialogues),
        "a row click keeps the panel"
    );
    assert_eq!(
        cx.update(|_, cx| router::path(cx).to_owned()),
        "/cc-agent/d0"
    );
    cx.simulate_click(away(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(open(cx), None);
}

#[gpui::test]
fn a_shift_click_in_the_panel_only_opens(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    show(cx);
    let row = drawn(cx, "task-rail-d0").unwrap();
    cx.simulate_click(row.center(), Modifiers::shift());
    cx.run_until_parked();
    assert_eq!(
        cx.update(|_, cx| router::path(cx).to_owned()),
        "/cc-agent/d0"
    );
    let selected = cx.update(|_, cx| crate::sessions::select::is_selected("d0", cx));
    assert!(!selected, "selection stays in the sidebar");
}

pub(super) fn enter(cx: &mut VisualTestContext) {
    cx.simulate_keystrokes("enter");
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").unwrap(),
    });
    cx.run_until_parked();
}

#[gpui::test]
fn a_keyboard_open_moves_focus_in_and_escape_hands_it_back(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    let tile = drawn(cx, "rail-dialogues").unwrap();
    cx.simulate_click(tile.center(), Modifiers::none());
    cx.run_until_parked();
    enter(cx);
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let (opener, inside) = cx.update(|window, cx| {
        let opener = cx
            .global::<Panels>()
            .opener
            .clone()
            .expect("a keyboard opener");
        let shell = cx.global::<Shell>().focus[0].clone().unwrap();
        (
            opener,
            shell.contains_focused(window, cx) && !shell.is_focused(window),
        )
    });
    assert!(inside, "focus moved to the panel's first stop");
    hover(cx, away());
    wait(cx, 300);
    assert_eq!(open(cx), Some(Section::Dialogues), "no hover close");
    cx.simulate_keystrokes("escape");
    assert_eq!(open(cx), None);
    assert!(
        cx.update(|window, _| opener.is_focused(window)),
        "back on the tile"
    );
}

#[gpui::test]
fn a_tile_out_of_view_closes_the_panel(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    show(cx);
    cx.simulate_resize(size(px(800.0), px(200.0)));
    cx.run_until_parked();
    assert_eq!(open(cx), None, "the frame that hides the tile closes it");
    assert!(drawn(cx, "rail-panel").is_none());
}

/// Focuses a tile with a click, then opens it from the keyboard.
pub(super) fn key_open(cx: &mut VisualTestContext, id: &str) {
    let tile = drawn(cx, id).unwrap();
    cx.simulate_click(tile.center(), Modifiers::none());
    cx.run_until_parked();
    enter(cx);
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

#[gpui::test]
fn a_keyboard_opened_projects_panel_closes_on_a_press_outside(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    key_open(cx, "rail-projects");
    assert_eq!(open(cx), Some(Section::Projects));
    cx.simulate_click(away(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(open(cx), None);
}

#[gpui::test]
fn an_empty_panel_leaves_focus_on_its_tile(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    cx.update(|_, cx| {
        let loaded = cx.global_mut::<Sessions>().loaded.as_mut().unwrap();
        loaded.sessions.retain(|s| s.id != "d0");
    });
    key_open(cx, "rail-dialogues");
    assert_eq!(open(cx), Some(Section::Dialogues));
    let kept = cx.update(|window, cx| {
        let opener = cx.global::<Panels>().opener.clone().unwrap();
        opener.is_focused(window)
    });
    assert!(kept, "no stop inside, so focus stays");
}

#[gpui::test]
fn show_all_is_a_keyboard_button(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    dialogues(cx);
    key_open(cx, "rail-dialogues");
    // From the first row: ten rows' Mores and nine rows, then Show all.
    for _ in 0..20 {
        cx.simulate_keystrokes("tab");
    }
    enter(cx);
    assert!(drawn(cx, "task-rail-d12").is_some(), "all shown");
}

fn right_click(cx: &mut VisualTestContext, at: Point<Pixels>) {
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
}

#[gpui::test]
fn a_row_menu_holds_the_panel_until_a_press_lands_outside(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    show(cx);
    let row = drawn(cx, "task-rail-d0").unwrap();
    right_click(cx, row.center());
    hover(cx, away());
    wait(cx, 300);
    assert_eq!(open(cx), Some(Section::Dialogues), "the menu holds it");
    let item = drawn(cx, "menu-row-Copy session link").expect("the menu");
    cx.simulate_click(item.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(open(cx), Some(Section::Dialogues), "a choice stays inside");
    right_click(cx, row.center());
    cx.simulate_click(away(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(open(cx), None, "a press past the menu closes both");
}

#[gpui::test]
fn a_rename_in_the_panel_is_the_panel_rows_own(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    show(cx);
    let row = drawn(cx, "task-rail-d0").unwrap();
    cx.simulate_click(row.center(), Modifiers::none());
    cx.simulate_event(gpui::MouseDownEvent {
        position: row.center(),
        button: MouseButton::Left,
        modifiers: Modifiers::none(),
        click_count: 2,
        first_mouse: false,
    });
    cx.simulate_event(gpui::MouseUpEvent {
        position: row.center(),
        button: MouseButton::Left,
        modifiers: Modifiers::none(),
        click_count: 2,
    });
    cx.run_until_parked();
    let (rail, sidebar) = cx.update(|_, cx| {
        (
            rename::editing("d0", Scope::Rail, cx).is_some(),
            rename::editing("d0", Scope::Sidebar, cx).is_some(),
        )
    });
    assert!(rail && !sidebar);
    hover(cx, away());
    wait(cx, 300);
    assert_eq!(open(cx), Some(Section::Dialogues), "editing holds it");
    cx.simulate_keystrokes("escape");
    assert_eq!(open(cx), Some(Section::Dialogues), "the field takes Escape");
}
