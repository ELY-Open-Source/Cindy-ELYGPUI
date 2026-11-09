use std::time::{Duration, Instant};

use gpui::{Modifiers, TestAppContext, VisualTestContext, point, px};

use super::{PER_VIEWPORT, Run};
use crate::{
    motion::{self, ReducedMotion},
    sessions::{
        Sessions, rename,
        view_mode::{self, Main},
        view_tests::open,
    },
};

#[test]
fn the_scroll_holds_at_both_ends() {
    let started = Instant::now();
    let run = Run {
        started,
        shift: -100.0,
        length: Duration::from_millis(1000),
    };
    let at = |ms| run.at(started + Duration::from_millis(ms));
    assert_eq!(at(0), (0.0, true));
    assert_eq!(at(120), (0.0, true));
    assert_eq!(at(500).0, -100.0 * motion::ease_move(0.5));
    assert_eq!(at(880), (-100.0, true));
    assert_eq!(at(1000), (-100.0, false), "held, forwards");
}

const LONG: &str =
    "a very long task title that cannot fit in the sidebar row at all, not even close";

fn retitle(cx: &mut VisualTestContext, id: &str, title: &str) {
    cx.update(|_, cx| {
        let loaded = cx.global_mut::<Sessions>().loaded.as_mut().unwrap();
        let s = loaded.sessions.iter_mut().find(|s| s.id == id).unwrap();
        s.title = title.into();
    });
    settle(cx);
}

fn settle(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

fn hover(cx: &mut VisualTestContext, id: &str) {
    let row = cx
        .debug_bounds(Box::leak(format!("task-{id}").into_boxed_str()))
        .unwrap();
    cx.simulate_mouse_move(row.center(), None, Modifiers::none());
    settle(cx);
}

/// The hovered row and its scroll: start, shift and length.
type Scroll = (Option<String>, Option<(Instant, f32, Duration)>);

fn run(cx: &mut VisualTestContext) -> Scroll {
    cx.update(|_, cx| {
        let state = &cx.global::<Sessions>().marquee;
        let run = state.run.as_ref().map(|r| (r.started, r.shift, r.length));
        (state.row.clone(), run)
    })
}

/// The track's offset from its title in the next frame, and whether the marquee asked for it.
fn offset(cx: &mut VisualTestContext, ms: u64) -> (f32, bool) {
    cx.executor().advance_clock(Duration::from_millis(ms));
    let asked = cx.update(|window, cx| window.simulate_next_frame(cx)) > 0;
    cx.run_until_parked();
    let title = cx.debug_bounds("task-title-a0").unwrap();
    let track = cx.debug_bounds("task-title-track-a0").unwrap();
    ((track.origin.x - title.origin.x) / px(1.0), asked)
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

fn scrolls_once_while_hovered(cx: &mut TestAppContext, main: Main) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    cx.update(|_, cx| view_mode::set_main(main, cx));
    retitle(cx, "a0", LONG);
    hover(cx, "a1");
    assert_eq!(
        run(cx),
        (Some("a1".into()), None),
        "{main:?}: a short title fits"
    );
    hover(cx, "a0");
    let (row, Some((_, shift, length))) = run(cx) else {
        panic!("{main:?}: the long title scrolls");
    };
    assert_eq!(row.as_deref(), Some("a0"));
    let width = cx.debug_bounds("task-title-a0").unwrap().size.width / px(1.0);
    assert!(
        shift < -width,
        "{main:?}: past twice the width: {shift} {width}"
    );
    assert_eq!(
        length,
        PER_VIEWPORT * ((width - shift) / width).ceil() as u32
    );
    let ms = |part: f32| (length.as_secs_f32() * 1000.0 * part) as u64;
    assert_eq!(offset(cx, ms(0.1)), (0.0, true), "{main:?}: still to 12%");
    let (half, _) = offset(cx, ms(0.4));
    assert!(
        near(half, shift * motion::ease_move(0.5)),
        "{main:?}: {half}"
    );
    let (end, asked) = offset(cx, ms(0.4));
    assert!(near(end, shift) && asked, "{main:?}: there at 90%: {end}");
    let (held, _) = offset(cx, ms(0.2));
    assert!(near(held, shift), "{main:?}: held");
    assert!(!offset(cx, 16).1, "{main:?}: no frames once done");
    retitle(cx, "a0", &format!("{LONG}, renamed"));
    let (_, Some((_, renamed, _))) = run(cx) else {
        panic!("{main:?}: a new title starts again");
    };
    assert!(renamed < shift, "{renamed} {shift}");
    cx.simulate_mouse_move(point(px(399.0), px(899.0)), None, Modifiers::none());
    settle(cx);
    assert_eq!(run(cx), (None, None), "{main:?}: leaving the row stops it");
}

#[gpui::test]
fn the_text_row_title_scrolls_while_hovered(cx: &mut TestAppContext) {
    scrolls_once_while_hovered(cx, Main::Text);
}

#[gpui::test]
fn the_list_row_title_scrolls_while_hovered(cx: &mut TestAppContext) {
    scrolls_once_while_hovered(cx, Main::List);
}

#[gpui::test]
fn a_key_press_keeps_the_hover(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    retitle(cx, "a0", LONG);
    hover(cx, "a0");
    let before = run(cx);
    assert!(before.1.is_some());
    cx.simulate_keystrokes("a");
    settle(cx);
    assert_eq!(run(cx), before, "CSS :hover ignores the keyboard");
}

#[gpui::test]
fn reduced_motion_keeps_the_ellipsis(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    let reduce = |cx: &mut VisualTestContext, on: bool| {
        cx.update(|_, cx| cx.set_global(ReducedMotion(on)));
        settle(cx);
    };
    reduce(cx, true);
    retitle(cx, "a0", LONG);
    hover(cx, "a0");
    assert_eq!(run(cx), (Some("a0".into()), None));
    reduce(cx, false);
    assert!(run(cx).1.is_some(), "motion back, the scroll starts");
    reduce(cx, true);
    assert_eq!(run(cx), (Some("a0".into()), None), "and stops again");
}

#[gpui::test]
fn a_title_back_from_rename_starts_again(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    retitle(cx, "a0", LONG);
    hover(cx, "a0");
    let Some((first, _, _)) = run(cx).1 else {
        panic!("the long title scrolls");
    };
    cx.update(|window, cx| {
        let loaded = cx.global::<Sessions>().loaded.as_ref().unwrap();
        let s = loaded
            .sessions
            .iter()
            .find(|s| s.id == "a0")
            .unwrap()
            .clone();
        rename::start(&s, crate::sessions::row::Scope::Sidebar, window, cx);
    });
    settle(cx);
    assert!(
        cx.debug_bounds("task-title-a0").is_none(),
        "the field stands in"
    );
    cx.executor().advance_clock(Duration::from_millis(100));
    cx.simulate_keystrokes("escape");
    settle(cx);
    assert!(cx.debug_bounds("task-title-a0").is_some());
    let (row, Some((again, _, _))) = run(cx) else {
        panic!("the title is back under the pointer");
    };
    assert_eq!(row.as_deref(), Some("a0"));
    assert!(again > first, "a fresh start");
}

#[gpui::test]
fn an_unmounted_row_lets_go(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    retitle(cx, "a0", LONG);
    hover(cx, "a0");
    assert!(run(cx).1.is_some());
    cx.update(|_, cx| {
        // Emptied, so no other row comes under the pointer.
        let loaded = cx.global_mut::<Sessions>().loaded.as_mut().unwrap();
        loaded.sessions.clear();
    });
    settle(cx);
    settle(cx);
    assert_eq!(run(cx), (None, None));
}
