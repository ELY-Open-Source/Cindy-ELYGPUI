use std::time::Duration;

use gpui::{
    Modifiers, ScrollDelta, ScrollEvent, TestAppContext, TouchPhase, VisualTestContext, point, px,
    size,
};

use super::{Reveal, duration, nearest};
use crate::{
    motion::ReducedMotion,
    router,
    sessions::{
        Sessions,
        view_mode::{self, Main, Pinned},
        view_tests::open,
    },
};

/// A short window, so the last rows sit below the list's end.
fn short(cx: &mut TestAppContext) -> (tempfile::TempDir, &mut VisualTestContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    cx.simulate_resize(size(px(400.0), px(240.0)));
    cx.run_until_parked();
    (dir, cx)
}

fn go(cx: &mut VisualTestContext, to: &str) {
    cx.update(|window, cx| {
        router::navigate(to, cx);
        window.refresh();
    });
    cx.run_until_parked();
}

/// The list's offset after `ms` more and one frame.
fn frame(cx: &mut VisualTestContext, ms: u64) -> f32 {
    cx.executor().advance_clock(Duration::from_millis(ms));
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();
    cx.update(|_, cx| f32::from(cx.global::<Reveal>().handle.offset().y))
}

fn in_view(cx: &mut VisualTestContext, id: &str) -> bool {
    let row = cx
        .debug_bounds(Box::leak(format!("task-{id}").into_boxed_str()))
        .unwrap();
    let list = cx.debug_bounds("main-list").unwrap();
    row.top() >= list.top() && row.bottom() <= list.bottom() + px(0.5)
}

#[test]
fn nearest_follows_cssom() {
    assert_eq!(nearest(10.0, 20.0, 0.0, 100.0), 0.0);
    assert_eq!(nearest(90.0, 120.0, 0.0, 100.0), 20.0);
    assert_eq!(nearest(-30.0, -10.0, 0.0, 100.0), -30.0);
    assert_eq!(nearest(-10.0, 110.0, 0.0, 100.0), 0.0, "covers the port");
    assert_eq!(
        nearest(50.0, 200.0, 0.0, 100.0),
        50.0,
        "taller, below: its top"
    );
    assert_eq!(
        nearest(-150.0, 50.0, 0.0, 100.0),
        -50.0,
        "taller, above: its bottom"
    );
}

#[test]
fn the_length_grows_with_the_root_of_the_delta() {
    assert_eq!(duration(0.0), Duration::ZERO);
    assert!((duration(400.0).as_secs_f32() - 20.0 / 60.0).abs() < 1e-6);
    assert_eq!(duration(-400.0), duration(400.0));
    assert_eq!(duration(1e7), Duration::from_millis(1500));
}

#[gpui::test]
fn an_active_row_below_the_end_scrolls_up_smoothly(cx: &mut TestAppContext) {
    let (_dir, cx) = short(cx);
    assert!(!in_view(cx, "a4"));
    go(cx, "/cc-agent/a4");
    let first = frame(cx, 16);
    let done = frame(cx, 2000);
    assert!(done < 0.0, "scrolled: {done}");
    assert!(first > done, "on its way first: {first} then {done}");
    assert!(in_view(cx, "a4"));
    assert_eq!(frame(cx, 100), done, "it stays");
}

#[gpui::test]
fn reduced_motion_jumps(cx: &mut TestAppContext) {
    let (_dir, cx) = short(cx);
    cx.update(|_, cx| cx.set_global(ReducedMotion(true)));
    go(cx, "/cc-agent/a4");
    frame(cx, 0);
    assert!(in_view(cx, "a4"));
}

#[gpui::test]
fn a_row_in_view_does_not_scroll(cx: &mut TestAppContext) {
    let (_dir, cx) = short(cx);
    go(cx, "/cc-agent/a1");
    assert_eq!(frame(cx, 2000), 0.0);
}

#[gpui::test]
fn a_wheel_takes_over(cx: &mut TestAppContext) {
    let (_dir, cx) = short(cx);
    go(cx, "/cc-agent/a4");
    let before = frame(cx, 16);
    assert!(before < 0.0);
    let list = cx.debug_bounds("main-list").unwrap();
    cx.simulate_event(ScrollEvent {
        position: list.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(40.0))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
        ..Default::default()
    });
    cx.run_until_parked();
    let user = frame(cx, 0);
    assert!(user > before, "the wheel moved it back up: {user}");
    assert_eq!(frame(cx, 2000), user, "the reveal let go");
    cx.update(|window, _| window.refresh());
    assert_eq!(
        frame(cx, 2000),
        user,
        "a render of the same active task stays put"
    );
}

#[gpui::test]
fn a_row_in_view_stops_the_scroll_under_way(cx: &mut TestAppContext) {
    let (_dir, cx) = short(cx);
    go(cx, "/cc-agent/a4");
    frame(cx, 16);
    go(cx, "/cc-agent/a1");
    let stopped = frame(cx, 0);
    assert_eq!(frame(cx, 2000), stopped, "no more steps toward a4");
    assert!(in_view(cx, "a1"));
}

/// Opens `id` in `before`'s look, lets it settle, then draws `after`.
fn relook(
    cx: &mut VisualTestContext,
    id: &str,
    before: impl Fn(&mut gpui::App),
    after: impl Fn(&mut gpui::App),
) {
    cx.update(|window, cx| {
        before(cx);
        window.refresh();
    });
    cx.run_until_parked();
    go(cx, &format!("/cc-agent/{id}"));
    frame(cx, 2000);
    cx.update(|window, cx| {
        after(cx);
        window.refresh();
    });
    cx.run_until_parked();
    frame(cx, 2000);
}

#[gpui::test]
fn a_new_look_mounts_the_active_row_again(cx: &mut TestAppContext) {
    let (_dir, cx) = short(cx);
    relook(
        cx,
        "a4",
        |cx| view_mode::set_main(Main::Text, cx),
        |cx| view_mode::set_main(Main::List, cx),
    );
    assert!(in_view(cx, "a4"));
}

#[gpui::test]
fn pinned_cards_reveal_after_text(cx: &mut TestAppContext) {
    let (_dir, cx) = short(cx);
    cx.update(|_, cx| {
        let loaded = cx.global_mut::<Sessions>().loaded.as_mut().unwrap();
        for s in loaded.sessions.iter_mut().take(5) {
            s.pinned_at = Some(1);
        }
    });
    relook(
        cx,
        "a4",
        |cx| view_mode::set_pinned(Pinned::Text, cx),
        |cx| view_mode::set_pinned(Pinned::Card, cx),
    );
    assert!(in_view(cx, "a4"));
}
