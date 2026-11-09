use gpui::{TestAppContext, VisualTestContext};
use rusqlite::Connection;

use crate::{
    localdb::search::conversations::tests::{say, task},
    sessions::{
        search::{
            query,
            run::tests::{search, wait},
        },
        view_tests::click,
    },
};

fn drawn(cx: &mut VisualTestContext, selector: &str) -> bool {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.debug_bounds(Box::leak(selector.to_owned().into_boxed_str()))
        .is_some()
}

fn tasks(db: &Connection) {
    task(db, "t", "Fix login flow", 10);
    task(db, "h", "Notes", 20);
    for i in 0..5 {
        say(db, &format!("m{i}"), "h", &format!("\"login step {i}\""));
    }
}

#[gpui::test]
fn results_take_the_list_place_and_clearing_brings_it_back(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = search(cx, dir.path(), Some(tasks));
    assert!(drawn(cx, "task-a0"));
    cx.simulate_input("login");
    assert!(drawn(cx, "search-searching"));
    assert!(!drawn(cx, "task-a0"), "the list hides while a query runs");
    wait(cx, 250);
    assert!(!drawn(cx, "search-searching"));
    assert!(drawn(cx, "search-result-t") && drawn(cx, "search-result-h"));
    click(cx, "nav-search-clear");
    assert!(!drawn(cx, "search-results"));
    assert!(drawn(cx, "task-a0"));
}

#[gpui::test]
fn three_hits_show_then_more_shows_the_rest(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = search(cx, dir.path(), Some(tasks));
    cx.simulate_input("step");
    wait(cx, 250);
    let shown = |cx: &mut VisualTestContext| {
        (0..5)
            .filter(|i| drawn(cx, &format!("search-hit-m{i}")))
            .count()
    };
    assert_eq!(shown(cx), 3);
    click(cx, "search-more-h");
    assert_eq!(shown(cx), 5);
    assert!(!drawn(cx, "search-more-h"));
}

#[gpui::test]
fn a_press_on_the_results_keeps_the_query(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = search(cx, dir.path(), Some(tasks));
    cx.simulate_input("login");
    wait(cx, 250);
    cx.update(|window, cx| window.blur(cx));
    click(cx, "search-result-t");
    assert_eq!(cx.update(|_, cx| query(cx).to_owned()), "login");
}

#[gpui::test]
fn no_match_says_so(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = search(cx, dir.path(), Some(tasks));
    cx.simulate_input("zzz");
    wait(cx, 900);
    assert!(drawn(cx, "search-none"));
}

#[gpui::test]
fn a_failed_search_says_so(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = search(cx, dir.path(), None);
    cx.simulate_input("login");
    wait(cx, 900);
    assert!(drawn(cx, "search-failed"));
}

fn path(cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| crate::router::path(cx).to_owned())
}

#[gpui::test]
fn a_result_opens_its_task_and_keeps_the_query(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = search(cx, dir.path(), Some(tasks));
    cx.simulate_input("login");
    wait(cx, 250);
    click(cx, "search-result-t");
    assert_eq!(path(cx), "/cc-agent/t");
    assert_eq!(cx.update(|_, cx| query(cx).to_owned()), "login");
    assert!(drawn(cx, "search-result-h"), "the results stay");
}

#[gpui::test]
fn a_hit_opens_its_task(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = search(cx, dir.path(), Some(tasks));
    cx.simulate_input("step");
    wait(cx, 250);
    click(cx, "search-hit-m0");
    assert_eq!(path(cx), "/cc-agent/h");
    assert_eq!(cx.update(|_, cx| query(cx).to_owned()), "step");
}

#[gpui::test]
fn tab_reaches_a_result_and_enter_opens_it(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = search(cx, dir.path(), Some(tasks));
    cx.simulate_input("fix");
    wait(cx, 250);
    for _ in 0..TABS {
        cx.simulate_keystrokes("tab");
    }
    cx.simulate_keystrokes("enter");
    let keystroke = gpui::Keystroke::parse("enter").unwrap();
    cx.simulate_event(gpui::KeyUpEvent { keystroke });
    cx.run_until_parked();
    assert_eq!(path(cx), "/cc-agent/t");
    assert_eq!(cx.update(|_, cx| query(cx).to_owned()), "fix");
}

/// The field, then clear and the options trigger, then the first result.
const TABS: usize = 3;
