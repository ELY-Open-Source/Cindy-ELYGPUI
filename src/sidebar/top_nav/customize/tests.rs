use gpui::{
    KeyUpEvent, Keystroke, Modifiers, MouseButton, Pixels, Point, TestAppContext,
    VisualTestContext, point, px,
};

use super::{Draft, ITEMS};
use crate::{
    sessions::{
        search,
        view_tests::{click, open},
    },
    sidebar::top_nav::more::More,
};

#[test]
fn arrows_move_one_place_within_the_list() {
    let mut d = Draft::default();
    d.move_by("automations", false);
    assert_eq!(d.order, ITEMS, "nothing above the first");
    d.move_by("automations", true);
    assert_eq!(d.order, ["plugins", "automations", "search"]);
    d.move_by("search", true);
    d.move_by("search", false);
    assert_eq!(d.order, ["plugins", "search", "automations"]);
}

#[test]
fn a_toggle_flips_one_entry() {
    let mut d = Draft::default();
    d.toggle("plugins");
    assert_eq!(d.visible, ["automations", "search"]);
    d.toggle("plugins");
    assert!(d.visible.contains(&"plugins"));
}

fn drawn(cx: &mut VisualTestContext, selector: &'static str) -> bool {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.debug_bounds(selector).is_some()
}

fn draft(cx: &mut VisualTestContext) -> Option<Draft> {
    cx.update(|_, cx| cx.global::<More>().customize.clone())
}

fn customize(cx: &mut TestAppContext) -> (tempfile::TempDir, &mut VisualTestContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    click(cx, "nav-more");
    click(cx, "nav-more-customize");
    (dir, cx)
}

fn center(cx: &mut VisualTestContext, selector: &'static str) -> Point<Pixels> {
    cx.debug_bounds(selector).unwrap().center()
}

fn visible(cx: &mut VisualTestContext) -> Vec<&'static str> {
    draft(cx).unwrap().visible
}

#[gpui::test]
fn customize_takes_the_rows_place_until_done(cx: &mut TestAppContext) {
    let (_dir, cx) = customize(cx);
    assert!(drawn(cx, "nav-customize"));
    assert!(!drawn(cx, "nav-more") && !drawn(cx, "nav-search-open"));
    click(cx, "nav-customize-done");
    assert_eq!(draft(cx), None, "Done stores nothing without a data owner");
    assert!(!drawn(cx, "nav-customize"));
    assert!(drawn(cx, "nav-more"), "the rows are back");
}

#[gpui::test]
fn the_row_and_its_checkbox_each_toggle_once(cx: &mut TestAppContext) {
    let (_dir, cx) = customize(cx);
    click(cx, "nav-customize-row-plugins");
    assert_eq!(visible(cx), ["automations", "search"]);
    let row = cx.debug_bounds("nav-customize-row-plugins").unwrap();
    let check = point(row.left() + px(14.0), row.center().y);
    cx.simulate_click(check, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(visible(cx), ["automations", "search", "plugins"]);
    let focused = cx.update(|window, cx| window.focused(cx));
    assert!(focused.is_some(), "the checkbox takes focus");
    cx.simulate_keystrokes("space");
    let keystroke = Keystroke::parse("space").unwrap();
    cx.simulate_event(KeyUpEvent { keystroke });
    cx.run_until_parked();
    assert_eq!(
        visible(cx),
        ["automations", "search"],
        "its key toggles too"
    );
}

#[gpui::test]
fn the_grip_moves_on_arrows_and_does_not_toggle(cx: &mut TestAppContext) {
    let (_dir, cx) = customize(cx);
    let row = cx.debug_bounds("nav-customize-row-search").unwrap();
    let grip = point(row.right() - px(18.0), row.center().y);
    cx.simulate_click(grip, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(visible(cx), ITEMS, "a click on the grip toggles nothing");
    cx.simulate_keystrokes("up");
    assert_eq!(
        draft(cx).unwrap().order,
        ["automations", "search", "plugins"]
    );
    cx.simulate_keystrokes("up up");
    assert_eq!(
        draft(cx).unwrap().order,
        ["search", "automations", "plugins"]
    );
}

#[gpui::test]
fn a_drag_reorders_the_draft_and_reset_restores_it(cx: &mut TestAppContext) {
    let (_dir, cx) = customize(cx);
    cx.update(|window, _| window.activate_window());
    let start = center(cx, "nav-customize-row-automations");
    let end = center(cx, "nav-customize-row-search");
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    let nudge = start + point(px(0.0), px(5.0));
    cx.simulate_mouse_move(nudge, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(150));
    cx.run_until_parked();
    let top = |cx: &mut VisualTestContext, s| cx.debug_bounds(s).unwrap().top();
    let (search, plugins) = (
        top(cx, "nav-customize-row-search"),
        top(cx, "nav-customize-row-plugins"),
    );
    assert!(
        search < end.y && plugins < search,
        "the rows made way before the drop"
    );
    cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        draft(cx).unwrap().order,
        ["plugins", "search", "automations"]
    );
    assert_eq!(visible(cx), ITEMS, "a drag toggles nothing");
    click(cx, "nav-customize-row-search");
    click(cx, "nav-customize-reset");
    assert_eq!(draft(cx), Some(Draft::default()));
}

#[gpui::test]
fn a_press_on_the_checkbox_does_not_drag(cx: &mut TestAppContext) {
    let (_dir, cx) = customize(cx);
    cx.update(|window, _| window.activate_window());
    let row = cx.debug_bounds("nav-customize-row-automations").unwrap();
    let start = point(row.left() + px(14.0), row.center().y);
    let end = center(cx, "nav-customize-row-search");
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(draft(cx).unwrap().order, ITEMS);
}

#[gpui::test]
fn a_press_on_the_panel_clears_a_query_it_was_opened_over(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    click(cx, "nav-search-open");
    cx.simulate_input("plan");
    let more = cx.debug_bounds("nav-more").unwrap().center();
    cx.simulate_mouse_move(more, None, Modifiers::none());
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(90));
    cx.run_until_parked();
    click(cx, "nav-more-customize");
    let query = |cx: &mut VisualTestContext| cx.update(|_, cx| search::query(cx).to_owned());
    assert_eq!(query(cx), "plan", "the menu's choice keeps it");
    click(cx, "nav-customize-row-plugins");
    assert_eq!(query(cx), "", "the panel is outside the search");
    assert_eq!(visible(cx), ["automations", "search"]);
}
