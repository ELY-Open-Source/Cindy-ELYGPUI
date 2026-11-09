//! `sessionBrowseHistory.test.ts`'s cases, then the hook's walk over real routes.

use gpui::TestAppContext;

use super::{History, can, go, session_id};
use crate::{preferences::Preferences, router, sessions};

#[test]
fn task_views_alone_enter_the_stack() {
    assert_eq!(session_id("/cc-agent/session-a"), Some("session-a"));
    assert_eq!(session_id("/cc-agent/remote:host-1"), Some("remote:host-1"));
    for path in [
        "/cc-agent/new",
        "/cc-agent/new-dialogue",
        "/cc-agent/scheduled",
        "/cc-agent/boot",
        "/cc-agent/files/session-a",
        "/cc-agent/orca/session-a",
        "/bots/bot-1/session/session-a",
        "/settings",
        "/cc-agent/",
    ] {
        assert_eq!(session_id(path), None, "{path}");
    }
}

fn stack(h: &History) -> (Vec<&str>, isize) {
    (h.ids.iter().map(String::as_str).collect(), h.index)
}

#[test]
fn walks_back_and_forward_and_stops_at_both_ends() {
    let mut h = History::new();
    for id in ["a", "b", "c"] {
        h.record(id);
    }
    assert_eq!(h.step(-1).as_deref(), Some("b"));
    assert_eq!(h.step(-1).as_deref(), Some("a"));
    assert_eq!(h.step(-1), None);
    assert_eq!(h.step(1).as_deref(), Some("b"));
    assert_eq!(h.step(1).as_deref(), Some("c"));
    assert_eq!(h.step(1), None);
    assert_eq!(stack(&h), (vec!["a", "b", "c"], 2));
}

#[test]
fn the_current_task_takes_no_slot() {
    let mut h = History::new();
    h.record("a");
    h.record("a");
    assert_eq!(stack(&h), (vec!["a"], 0));
    assert_eq!(h.step(-1), None);
}

#[test]
fn opening_from_the_middle_drops_the_forward_side() {
    let mut h = History::new();
    h.record("a");
    h.record("b");
    h.step(-1);
    h.record("c");
    assert_eq!(stack(&h), (vec!["a", "c"], 1));
    assert_eq!(h.step(1), None);
    assert_eq!(h.step(-1).as_deref(), Some("a"));
}

#[test]
fn the_oldest_entry_drops_past_fifty() {
    let mut h = History::new();
    for i in 0..51 {
        h.record(&format!("t{i}"));
    }
    assert_eq!(h.ids.len(), 50);
    assert_eq!((h.ids[0].as_str(), h.index), ("t1", 49));
}

#[test]
fn only_another_owner_clears_it() {
    let mut h = History::new();
    h.record("a");
    h.bind_owner(Some("owner-1".into()));
    assert_eq!(stack(&h), (vec![], -1));
    h.record("b");
    h.bind_owner(Some("owner-1".into()));
    assert_eq!(stack(&h), (vec!["b"], 0));
    h.bind_owner(Some("owner-2".into()));
    assert_eq!(stack(&h), (vec![], -1));
}

fn start(cx: &mut TestAppContext) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_owned();
    cx.update(|cx| {
        cx.set_global(Preferences::load(&path).unwrap());
        sessions::install(std::path::PathBuf::new(), None, cx);
        router::install(cx);
        super::install(cx);
    });
    cx.run_until_parked();
    dir
}

fn open(cx: &mut TestAppContext, to: &str) {
    cx.update(|cx| router::navigate(to, cx));
    cx.run_until_parked();
}

fn hop(cx: &mut TestAppContext, direction: isize) -> bool {
    let went = cx.update(|cx| go(direction, cx));
    cx.run_until_parked();
    went
}

fn here(cx: &mut TestAppContext) -> String {
    cx.update(|cx| router::path(cx).to_owned())
}

#[gpui::test]
fn back_and_forward_walk_the_visited_tasks(cx: &mut TestAppContext) {
    let _dir = start(cx);
    for id in ["a", "b", "c"] {
        open(cx, &format!("/cc-agent/{id}"));
    }
    assert_eq!(cx.update(|cx| can(cx)), (true, false));
    assert!(hop(cx, -1));
    assert!(hop(cx, -1));
    assert_eq!(here(cx), "/cc-agent/a");
    assert_eq!(cx.update(|cx| can(cx)), (false, true));
    assert!(!hop(cx, -1), "the oldest has no back");
    assert!(hop(cx, 1));
    assert_eq!(here(cx), "/cc-agent/b");
    assert_eq!(
        cx.update(|cx| stack(cx.global::<History>()).0.len()),
        3,
        "a hop records nothing"
    );
}

#[gpui::test]
fn off_the_stack_back_returns_to_the_last_task(cx: &mut TestAppContext) {
    let _dir = start(cx);
    open(cx, "/cc-agent/a");
    open(cx, "/cc-agent/b");
    open(cx, "/cc-agent/new");
    assert_eq!(cx.update(|cx| can(cx)), (true, false));
    assert!(!hop(cx, 1), "no forward from a draft");
    assert!(hop(cx, -1));
    assert_eq!(
        here(cx),
        "/cc-agent/b",
        "the cursor's task, not one before it"
    );
    assert!(hop(cx, -1));
    assert_eq!(here(cx), "/cc-agent/a");
}

#[gpui::test]
fn an_empty_stack_has_nowhere_to_go(cx: &mut TestAppContext) {
    let _dir = start(cx);
    open(cx, "/cc-agent/new");
    assert_eq!(cx.update(|cx| can(cx)), (false, false));
    assert!(!hop(cx, -1));
}
