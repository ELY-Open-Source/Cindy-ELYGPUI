use std::time::Duration;

use gpui::TestAppContext;
use rusqlite::Connection;

use super::*;
use crate::{chat_message::merge, i18n::Translator, rich_answer::js::Js};

fn put(db: &Connection, n: i64, role: &str, content: &str, tool: Option<&str>) {
    db.execute(
        "INSERT INTO messages (id, client_id, session_id, role, content, tool_use_id, created_at)
         VALUES (?, ?, 's1', ?, ?, ?, ?)",
        (
            n.to_string(),
            format!("c{n}"),
            role,
            content,
            tool,
            1_700_000_000_000 + n,
        ),
    )
    .unwrap();
}

/// A turn with one Read and one Bash call, both folded behind details.
fn turn(db: &Connection, at: i64) {
    put(db, at, "user", "\"Look\"", None);
    for (n, (tool, id)) in [("Read", "r"), ("Bash", "b")].into_iter().enumerate() {
        let n = at + 1 + 2 * n as i64;
        let id = format!("{id}{at}");
        let input = r#"{"file_path":"/a","command":"ls"}"#;
        let call = format!(r#"{{"toolName":"{tool}","toolUseId":"{id}","input":{input}}}"#);
        put(db, n, "tool_use", &call, Some(&id));
        put(db, n + 1, "tool_result", "\"ok\"", Some(&id));
    }
    put(db, at + 5, "assistant", "\"Done\"", None);
}

fn serve(db: Connection, cx: &mut TestAppContext) -> Store {
    cx.update(|cx| {
        crate::localdb::install(None, cx);
        cx.set_global(Translator::new("en").unwrap());
        crate::localdb::serve(db, cx);
        install(cx);
        Store::global(cx)
    })
}

fn one_turn() -> Connection {
    let db = crate::history_view::rows::tests::db();
    turn(&db, 0);
    db
}

fn ids(store: &Store) -> Vec<String> {
    let state = store.state("s1").unwrap();
    let ids = state
        .messages
        .iter()
        .map(|m| text(m, "clientId").to_owned());
    ids.collect()
}

fn text<'a>(m: &'a merge::Message, key: &str) -> &'a str {
    merge::get(m, key).and_then(Js::str).unwrap_or("")
}

fn open(store: &Store, cx: &mut TestAppContext) {
    cx.update(|cx| store.ensure_initial_messages("s1", cx));
    cx.run_until_parked();
}

#[gpui::test]
async fn the_first_page_fills_the_slice(cx: &mut TestAppContext) {
    let store = serve(one_turn(), cx);
    cx.update(|cx| {
        store.ensure_initial_messages("s1", cx);
        // A second caller joins the read in flight.
        store.ensure_initial_messages("s1", cx);
        assert!(store.0.borrow().in_flight.contains("s1"));
    });
    cx.run_until_parked();
    let state = store.state("s1").unwrap();
    assert!(state.history_loaded);
    assert!(!state.has_more_messages);
    assert!(!state.is_loading_more);
    assert_eq!(ids(&store), ["c0", "c5"]);
    assert!(!store.0.borrow().in_flight.contains("s1"));
}

#[gpui::test]
async fn a_failed_first_read_can_run_again(cx: &mut TestAppContext) {
    let store = serve(one_turn(), cx);
    cx.update(|cx| crate::localdb::install(None, cx));
    open(&store, cx);
    let state = store.state("s1").unwrap();
    assert!(!state.history_loaded);
    assert!(store.0.borrow().fetch_tokens.is_empty());
    assert!(store.0.borrow().in_flight.is_empty());
    let view = store.view("s1").unwrap();
    assert_eq!(view.snapshot().error.as_deref(), Some("DbClient not ready"));
    cx.update(|cx| crate::localdb::serve(one_turn(), cx));
    open(&store, cx);
    assert_eq!(ids(&store), ["c0", "c5"]);
}

