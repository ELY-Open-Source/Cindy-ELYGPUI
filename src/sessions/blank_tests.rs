//! `handleSidebarBlankContextMenu`: a right click on the list's blank space opens the filter menu.

use gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext, point, px};

use super::view_tests::{click, open};
use crate::preferences::Preferences;

fn right_click(cx: &mut VisualTestContext, at: gpui::Point<gpui::Pixels>) {
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

fn blank(cx: &mut VisualTestContext) -> gpui::Point<gpui::Pixels> {
    cx.debug_bounds("main-list").unwrap().bottom_left() + point(px(40.0), px(-8.0))
}

#[gpui::test]
fn a_blank_right_click_opens_the_filter_menu_at_the_pointer(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    let at = blank(cx);
    right_click(cx, at);
    let panel = cx.debug_bounds("menu-panel-0").expect("menu open");
    assert!((panel.left() - at.x).abs() < px(2.0), "{panel:?} at {at:?}");
    // Near the window's foot the menu shifts up to fit, as Radix's collision does.
    assert!(
        panel.top() <= at.y && at.y <= panel.bottom(),
        "{panel:?} at {at:?}"
    );
    click(cx, "menu-row-Group by");
    click(cx, "menu-row-Project");
    let saved = Preferences::load(dir.path()).unwrap();
    assert_eq!(
        saved.text("cc-agent.sidebar.filter.groupBy").unwrap(),
        Some("flat")
    );
}

#[gpui::test]
fn a_rows_right_click_keeps_its_own_menu(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    for selector in [
        "task-a0",
        "project:local:/work/app-header",
        "dialogue:all-header",
    ] {
        let at = cx.debug_bounds(selector).unwrap().center();
        right_click(cx, at);
        assert!(
            cx.debug_bounds("menu-row-Group by").is_none(),
            "{selector} keeps the filter menu shut"
        );
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
    }
}
