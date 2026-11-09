//! The rail's column over the list, as `CCAgentSidebarUpper` crossfades them.

use std::time::Duration;

use gpui::{Modifiers, TestAppContext, VisualTestContext};

use super::tests::open;

fn rail(sidebar: &gpui::Entity<super::Sidebar>, cx: &mut VisualTestContext) -> f32 {
    cx.update(|window, cx| {
        window.refresh();
        let at = cx.background_executor().now();
        sidebar.read(cx).upper.at(at).0
    })
}

fn wait(cx: &mut VisualTestContext, ms: u64) {
    cx.executor().advance_clock(Duration::from_millis(ms));
    cx.run_until_parked();
}

#[gpui::test]
fn the_rail_shows_its_column_and_new_opens_the_draft(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (row, cx) = open(cx, dir.path(), r#"{"sidebar-rail":"true"}"#);
    let sidebar = cx.update(|_, cx| row.read(cx).sidebar.clone());
    assert_eq!(rail(&sidebar, cx), 1.0, "a start on the rail takes no fade");
    let new = cx.debug_bounds("rail-new").expect("the New tile");
    assert_eq!(f32::from(new.size.width), 36.0);
    cx.simulate_click(new.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        cx.update(|_, cx| crate::router::path(cx).to_owned()),
        "/cc-agent/new"
    );
}

#[gpui::test]
fn entering_the_rail_crossfades_in_200ms(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (row, cx) = open(cx, dir.path(), "{}");
    let sidebar = cx.update(|_, cx| row.read(cx).sidebar.clone());
    assert_eq!(rail(&sidebar, cx), 0.0);
    cx.update(|_, cx| {
        sidebar.update(cx, |s, cx| {
            s.set_rail(true, cx);
            s.retarget(cx);
        })
    });
    cx.run_until_parked();
    wait(cx, 100);
    let half = rail(&sidebar, cx);
    assert!(half > 0.0 && half < 1.0, "{half}");
    wait(cx, 110);
    assert_eq!(rail(&sidebar, cx), 1.0);
}

#[gpui::test]
fn the_hidden_rail_takes_no_click(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (_row, cx) = open(cx, dir.path(), "{}");
    let before = cx.update(|_, cx| crate::router::path(cx).to_owned());
    let new = cx.debug_bounds("rail-new").expect("drawn under the list");
    cx.simulate_click(new.center(), Modifiers::none());
    cx.run_until_parked();
    let after = cx.update(|_, cx| crate::router::path(cx).to_owned());
    assert_eq!(after, before);
}

#[gpui::test]
fn the_pointer_moves_with_the_state_not_the_fade(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (row, cx) = open(cx, dir.path(), "{}");
    let sidebar = cx.update(|_, cx| row.read(cx).sidebar.clone());
    cx.update(|_, cx| {
        sidebar.update(cx, |s, cx| {
            s.set_rail(true, cx);
            s.retarget(cx);
        })
    });
    wait(cx, 16);
    assert!(rail(&sidebar, cx) < 0.5, "still fading in");
    let new = cx.debug_bounds("rail-new").unwrap();
    cx.simulate_click(new.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        cx.update(|_, cx| crate::router::path(cx).to_owned()),
        "/cc-agent/new"
    );
}

fn pressed_face(cx: &mut TestAppContext, reduced: bool) -> f32 {
    let dir = tempfile::tempdir().unwrap();
    let (_row, cx) = open(cx, dir.path(), r#"{"sidebar-rail":"true"}"#);
    cx.update(|_, cx| cx.set_global(crate::motion::ReducedMotion(reduced)));
    let at = cx.debug_bounds("rail-new").unwrap().center();
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.simulate_mouse_down(at, gpui::MouseButton::Left, Modifiers::none());
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    f32::from(cx.debug_bounds("rail-new-face").unwrap().size.width)
}

#[gpui::test]
fn a_press_insets_the_face_unless_motion_is_reduced(cx: &mut TestAppContext) {
    assert_eq!(pressed_face(cx, false), 34.0);
}

#[gpui::test]
fn reduced_motion_keeps_the_pressed_face_still(cx: &mut TestAppContext) {
    assert_eq!(pressed_face(cx, true), 36.0);
}
