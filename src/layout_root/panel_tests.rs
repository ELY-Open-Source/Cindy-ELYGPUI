//! `panelDragController.test.ts`, and the gesture driven by mouse events.

use std::time::Duration;

use gpui::{Bounds, Modifiers, MouseButton, TestAppContext, VisualTestContext, point, px, size};

use super::screen_tests::{fractions, open, open_beside, saved, tree};
use super::view::panel_drag::{drop_zone, is_droppable, is_swap};
use crate::{layout_tree::LayoutStore, layout_tree::Node, toast::Toasts};

fn rect(left: f32, top: f32, width: f32, height: f32) -> Bounds<gpui::Pixels> {
    Bounds {
        origin: point(px(left), px(top)),
        size: size(px(width), px(height)),
    }
}

#[test]
fn drop_rules_match_cindy() {
    assert!(is_droppable(&rect(0.0, 0.0, 300.0, 800.0)));
    for (w, h) in [(0.0, 800.0), (0.0, 0.0), (300.0, 0.0), (20.0, 800.0)] {
        assert!(!is_droppable(&rect(0.0, 0.0, w, h)));
    }
    let pane = rect(100.0, 100.0, 400.0, 600.0);
    assert!(is_swap(point(px(300.0), px(400.0)), &pane, 0, 1));
    assert!(!is_swap(point(px(300.0), px(400.0)), &pane, 1, 1));
    assert!(!is_swap(point(px(50.0), px(400.0)), &pane, 0, 1));
    assert_eq!(
        drop_zone(&rect(80.0, 80.0, 440.0, 640.0)),
        rect(86.0, 86.0, 428.0, 628.0)
    );
}

fn kinds(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|_, cx| {
        let Node::Split(split) = &cx.global::<LayoutStore>().layout().content else {
            panic!("row")
        };
        split
            .children
            .iter()
            .map(|c| match &c.node {
                Node::Pane(p) => p.panel_kind.clone(),
                Node::Split(s) => s.id.clone(),
            })
            .collect()
    })
}

fn toasts(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|window, cx| {
        let id = window.window_handle().window_id();
        cx.global::<Toasts>()
            .items(id)
            .iter()
            .map(|t| t.message.to_string())
            .collect()
    })
}

fn hold(cx: &mut VisualTestContext, x: f32, ms: u64) {
    cx.simulate_mouse_down(
        point(px(x), px(400.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.executor().advance_clock(Duration::from_millis(ms));
    cx.run_until_parked();
}

#[gpui::test]
fn a_long_press_drags_chat_onto_the_right_panel(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), tree(0.5, 0.5), 1660.0);
    hold(cx, 300.0, 600);
    cx.simulate_mouse_move(
        point(px(1400.0), px(400.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    assert_eq!(
        kinds(cx),
        ["chat-main", "right-tabs"],
        "nothing moves before release"
    );
    cx.simulate_mouse_up(
        point(px(1400.0), px(400.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    assert_eq!(kinds(cx), ["right-tabs", "chat-main"]);
    assert_eq!(
        saved(dir.path())["content"]["children"][0]["node"]["panelKind"],
        "right-tabs"
    );
    assert_eq!(
        toasts(cx),
        ["Panel position saved. Restore the default layout in Settings → Appearance."]
    );
    assert_eq!(
        fractions(cx).0,
        vec![0.5, 0.5],
        "fractions travel with their regions"
    );
}

#[gpui::test]
fn a_moving_or_short_press_never_drags(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), tree(0.5, 0.5), 1660.0);
    hold(cx, 300.0, 100);
    cx.simulate_mouse_move(
        point(px(309.0), px(400.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.executor().advance_clock(Duration::from_millis(600));
    cx.simulate_mouse_move(
        point(px(1400.0), px(400.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_mouse_up(
        point(px(1400.0), px(400.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    hold(cx, 300.0, 599);
    cx.simulate_mouse_up(
        point(px(300.0), px(400.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.executor().advance_clock(Duration::from_millis(100));
    cx.simulate_mouse_move(
        point(px(1400.0), px(400.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_mouse_up(
        point(px(1400.0), px(400.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    assert_eq!(kinds(cx), ["chat-main", "right-tabs"]);
    assert!(toasts(cx).is_empty());
}

#[gpui::test]
fn escape_or_a_drop_on_its_own_region_cancels(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), tree(0.5, 0.5), 1660.0);
    hold(cx, 300.0, 600);
    cx.simulate_mouse_move(
        point(px(1400.0), px(400.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_keystrokes("escape");
    cx.simulate_mouse_up(
        point(px(1400.0), px(400.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    hold(cx, 300.0, 600);
    cx.simulate_mouse_move(
        point(px(500.0), px(400.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_mouse_up(
        point(px(500.0), px(400.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    assert_eq!(kinds(cx), ["chat-main", "right-tabs"]);
    assert!(toasts(cx).is_empty());
}

#[gpui::test]
fn self_heal_waits_for_a_held_panel(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), tree(0.9, 0.1), 1660.0);
    hold(cx, 200.0, 600);
    cx.simulate_resize(size(px(800.0), px(900.0)));
    cx.simulate_mouse_move(
        point(px(250.0), px(400.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.executor().advance_clock(Duration::from_millis(1000));
    cx.run_until_parked();
    assert_eq!(
        fractions(cx).0,
        vec![0.9, 0.1],
        "a starved pane waits while a panel is held"
    );
    cx.simulate_mouse_up(
        point(px(250.0), px(400.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.executor().advance_clock(Duration::from_millis(120));
    cx.run_until_parked();
    assert!((fractions(cx).0[1] * 800.0 - 120.0).abs() < 1e-6);
}

#[gpui::test]
fn the_ghost_keeps_its_offset_at_the_window_edge(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open_beside(cx, dir.path(), tree(0.5, 0.5), 1920.0, 260.0);
    cx.simulate_mouse_down(
        point(px(500.0), px(400.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.executor().advance_clock(Duration::from_millis(600));
    cx.run_until_parked();
    cx.simulate_mouse_move(
        point(px(1915.0), px(895.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    let ghost = cx.debug_bounds("panel-drag-ghost").expect("ghost painted");
    let want = point(px(1915.0 - 85.0), px(895.0 - 110.0 * 0.58));
    let near = |a: gpui::Pixels, b: gpui::Pixels| (a - b).abs() <= px(0.5);
    assert!(
        near(ghost.origin.x, want.x) && near(ghost.origin.y, want.y),
        "{ghost:?}"
    );
    assert_eq!(ghost.size, size(px(170.0), px(110.0)));
    let zone = cx.debug_bounds("panel-drop-zone").expect("zone painted");
    assert_eq!(zone, rect(1090.0 + 6.0, 6.0, 830.0 - 12.0, 900.0 - 12.0));
}
