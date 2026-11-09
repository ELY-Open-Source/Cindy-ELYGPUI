use std::{path::PathBuf, time::Duration};

use gpui::{Modifiers, MouseButton, Pixels, Point, TestAppContext, VisualTestContext, point, px};

use crate::sessions::{
    Sessions, settings,
    view_mode::{self, Pinned},
    view_tests::open,
};

/// Pins a0, a1 and a2; newest first, the section shows a2, a1, a0.
fn pins(cx: &mut TestAppContext) -> (tempfile::TempDir, PathBuf, &mut VisualTestContext) {
    let dir = tempfile::tempdir().unwrap();
    let owner = dir.path().join("owner");
    let cx = open(cx, dir.path());
    cx.update(|window, cx| {
        let store = cx.global_mut::<Sessions>();
        store.owner_dir = Some(owner.clone());
        let loaded = store.loaded.as_mut().unwrap();
        for (i, s) in loaded.sessions.iter_mut().take(3).enumerate() {
            s.pinned_at = Some(i as i64 + 1);
        }
        window.activate_window();
        window.refresh();
    });
    cx.run_until_parked();
    (dir, owner, cx)
}

fn center(cx: &mut VisualTestContext, id: &str) -> Point<Pixels> {
    cx.update(|_, cx| cx.global::<Sessions>().sortable.bounds[id].center())
}

fn order(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|_, cx| {
        let loaded = cx.global::<Sessions>().loaded.as_ref().unwrap();
        loaded.settings.pinned_order.clone()
    })
}

fn dragging(cx: &mut VisualTestContext) -> Option<Vec<String>> {
    cx.update(|_, cx| {
        let active = cx.global::<Sessions>().sortable.active.as_ref();
        active.map(|a| a.columns.concat())
    })
}

