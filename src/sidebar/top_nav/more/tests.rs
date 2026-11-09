use std::time::Duration;

use gpui::{Modifiers, MouseMoveEvent, TestAppContext, VisualTestContext, point, px};

use super::{More, place};
use crate::sessions::view_tests::{click, open};

fn drawn(cx: &mut VisualTestContext, selector: &'static str) -> bool {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.debug_bounds(selector).is_some()
}

fn move_to(cx: &mut VisualTestContext, at: gpui::Point<gpui::Pixels>) {
    cx.simulate_event(MouseMoveEvent {
        position: at,
        modifiers: Modifiers::none(),
        pressed_button: None,
    });
    cx.run_until_parked();
}

fn wait(cx: &mut VisualTestContext, ms: u64) {
    cx.executor().advance_clock(Duration::from_millis(ms));
    cx.run_until_parked();
}

#[gpui::test]
fn hover_opens_more_beside_its_row_without_focus(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    let row = cx.debug_bounds("nav-more").expect("More follows Search");
    let before = cx.update(|window, cx| window.focused(cx));
    move_to(cx, row.center());
    wait(cx, 90);
    assert!(drawn(cx, "nav-more-menu"));
    let menu = cx.debug_bounds("nav-more-menu").unwrap();
    assert!(
        menu.right() <= px(392.0) && menu.top() >= row.top(),
        "{menu:?} {row:?}"
    );
    let after = cx.update(|window, cx| window.focused(cx));
    assert_eq!(after, before, "hover takes no focus");
    move_to(cx, point(px(390.0), px(890.0)));
    wait(cx, 160);
    assert!(!drawn(cx, "nav-more-menu"));
}

#[gpui::test]
fn a_click_opens_and_a_second_click_keeps_it_open(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    click(cx, "nav-more");
    assert!(drawn(cx, "nav-more-menu"));
    click(cx, "nav-more");
    assert!(drawn(cx, "nav-more-menu"), "a click only opens");
}

#[gpui::test]
fn customize_closes_the_menu(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    click(cx, "nav-more");
    click(cx, "nav-more-customize");
    assert!(!drawn(cx, "nav-more-menu"));
    assert!(cx.update(|_, cx| cx.global::<More>().customizing));
}

#[gpui::test]
fn a_press_outside_or_escape_closes_it(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    click(cx, "nav-more");
    cx.simulate_click(point(px(200.0), px(880.0)), Modifiers::none());
    cx.run_until_parked();
    assert!(!drawn(cx, "nav-more-menu"));
    click(cx, "nav-more");
    cx.simulate_keystrokes("escape");
    assert!(!drawn(cx, "nav-more-menu"));
}

#[test]
fn the_menu_sits_right_and_flips_left_without_room() {
    let row = gpui::Bounds::new(point(px(12.0), px(100.0)), gpui::size(px(236.0), px(32.0)));
    let (gap, top) = (px(8.0), px(58.0));
    assert_eq!(
        place(row, px(172.0), px(1200.0), gap, top),
        point(px(256.0), px(100.0))
    );
    let far = gpui::Bounds::new(point(px(900.0), px(20.0)), gpui::size(px(236.0), px(32.0)));
    assert_eq!(
        place(far, px(172.0), px(1200.0), gap, top),
        point(px(720.0), px(58.0))
    );
}

fn hover_open(cx: &mut VisualTestContext) {
    let row = cx.debug_bounds("nav-more").unwrap();
    move_to(cx, row.center());
    wait(cx, 90);
    assert!(drawn(cx, "nav-more-menu"));
}

#[gpui::test]
fn keys_under_a_still_pointer_keep_it_open(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    hover_open(cx);
    cx.simulate_keystrokes("a");
    wait(cx, 200);
    assert!(drawn(cx, "nav-more-menu"));
}

#[gpui::test]
fn escape_closes_a_hover_open_while_focus_is_elsewhere(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    click(cx, "nav-search-open");
    hover_open(cx);
    cx.simulate_keystrokes("escape");
    assert!(!drawn(cx, "nav-more-menu"));
}

#[gpui::test]
fn arrow_down_opens_onto_customize_and_enter_chooses_it(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    click(cx, "nav-more");
    cx.simulate_keystrokes("escape");
    cx.simulate_keystrokes("down");
    assert!(drawn(cx, "nav-more-menu"), "the focused trigger opened it");
    cx.simulate_keystrokes("escape");
    click(cx, "nav-more");
    cx.simulate_keystrokes("down");
    cx.simulate_keystrokes("enter");
    let keystroke = gpui::Keystroke::parse("enter").unwrap();
    cx.simulate_event(gpui::KeyUpEvent { keystroke });
    cx.run_until_parked();
    assert!(cx.update(|_, cx| cx.global::<More>().customizing));
}

#[gpui::test]
fn a_press_on_the_menu_keeps_a_running_query(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    click(cx, "nav-search-open");
    cx.simulate_input("plan");
    hover_open(cx);
    let menu = cx.debug_bounds("nav-more-menu").unwrap();
    cx.simulate_click(
        point(menu.left() + px(2.0), menu.top() + px(2.0)),
        Modifiers::none(),
    );
    cx.run_until_parked();
    let query = cx.update(|_, cx| crate::sessions::search::query(cx).to_owned());
    assert_eq!(query, "plan");
}

#[gpui::test]
fn a_press_on_the_open_trigger_moves_no_focus(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    let before = cx.update(|window, cx| window.focused(cx));
    hover_open(cx);
    click(cx, "nav-more");
    assert!(drawn(cx, "nav-more-menu"));
    assert_eq!(cx.update(|window, cx| window.focused(cx)), before);
}

#[gpui::test]
fn tab_and_shift_tab_stay_in_the_menu(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    click(cx, "nav-more");
    cx.simulate_keystrokes("down");
    let item = cx.update(|window, cx| window.focused(cx));
    for keys in ["tab", "shift-tab"] {
        cx.simulate_keystrokes(keys);
        assert_eq!(cx.update(|window, cx| window.focused(cx)), item, "{keys}");
        assert!(drawn(cx, "nav-more-menu"), "{keys}");
    }
}