#[gpui::test]
async fn a_window_changed_mid_read_reads_again(cx: &mut TestAppContext) {
    let store = serve(one_turn(), cx);
    cx.update(|cx| store.ensure_initial_messages("s1", cx));
    let token = store.0.borrow().fetch_tokens["s1"];
    *store.0.borrow_mut().epochs.entry("s1".into()).or_default() += 1;
    cx.run_until_parked();
    // The retry took a new token and its own read landed in the new epoch.
    assert!(store.0.borrow().fetch_tokens["s1"] > token);
    assert!(store.state("s1").unwrap().history_loaded);
    assert_eq!(ids(&store), ["c0", "c5"]);
}

#[gpui::test]
async fn views_count_and_the_last_to_leave_pauses(cx: &mut TestAppContext) {
    let store = serve(one_turn(), cx);
    open(&store, cx);
    let view = store.view("s1").unwrap();
    cx.update(|cx| {
        store.enter_view("s1", cx);
        store.enter_view("s1", cx);
    });
    store.leave_view("s1");
    assert!(view.is_active());
    assert!(store.0.borrow().active.contains_key("s1"));
    store.leave_view("s1");
    assert!(!view.is_active());
    assert!(!store.0.borrow().active.contains_key("s1"));
    assert!(view.snapshot().stale);
    assert!(store.0.borrow().views["s1"].mirror_paused);
    // An extra leave is ignored.
    store.leave_view("s1");
}

#[gpui::test]
async fn a_returning_view_reads_rows_written_while_away(cx: &mut TestAppContext) {
    let store = serve(one_turn(), cx);
    open(&store, cx);
    cx.update(|cx| store.enter_view("s1", cx));
    store.leave_view("s1");
    let write = cx.update(|cx| {
        let crate::localdb::State::Ready(db) = &cx.global::<crate::localdb::LocalDb>().state else {
            panic!("a ready database");
        };
        db.call(|conn| put(conn, 6, "user", "\"More\"", None))
    });
    write.await.unwrap();
    // A paused mirror ignores the store's other reads.
    assert_eq!(ids(&store), ["c0", "c5"]);
    cx.update(|cx| store.enter_view("s1", cx));
    cx.run_until_parked();
    assert_eq!(ids(&store), ["c0", "c5", "c6"]);
    assert!(store.0.borrow().force.is_empty());
}

#[gpui::test]
async fn folded_read_bodies_retire_once_history_owns_them(cx: &mut TestAppContext) {
    let store = serve(one_turn(), cx);
    open(&store, cx);
    let view = store.view("s1").unwrap();
    let summary = {
        let snapshot = view.snapshot();
        let all = crate::history_view::controller::summaries(&snapshot.items);
        all[0].clone()
    };
    view.load_details(summary, true).await;
    cx.run_until_parked();
    assert_eq!(ids(&store), ["c0", "c1", "c2", "c3", "c4", "c5"]);
    view.reset();
    cx.run_until_parked();
    // The Read pair goes back to its detail range; the Bash pair stays for other readers.
    assert_eq!(ids(&store), ["c0", "c3", "c4", "c5"]);
}

#[gpui::test]
async fn older_pages_move_the_window_back(cx: &mut TestAppContext) {
    let db = crate::history_view::rows::tests::db();
    for n in 0..30 {
        put(&db, 2 * n, "user", "\"Ask\"", None);
        put(&db, 2 * n + 1, "assistant", "\"Answer\"", None);
    }
    let store = serve(db, cx);
    assert!(!store.load_older_messages("s1").await.unwrap());
    open(&store, cx);
    let first = store.state("s1").unwrap();
    assert!(first.has_more_messages);
    let cursor = first.oldest_message_id.clone();
    assert!(cursor.is_some());
    let moved = store.load_older_messages("s1");
    cx.run_until_parked();
    assert!(moved.await.unwrap());
    let state = store.state("s1").unwrap();
    assert_ne!(state.oldest_message_id, cursor);
    assert!(state.messages.len() > first.messages.len());
    assert!(!state.is_loading_more);
    while store.state("s1").unwrap().has_more_messages {
        let more = store.load_older_messages("s1");
        cx.run_until_parked();
        more.await.unwrap();
    }
    assert_eq!(store.state("s1").unwrap().messages.len(), 60);
    // At the start of history the window cannot move.
    assert!(!store.load_older_messages("s1").await.unwrap());
}