/// Presses `from`, moves past gpui's threshold, then onto `to`.
fn drag(cx: &mut VisualTestContext, from: &str, to: &str) {
    let start = center(cx, from);
    let end = center(cx, to);
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(
        start + point(px(0.0), px(5.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
}

fn release(cx: &mut VisualTestContext, at: Point<Pixels>) {
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
}

fn saved(owner: &std::path::Path) -> Vec<String> {
    settings::read(owner, "linux").pinned_order
}

fn drops_where_it_was_dragged(cx: &mut TestAppContext, mode: Pinned) {
    let (_dir, owner, cx) = pins(cx);
    cx.update(|_, cx| view_mode::set_pinned(mode, cx));
    cx.run_until_parked();
    drag(cx, "a2", "a0");
    assert_eq!(
        dragging(cx),
        Some(vec!["a1".into(), "a0".into(), "a2".into()]),
        "{mode:?}"
    );
    let at = center(cx, "a2");
    release(cx, at);
    let want = ["a1", "a0", "a2"];
    assert_eq!(order(cx), want, "{mode:?}: shown at once");
    assert_eq!(saved(&owner), want, "{mode:?}: saved as a reorder");
    assert_eq!(dragging(cx), None);
}

#[gpui::test]
fn a_text_row_drops_where_it_was_dragged(cx: &mut TestAppContext) {
    drops_where_it_was_dragged(cx, Pinned::Text);
}

#[gpui::test]
fn a_list_row_drops_where_it_was_dragged(cx: &mut TestAppContext) {
    drops_where_it_was_dragged(cx, Pinned::List);
}

#[gpui::test]
fn displaced_rows_slide_into_place(cx: &mut TestAppContext) {
    let (_dir, _owner, cx) = pins(cx);
    let top = |cx: &mut VisualTestContext| cx.debug_bounds("task-a1").unwrap().origin.y;
    let (from, to) = (top(cx), center(cx, "a2").y);
    drag(cx, "a2", "a1");
    let mut seen = vec![top(cx)];
    for _ in 0..4 {
        cx.executor().advance_clock(Duration::from_millis(50));
        let asked = cx.update(|window, cx| window.simulate_next_frame(cx)) > 0;
        cx.run_until_parked();
        seen.push(top(cx));
        if !asked {
            break;
        }
    }
    let last = *seen.last().unwrap();
    assert!(
        (seen[0] - from).abs() < px(1.0),
        "starts where it was: {seen:?}"
    );
    assert!(seen[1] < from && seen[1] > last, "on its way: {seen:?}");
    assert!(last < to, "ends a slot higher: {seen:?} {to:?}");
}

#[gpui::test]
fn escape_puts_the_order_back(cx: &mut TestAppContext) {
    let (_dir, owner, cx) = pins(cx);
    drag(cx, "a2", "a0");
    assert!(dragging(cx).is_some());
    cx.simulate_keystrokes("escape");
    assert_eq!(dragging(cx), None);
    let at = center(cx, "a0");
    release(cx, at);
    assert!(order(cx).is_empty() && saved(&owner).is_empty());
}

#[gpui::test]
fn a_drop_off_the_list_saves_nothing(cx: &mut TestAppContext) {
    let (_dir, owner, cx) = pins(cx);
    drag(cx, "a2", "a0");
    assert!(dragging(cx).is_some());
    release(cx, point(px(399.0), px(899.0)));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert_eq!(dragging(cx), None);
    assert!(order(cx).is_empty() && saved(&owner).is_empty());
}

#[gpui::test]
fn a_lost_window_saves_nothing(cx: &mut TestAppContext) {
    let (_dir, owner, cx) = pins(cx);
    drag(cx, "a2", "a0");
    assert!(dragging(cx).is_some());
    cx.deactivate_window();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert_eq!(dragging(cx), None);
    assert!(!cx.update(|_, cx| cx.has_active_drag()));
    assert!(saved(&owner).is_empty());
}

#[gpui::test]
fn the_more_button_does_not_drag(cx: &mut TestAppContext) {
    let (_dir, _owner, cx) = pins(cx);
    let more = cx.debug_bounds("task-more-a2").unwrap().center();
    cx.simulate_mouse_down(more, MouseButton::Left, Modifiers::none());
    let below = center(cx, "a0");
    cx.simulate_mouse_move(below, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(dragging(cx), None);
    release(cx, below);
}

#[gpui::test]
fn a_pinned_project_drags_by_its_header_only(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    let entry = "project:local:/work/app";
    cx.update(|window, cx| {
        let loaded = cx.global_mut::<Sessions>().loaded.as_mut().unwrap();
        loaded.sessions[0].pinned_at = Some(1);
        loaded.settings.pinned_order = vec![entry.into()];
        window.activate_window();
        window.refresh();
    });
    cx.run_until_parked();
    let press = |cx: &mut VisualTestContext, at: Point<Pixels>| {
        cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
        let below = at + point(px(0.0), px(8.0));
        cx.simulate_mouse_move(below, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        let started = dragging(cx).is_some();
        cx.simulate_keystrokes("escape");
        release(cx, below);
        started
    };
    let task = cx.debug_bounds("task-a1").unwrap().center();
    assert!(!press(cx, task), "data-no-drag: a task in the project");
    let header = cx
        .debug_bounds("project:local:/work/app-header")
        .unwrap()
        .center();
    assert!(press(cx, header), "the project's header drags it");
}

#[gpui::test]
fn a_move_mid_slide_starts_where_the_row_is(cx: &mut TestAppContext) {
    let (_dir, _owner, cx) = pins(cx);
    let top = |cx: &mut VisualTestContext| cx.debug_bounds("task-a1").unwrap().origin.y;
    drag(cx, "a2", "a1");
    let frame = |cx: &mut VisualTestContext, ms: u64| {
        cx.executor().advance_clock(Duration::from_millis(ms));
        cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.run_until_parked();
    };
    frame(cx, 50);
    let midway = top(cx);
    let back = center(cx, "a1");
    cx.simulate_mouse_move(back, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        dragging(cx),
        Some(vec!["a2".into(), "a1".into(), "a0".into()])
    );
    assert!(
        (top(cx) - midway).abs() < px(1.0),
        "no jump: {midway:?} {:?}",
        top(cx)
    );
    frame(cx, 200);
    assert!(top(cx) > midway, "then down to its old slot");
}

#[gpui::test]
fn an_unpinned_entry_is_forgotten(cx: &mut TestAppContext) {
    let (_dir, _owner, cx) = pins(cx);
    let known = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| {
            let state = &cx.global::<Sessions>().sortable;
            (
                state.bounds.contains_key("a2"),
                state.tracks.contains_key("a2"),
            )
        })
    };
    assert_eq!(known(cx), (true, true));
    cx.update(|window, cx| {
        let loaded = cx.global_mut::<Sessions>().loaded.as_mut().unwrap();
        loaded
            .sessions
            .iter_mut()
            .find(|s| s.id == "a2")
            .unwrap()
            .pinned_at = None;
        window.refresh();
    });
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert_eq!(known(cx), (false, false));
}

/// Pins in card mode: two columns at the test width, [a2, a0] and [a1].
fn cards(cx: &mut TestAppContext) -> (tempfile::TempDir, PathBuf, &mut VisualTestContext) {
    let (dir, owner, cx) = pins(cx);
    cx.update(|_, cx| view_mode::set_pinned(Pinned::Card, cx));
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let (a, b) = (center(cx, "a2"), center(cx, "a1"));
    assert!(a.y == b.y && a.x < b.x, "two columns: {a:?} {b:?}");
    (dir, owner, cx)
}

#[gpui::test]
fn a_card_dropped_in_another_column_saves_its_slot(cx: &mut TestAppContext) {
    let (_dir, owner, cx) = cards(cx);
    drag(cx, "a0", "a1");
    assert_eq!(
        dragging(cx),
        Some(vec!["a2".into(), "a0".into(), "a1".into()]),
        "lower, so before a1"
    );
    let at = center(cx, "a0");
    release(cx, at);
    let want = ["a2", "a0", "a1"];
    assert_eq!(order(cx), want, "row-major from column 1, row 0");
    assert_eq!(saved(&owner), want);
}

#[gpui::test]
fn a_card_dropped_in_a_column_tail_goes_last_there(cx: &mut TestAppContext) {
    let (_dir, owner, cx) = cards(cx);
    let tail = point(center(cx, "a1").x, center(cx, "a0").y);
    drag(cx, "a2", "a0");
    cx.simulate_mouse_move(tail, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let columns = cx.update(|_, cx| {
        let active = cx.global::<Sessions>().sortable.active.as_ref();
        active.map(|a| a.columns.clone())
    });
    let want: Vec<Vec<String>> = vec![vec!["a0".into()], vec!["a1".into(), "a2".into()]];
    assert_eq!(columns, Some(want));
    let at = center(cx, "a2");
    release(cx, at);
    assert_eq!(saved(&owner), ["a1", "a0", "a2"]);
}

#[gpui::test]
fn a_card_slides_across_columns(cx: &mut TestAppContext) {
    let (_dir, _owner, cx) = cards(cx);
    // The drag image draws `task-a0` too, so the ghost is read from its track.
    let left = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| {
            let now = cx.background_executor().now();
            let state = &cx.global::<Sessions>().sortable;
            state.bounds["a0"].origin.x + state.tracks["a0"].offset(now, super::SLIDE).x
        })
    };
    let (from, to) = (
        left(cx),
        cx.update(|_, cx| cx.global::<Sessions>().sortable.bounds["a1"].origin.x),
    );
    drag(cx, "a0", "a1");
    let start = left(cx);
    cx.executor().advance_clock(Duration::from_millis(50));
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();
    let midway = left(cx);
    cx.executor().advance_clock(Duration::from_millis(200));
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();
    assert!(
        (start - from).abs() < px(1.0),
        "starts in its column: {start:?}"
    );
    assert!(midway > from && midway < to, "on its way: {midway:?}");
    let end = left(cx);
    assert!(
        (end - to).abs() < px(1.0),
        "ends in the other column: {end:?}"
    );
}
