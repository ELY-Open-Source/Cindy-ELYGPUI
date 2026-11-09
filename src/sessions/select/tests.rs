use gpui::{
    Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, TestAppContext, VisualTestContext,
};

use super::{Drawn, visible};
use crate::{
    router,
    sessions::{
        Sessions,
        view_mode::{self, Pinned},
        view_tests::{click, open},
    },
};

fn press(cx: &mut VisualTestContext, id: &str, modifiers: Modifiers) {
    let selector: &'static str = Box::leak(format!("task-{id}").into_boxed_str());
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} not drawn"));
    let at = gpui::point(bounds.origin.x + gpui::px(20.0), bounds.center().y);
    cx.simulate_event(MouseDownEvent {
        position: at,
        modifiers,
        button: MouseButton::Left,
        click_count: 1,
        first_mouse: false,
    });
    cx.simulate_event(MouseUpEvent {
        position: at,
        modifiers,
        button: MouseButton::Left,
        click_count: 1,
    });
    cx.run_until_parked();
}

fn selected(cx: &mut VisualTestContext) -> Vec<String> {
    let mut ids: Vec<String> = cx.update(|_, cx| {
        let ids = &cx.global::<Sessions>().selection.ids;
        ids.iter().cloned().collect()
    });
    ids.sort();
    ids
}

fn shown(cx: &mut VisualTestContext, selector: &'static str) -> bool {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.debug_bounds(selector).is_some()
}

fn here(cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| router::path(cx).to_owned())
}

fn shift_mod() -> Modifiers {
    Modifiers {
        shift: true,
        control: true,
        ..Modifiers::none()
    }
}

#[gpui::test]
fn a_mod_click_toggles_and_the_bar_counts(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    assert!(!shown(cx, "bulk-count"), "no bar without a selection");
    press(cx, "a1", Modifiers::control());
    press(cx, "a2", Modifiers::command());
    assert_eq!(selected(cx), ["a1", "a2"]);
    assert!(shown(cx, "task-selected-a1") && shown(cx, "bulk-count"));
    press(cx, "a1", Modifiers::control());
    assert_eq!(selected(cx), ["a2"]);
    assert!(!shown(cx, "task-selected-a1"));
    assert_eq!(here(cx), "/cc-agent/a0", "a selection click opens nothing");
}

#[gpui::test]
fn shift_spans_the_drawn_rows_from_the_anchor(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    press(cx, "a1", Modifiers::none());
    press(cx, "a3", Modifiers::shift());
    assert_eq!(selected(cx), ["a1", "a2", "a3"]);
    press(cx, "a1", Modifiers::control());
    press(cx, "a4", shift_mod());
    assert_eq!(
        selected(cx),
        ["a1", "a2", "a3", "a4"],
        "added to what stayed"
    );
    press(cx, "a2", Modifiers::shift());
    assert_eq!(selected(cx), ["a1", "a2"], "replaced, the anchor kept");
    assert_eq!(here(cx), "/cc-agent/a1");
}

#[gpui::test]
fn the_first_shift_click_anchors_itself(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    press(cx, "a2", Modifiers::shift());
    assert_eq!(selected(cx), ["a2"]);
    press(cx, "a4", Modifiers::shift());
    assert_eq!(selected(cx), ["a2", "a3", "a4"]);
}

#[gpui::test]
fn a_plain_click_or_clear_ends_the_selection(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    press(cx, "a1", Modifiers::control());
    press(cx, "a4", Modifiers::none());
    assert!(selected(cx).is_empty());
    assert_eq!(here(cx), "/cc-agent/a4");
    press(cx, "a1", Modifiers::control());
    assert!(shown(cx, "bulk-clear"));
    click(cx, "bulk-clear");
    assert!(selected(cx).is_empty());
    assert!(!shown(cx, "bulk-count"));
    press(cx, "a3", Modifiers::shift());
    assert_eq!(selected(cx), ["a3"], "Clear dropped the anchor too");
}

#[gpui::test]
fn landing_on_new_clears_the_selection(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    press(cx, "a1", Modifiers::control());
    cx.update(|_, cx| router::navigate("/cc-agent/a3", cx));
    assert_eq!(selected(cx), ["a1"], "another task keeps it");
    cx.update(|_, cx| router::navigate("/cc-agent/new", cx));
    assert!(selected(cx).is_empty());
}

#[gpui::test]
fn a_task_that_leaves_the_list_leaves_the_selection(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    press(cx, "a1", Modifiers::control());
    press(cx, "a2", Modifiers::control());
    cx.update(|window, cx| {
        let store = cx.global_mut::<Sessions>();
        let mut loaded = store.loaded.take().unwrap();
        loaded.sessions.retain(|s| s.id != "a1");
        store.show(loaded);
        window.refresh();
    });
    cx.run_until_parked();
    assert_eq!(selected(cx), ["a2"]);
}

#[gpui::test]
fn ranked_card_slots_come_first_in_row_major_order(cx: &mut TestAppContext) {
    let cx = cx.add_empty_window();
    cx.update(|_, cx| {
        let row = |rank: Option<usize>, id: &str| (rank, id.to_owned());
        cx.set_global(Drawn {
            rows: vec![
                row(None, "x"),
                row(Some(1), "b"),
                row(Some(0), "a"),
                row(None, "y"),
                row(Some(0), "a"),
            ],
            rank: None,
        });
        assert_eq!(visible(cx), ["a", "b", "x", "y"]);
    });
}

#[gpui::test]
fn pinned_cards_draw_with_their_ranks(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    cx.update(|window, cx| {
        let loaded = cx.global_mut::<Sessions>().loaded.as_mut().unwrap();
        loaded.sessions[3].pinned_at = Some(1);
        view_mode::set_pinned(Pinned::Card, cx);
        window.refresh();
    });
    cx.run_until_parked();
    let rows = cx.update(|_, cx| cx.global::<Drawn>().rows.clone());
    assert!(rows.contains(&(Some(0), "a3".to_owned())), "{rows:?}");
    assert!(rows.iter().any(|(rank, id)| rank.is_none() && id == "a0"));
    assert_eq!(cx.update(|_, cx| visible(cx))[0], "a3");
}