#[gpui::test]
async fn a_database_change_rereads_the_shown_view(cx: &mut TestAppContext) {
    let store = serve(one_turn(), cx);
    open(&store, cx);
    cx.update(|cx| store.enter_view("s1", cx));
    cx.run_until_parked();
    let write = cx.update(|cx| {
        let crate::localdb::State::Ready(db) = &cx.global::<crate::localdb::LocalDb>().state else {
            panic!("a ready database");
        };
        db.call(|conn| put(conn, 6, "user", "\"More\"", None))
    });
    write.await.unwrap();
    store.invalidate_all();
    cx.run_until_parked();
    assert_eq!(ids(&store), ["c0", "c5"]);
    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    assert_eq!(ids(&store), ["c0", "c5", "c6"]);
}

fn write(cx: &mut TestAppContext, sql: &'static str) -> impl std::future::Future<Output = ()> {
    let call = cx.update(|cx| {
        let crate::localdb::State::Ready(db) = &cx.global::<crate::localdb::LocalDb>().state else {
            panic!("a ready database");
        };
        db.call(move |conn| conn.execute(sql, []).map(|_| ()).unwrap())
    });
    async move { call.await.unwrap() }
}

const EDIT: &str = "UPDATE messages SET content = '\"Edited\"' WHERE id = '5'";

#[gpui::test]
async fn only_the_forced_resume_read_hydrates_rows_already_held(cx: &mut TestAppContext) {
    let store = serve(one_turn(), cx);
    open(&store, cx);
    cx.update(|cx| store.enter_view("s1", cx));
    write(cx, EDIT).await;
    store.invalidate_all();
    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    let answer =
        |store: &Store| text(&store.state("s1").unwrap().messages[1], "content").to_owned();
    assert_eq!(answer(&store), "Done");
    store.leave_view("s1");
    cx.update(|cx| store.enter_view("s1", cx));
    cx.run_until_parked();
    assert_eq!(answer(&store), "Edited");
}

#[gpui::test]
async fn a_resume_during_a_forced_read_runs_it_again(cx: &mut TestAppContext) {
    let store = serve(one_turn(), cx);
    open(&store, cx);
    cx.update(|cx| store.enter_view("s1", cx));
    store.leave_view("s1");
    cx.update(|cx| store.enter_view("s1", cx));
    let first = store.0.borrow().force["s1"].clone();
    store.leave_view("s1");
    cx.update(|cx| store.enter_view("s1", cx));
    assert!(first.borrow().rerun);
    write(cx, EDIT).await;
    cx.run_until_parked();
    assert!(store.0.borrow().force.is_empty());
    assert_eq!(ids(&store), ["c0", "c5"]);
}

/// The reviewer's case: a row changed right after the resume keeps its change.
#[gpui::test]
async fn a_row_changed_after_the_resume_began_is_kept(cx: &mut TestAppContext) {
    let store = serve(one_turn(), cx);
    open(&store, cx);
    cx.update(|cx| store.enter_view("s1", cx));
    store.leave_view("s1");
    cx.update(|cx| store.enter_view("s1", cx));
    store.set("s1", |s| {
        let mut rows = (*s.messages).clone();
        let mut row = (*rows[1]).clone();
        let live = Js::Str("live ingress".into());
        row.insert("content".into(), Some(std::rc::Rc::new(live)));
        rows[1] = std::rc::Rc::new(row);
        Some(SessionState {
            messages: std::rc::Rc::new(rows),
            ..s.clone()
        })
    });
    cx.run_until_parked();
    let state = store.state("s1").unwrap();
    assert_eq!(text(&state.messages[1], "content"), "live ingress");
}
