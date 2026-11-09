use gpui::{
    Modifiers, ScrollDelta, ScrollEvent, TestAppContext, TouchPhase, VisualTestContext, point, px,
    size,
};
use rusqlite::Connection;

use crate::{
    localdb::search::conversations::tests::task,
    sessions::search::{
        query,
        run::tests::{search, wait},
    },
};

fn plans(db: &Connection) {
    for i in 0..30 {
        task(db, &format!("p{i}"), &format!("plan {i}"), i);
    }
}

fn scroll(cx: &mut VisualTestContext, by: f32) {
    let list = cx.debug_bounds("main-list").unwrap();
    cx.simulate_event(ScrollEvent {
        position: list.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-by))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
        ..Default::default()
    });
    cx.run_until_parked();
}

fn top(cx: &mut VisualTestContext, selector: &'static str) -> gpui::Pixels {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.debug_bounds(selector).unwrap().top()
}

/// The results scrolled 120 under a running query.
fn scrolled<'a>(cx: &'a mut TestAppContext, dir: &std::path::Path) -> &'a mut VisualTestContext {
    let cx = search(cx, dir, Some(plans));
    cx.simulate_resize(size(px(400.0), px(300.0)));
    cx.simulate_input("plan");
    wait(cx, 250);
    scroll(cx, 120.0);
    cx
}

#[gpui::test]
fn a_running_query_holds_the_row_at_the_top(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = scrolled(cx, dir.path());
    assert_eq!(top(cx, "nav-search"), top(cx, "main-list"));
    assert!(
        top(cx, "search-results") < top(cx, "main-list"),
        "the results moved"
    );
}

#[gpui::test]
fn the_results_under_the_row_take_no_press(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = scrolled(cx, dir.path());
    let row = cx.debug_bounds("nav-search").unwrap();
    cx.simulate_click(point(px(200.0), row.bottom() - px(2.0)), Modifiers::none());
    cx.run_until_parked();
    let path = cx.update(|_, cx| crate::router::path(cx).to_owned());
    assert!(!path.starts_with("/cc-agent/p"), "{path}");
    assert_eq!(cx.update(|_, cx| query(cx).to_owned()), "plan");
}

#[gpui::test]
fn without_a_query_the_row_scrolls_away(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = search(cx, dir.path(), Some(plans));
    cx.simulate_resize(size(px(400.0), px(300.0)));
    scroll(cx, 60.0);
    assert!(top(cx, "nav-search") < top(cx, "main-list"));
}
