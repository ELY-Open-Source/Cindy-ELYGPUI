use std::{rc::Rc, time::Duration};

use gpui::TestAppContext;

use super::*;
use crate::chat_store::{memory::Budget, window::Island};

fn row(id: usize) -> merge::Message {
    let fields = [
        ("clientId", Js::Str(format!("c{id}"))),
        ("id", Js::Str(format!("{id}"))),
        ("content", Js::Str("ab".into())),
    ];
    let fields = fields
        .into_iter()
        .map(|(k, v)| (k.to_owned(), Some(Rc::new(v))));
    Rc::new(fields.collect())
}

fn fill(store: &Store, id: &str, rows: usize) {
    store.set(id, |s| {
        Some(SessionState {
            messages: Rc::new((0..rows).map(row).collect()),
            history_loaded: true,
            ..s.clone()
        })
    });
}

fn island(o: usize, n: usize) -> Island {
    Island {
        oldest_client_id: format!("c{o}"),
        newest_client_id: format!("c{n}"),
    }
}

fn store(cx: &mut TestAppContext) -> Store {
    serve(one_turn(), cx)
}

#[gpui::test]
async fn the_cache_keeps_twenty_and_spares_shown_tasks(cx: &mut TestAppContext) {
    let store = store(cx);
    open(&store, cx);
    let view = store.view("s1").unwrap();
    cx.update(|cx| store.enter_view("s1", cx));
    for n in 0..20 {
        store.get_or_create(&format!("t{n}"));
    }
    // s1 is shown, so the next least recently used task goes.
    assert!(store.has("s1"));
    assert!(!store.has("t0"));
    store.leave_view("s1");
    // The view's own writes touched s1 when it was shown.
    store.get_or_create("t20");
    assert!(store.has("s1") && !store.has("t1"));
    for n in 2..=20 {
        store.get_or_create(&format!("t{n}"));
    }
    store.get_or_create("t21");
    assert!(!store.has("s1"));
    assert!(store.view("s1").is_none());
    assert!(!view.is_active());
    assert_eq!(store.0.borrow().sessions.len(), 20);
    // Touching a task moves it to the back of the line.
    store.get_or_create("t2");
    store.get_or_create("t22");
    assert!(store.has("t2"));
    assert!(!store.has("t3"));
}

#[gpui::test]
async fn a_left_window_over_300_keeps_its_newest_200(cx: &mut TestAppContext) {
    let store = store(cx);
    fill(&store, "a", 350);
    store.set("a", |s| {
        Some(SessionState {
            history_window_islands: Rc::new(vec![island(10, 20), island(140, 170)]),
            oldest_message_id: Some("200".into()),
            is_loading_more: true,
            has_more_messages: false,
            ..s.clone()
        })
    });
    fill(&store, "b", 301);
    fill(&store, "c", 300);
    cx.update(|cx| store.enter_view("b", cx));
    store.trim_messages();
    let a = store.state("a").unwrap();
    assert_eq!(a.messages.len(), 200);
    assert_eq!(window::text(&a.messages[0], "clientId"), Some("c150"));
    assert_eq!(*a.history_window_islands, [island(150, 170)]);
    assert_eq!(a.oldest_message_id.as_deref(), Some("200"));
    assert!(a.has_more_messages && !a.is_loading_more);
    assert_eq!(store.epoch("a"), 1);
    // A shown window and one at the threshold stay whole.
    assert_eq!(store.state("b").unwrap().messages.len(), 301);
    assert_eq!(store.state("c").unwrap().messages.len(), 300);
    assert_eq!(store.epoch("c"), 0);
    // Without islands the cursor clears, so paging starts from the kept rows.
    store.leave_view("b");
    let b = store.state("b").unwrap();
    assert_eq!(b.messages.len(), 200);
    assert_eq!(b.oldest_message_id, None);
}

#[gpui::test]
async fn over_budget_the_longest_left_windows_go_first(cx: &mut TestAppContext) {
    let store = store(cx);
    open(&store, cx);
    let view = store.view("s1").unwrap();
    store.0.borrow_mut().budget = Budget {
        messages: 11,
        characters: usize::MAX,
    };
    fill(&store, "a", 10);
    for id in ["s1", "a"] {
        cx.update(|cx| store.enter_view(id, cx));
        store.leave_view(id);
        cx.executor().advance_clock(Duration::from_millis(1));
    }
    // Twelve rows, one over: s1 left first and goes; a stays.
    let s1 = store.state("s1").unwrap();
    assert!(!s1.history_loaded && s1.messages.is_empty());
    assert!(store.view("s1").is_none());
    assert!(!view.is_active());
    assert_eq!(store.state("a").unwrap().messages.len(), 10);
    // A task never left, or with an unsaved row, stays.
    fill(&store, "never", 10);
    fill(&store, "pending", 10);
    store.set("pending", |s| {
        let mut rows = (*s.messages).clone();
        let mut last = (*rows[9]).clone();
        last.insert("isPendingPersist".into(), Some(Rc::new(Js::Bool(true))));
        rows[9] = Rc::new(last);
        Some(SessionState {
            messages: Rc::new(rows),
            ..s.clone()
        })
    });
    store.note_left("pending");
    store.0.borrow_mut().budget.messages = 0;
    store.soft_evict();
    assert_eq!(store.state("never").unwrap().messages.len(), 10);
    assert_eq!(store.state("pending").unwrap().messages.len(), 10);
    assert!(store.state("a").unwrap().messages.is_empty());
}

#[gpui::test]
async fn the_check_runs_every_30_seconds(cx: &mut TestAppContext) {
    let store = store(cx);
    fill(&store, "a", 5);
    store.note_left("a");
    cx.update(|cx| store.ensure_initial_messages("s1", cx));
    cx.run_until_parked();
    store.0.borrow_mut().budget.messages = 0;
    cx.executor().advance_clock(Duration::from_secs(29));
    cx.run_until_parked();
    assert_eq!(store.state("a").unwrap().messages.len(), 5);
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert!(store.state("a").unwrap().messages.is_empty());
}

#[gpui::test]
async fn the_earlier_leave_goes_first(cx: &mut TestAppContext) {
    let store = store(cx);
    for id in ["y", "x"] {
        fill(&store, id, 10);
    }
    store.note_left("x");
    cx.executor().advance_clock(Duration::from_millis(5));
    store.note_left("y");
    store.0.borrow_mut().budget.messages = 15;
    store.soft_evict();
    assert!(store.state("x").unwrap().messages.is_empty());
    assert_eq!(store.state("y").unwrap().messages.len(), 10);
}

/// The reviewer's case: a no-op write keeps a slice the full cache pushed out.
#[gpui::test]
async fn a_noop_write_keeps_a_slice_the_cap_pushed_out(cx: &mut TestAppContext) {
    let store = store(cx);
    for n in 0..20 {
        let id = format!("active-{n}");
        store.get_or_create(&id);
        cx.update(|cx| store.enter_view(&id, cx));
    }
    store.set("new", |_| None);
    assert!(!store.has("new"));
    assert_eq!(store.0.borrow().sessions.len(), 20);
    assert_eq!(store.0.borrow().access.len(), 20);
}
