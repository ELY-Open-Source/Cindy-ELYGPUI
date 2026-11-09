use std::{collections::HashSet, rc::Rc};

use gpui::TestAppContext;

use super::*;
use crate::chat_store::{
    reconcile::Flight,
    window::{self, Island},
};

fn row(fields: &[(&str, Js)]) -> merge::Message {
    let fields = fields
        .iter()
        .map(|(k, v)| ((*k).to_owned(), Some(Rc::new(v.clone()))));
    Rc::new(fields.collect())
}

fn seed(store: &Store, messages: Vec<merge::Message>, cursor: Option<&str>) {
    store.set("s1", |s| {
        Some(SessionState {
            messages: Rc::new(messages),
            history_loaded: true,
            oldest_message_id: cursor.map(str::to_owned),
            ..s.clone()
        })
    });
}

fn talk(n: i64) -> rusqlite::Connection {
    let db = crate::history_view::rows::tests::db();
    for i in 0..n {
        put(&db, 2 * i, "user", "\"Ask\"", None);
        put(&db, 2 * i + 1, "assistant", "\"Answer\"", None);
    }
    db
}

fn reconcile(store: &Store, cx: &mut TestAppContext) -> Result<bool, String> {
    let flight = Flight::starting_at(&store.get_or_create("s1").messages);
    let (cx, preserve) = (cx.to_async(), HashSet::new());
    futures::executor::block_on(store.raw_reconcile("s1", &preserve, &flight, &cx))
}

#[gpui::test]
async fn raw_pages_step_back_from_the_cursor(cx: &mut TestAppContext) {
    let store = serve(talk(30), cx);
    seed(&store, Vec::new(), Some("50"));
    let more = cx.update(|cx| store.load_older_messages("s1", cx));
    assert!(more.await.unwrap());
    let state = store.state("s1").unwrap();
    // One page of 50 holds a prompt, so the gesture stops there.
    assert_eq!(state.messages.len(), 50);
    assert_eq!(state.oldest_message_id.as_deref(), Some("0"));
    assert!(state.has_more_messages);
    assert!(!state.is_loading_more);
    let more = cx.update(|cx| store.load_older_messages("s1", cx));
    assert!(!more.await.unwrap());
    let state = store.state("s1").unwrap();
    assert!(!state.has_more_messages);
    assert!(!state.is_loading_more);
}

#[gpui::test]
async fn without_a_cursor_raw_pages_start_before_the_oldest_time(cx: &mut TestAppContext) {
    let store = serve(talk(5), cx);
    let newest = row(&[
        ("clientId", Js::Str("c9".into())),
        ("createdAt", Js::Str("2023-11-14T22:13:20.009Z".into())),
    ]);
    seed(&store, vec![newest], None);
    let more = cx.update(|cx| store.load_older_messages("s1", cx));
    assert!(more.await.unwrap());
    assert_eq!(
        ids(&store),
        ["c0", "c1", "c2", "c3", "c4", "c5", "c6", "c7", "c8", "c9"]
    );
    let state = store.state("s1").unwrap();
    assert_eq!(state.oldest_message_id.as_deref(), Some("0"));
    assert!(!state.has_more_messages);
    // A row without a time gives no cursor.
    seed(
        &store,
        vec![row(&[("clientId", Js::Str("x".into()))])],
        None,
    );
    store.set("s1", |s| {
        Some(SessionState {
            has_more_messages: true,
            ..s.clone()
        })
    });
    let more = cx.update(|cx| store.load_older_messages("s1", cx));
    assert!(!more.await.unwrap());
    assert!(!store.state("s1").unwrap().is_loading_more);
}

#[gpui::test]
async fn a_raw_page_that_meets_the_window_hydrates_untouched_rows(cx: &mut TestAppContext) {
    let store = serve(one_turn(), cx);
    open(&store, cx);
    write(
        cx,
        "UPDATE messages SET content = '\"Edited\"' WHERE id = '5'",
    )
    .await;
    let epoch = store.epoch("s1");
    assert_eq!(reconcile(&store, cx), Ok(true));
    let state = store.state("s1").unwrap();
    assert_eq!(
        text(&state.messages.last().unwrap().clone(), "content"),
        "Edited"
    );
    assert_eq!(ids(&store).len(), 6);
    // A contiguous page keeps the window and its epoch.
    assert_eq!(store.epoch("s1"), epoch);
}

#[gpui::test]
async fn a_raw_page_that_misses_the_window_replaces_it(cx: &mut TestAppContext) {
    let store = serve(one_turn(), cx);
    let stale = row(&[("clientId", Js::Str("gone".into()))]);
    seed(&store, vec![stale], None);
    let epoch = store.epoch("s1");
    assert_eq!(reconcile(&store, cx), Ok(true));
    let state = store.state("s1").unwrap();
    assert_eq!(ids(&store), ["c0", "c1", "c2", "c3", "c4", "c5"]);
    assert_eq!(state.oldest_message_id.as_deref(), Some("0"));
    assert!(!state.has_more_messages);
    assert!(state.history_window_islands.is_empty());
    assert_eq!(store.epoch("s1"), epoch + 1);
}

#[gpui::test]
async fn a_kept_live_row_outside_the_page_marks_the_window_detached(cx: &mut TestAppContext) {
    let store = serve(one_turn(), cx);
    let live = row(&[
        ("clientId", Js::Str("live".into())),
        ("createdAt", Js::Str("2020-01-01T00:00:00.000Z".into())),
        ("isStreaming", Js::Bool(true)),
    ]);
    seed(&store, vec![live], None);
    assert_eq!(reconcile(&store, cx), Ok(true));
    let state = store.state("s1").unwrap();
    assert_eq!(ids(&store)[0], "live");
    let island = Island {
        oldest_client_id: "c5".into(),
        newest_client_id: "c5".into(),
    };
    assert_eq!(*state.history_window_islands, [island]);
}

#[gpui::test]
async fn a_failed_raw_read_fails_the_reconcile(cx: &mut TestAppContext) {
    let store = serve(one_turn(), cx);
    open(&store, cx);
    cx.update(|cx| crate::localdb::install(None, cx));
    assert_eq!(reconcile(&store, cx), Err("DbClient not ready".into()));
    // An unloaded task is not reconciled.
    store.set("s1", |s| {
        Some(SessionState {
            history_loaded: false,
            ..s.clone()
        })
    });
    assert_eq!(reconcile(&store, cx), Ok(false));
}

fn named(id: &str) -> merge::Message {
    row(&[
        ("clientId", Js::Str(id.into())),
        ("id", Js::Str(format!("id-{id}"))),
    ])
}

#[test]
fn paging_absorbs_the_islands_it_crosses() {
    let messages: Vec<_> = ["a", "b", "c", "d", "e", "f"].map(named).into();
    let island = |o: &str, n: &str| Island {
        oldest_client_id: o.into(),
        newest_client_id: n.into(),
    };
    let islands = [island("b", "b"), island("c", "e")];
    // The boundary matches a row by id too.
    let absorbed = window::absorb(&islands, &messages, Some("id-d")).unwrap();
    assert_eq!(absorbed, [island("b", "b"), island("c", "c")]);
    let absorbed = window::absorb(&islands, &messages, Some("a")).unwrap();
    assert!(absorbed.is_empty());
    let kept = window::absorb(&islands, &messages, Some("zz")).unwrap();
    assert_eq!(kept, islands);
    let broken = window::absorb(&[island("gone", "a")], &messages, Some("a"));
    assert_eq!(
        broken,
        Err("Cannot read properties of undefined (reading 'clientId')".into())
    );
    assert_eq!(
        window::main_run_oldest(&messages, &islands),
        Some(&messages[5])
    );
    assert_eq!(window::conservative(&messages), [island("f", "f")]);
}

#[gpui::test]
async fn a_forced_read_with_a_failed_range_falls_back_to_raw_pages(cx: &mut TestAppContext) {
    let db = crate::history_view::rows::tests::db();
    put(&db, 0, "user", "\"Run it\"", None);
    let call = r#"{"toolName":"constructor","toolUseId":"t1","input":{"a":1}}"#;
    put(&db, 1, "tool_use", call, Some("t1"));
    put(&db, 2, "tool_result", "\"ok\"", Some("t1"));
    put(&db, 3, "assistant", "\"Done\"", None);
    let store = serve(db, cx);
    open(&store, cx);
    cx.update(|cx| store.enter_view("s1", cx));
    let view = store.view("s1").unwrap();
    let key = {
        let snapshot = view.snapshot();
        crate::history_view::controller::summaries(&snapshot.items)[0]
            .key
            .clone()
    };
    view.set_expanded(&key, true);
    cx.run_until_parked();
    let run = cx.update(|cx| store.reconcile("s1", Default::default(), &cx.to_async()));
    cx.run_until_parked();
    // The raw page maps the same row, so the fallback fails as Cindy's does.
    assert_eq!(run.await, Err("paramKeys is not iterable".into()));
}

#[test]
fn raw_page_rules() {
    let parse = |s: &str| crate::rich_answer::js::parse(s).unwrap();
    let rows = parse(
        r#"[{"id":"a","createdAt":"2024-01-01T00:00:00.000Z"},{"id":"b","createdAt":"2024-01-01T00:00:00.000Z"},
            {"id":"c","createdAt":"2025-01-01T00:00:00.000Z","agentMeta":{"remoteRowsTrimmed":true}}]"#,
    );
    let rows = rows.arr().unwrap();
    let id = |r: Option<&crate::rich_answer::js::Obj>| r.unwrap()["id"].str().unwrap().to_owned();
    // A tie goes to the later row of a newest-first page, the earlier of an oldest-first one.
    assert_eq!(
        id(window::oldest_row(rows, merge::RowsOrder::NewestFirst)),
        "b"
    );
    assert_eq!(
        id(window::oldest_row(rows, merge::RowsOrder::OldestFirst)),
        "a"
    );
    assert_eq!(id(window::newest_row(rows)), "c");
    assert!(window::page_has_more(rows));
    assert!(!window::page_has_more(&rows[..2]));
    let trigger = parse(r#"{"role":"user","content":{"text":"[UI_ACTION_TRIGGER]go"}}"#);
    assert!(window::synthetic_trigger(trigger.obj().unwrap()));
    let typed = parse(r#"{"role":"user","content":"[UI_ACTION_TRIGGER]go"}"#);
    assert!(window::synthetic_trigger(typed.obj().unwrap()));
    let prompt = parse(r#"{"role":"user","content":"go"}"#);
    assert!(!window::synthetic_trigger(prompt.obj().unwrap()));
}

#[gpui::test]
async fn a_zero_time_gives_no_cursor(cx: &mut TestAppContext) {
    let store = serve(talk(2), cx);
    let epoch = row(&[
        ("clientId", Js::Str("x".into())),
        ("createdAt", Js::Num(0.0)),
    ]);
    seed(&store, vec![epoch], None);
    let more = cx.update(|cx| store.load_older_messages("s1", cx));
    assert!(!more.await.unwrap());
    assert_eq!(ids(&store), ["x"]);
    // No page was read, so nothing proved the start of history.
    assert!(store.state("s1").unwrap().has_more_messages);
}

/// The reviewer's case: a dropped waiter does not stop the read or keep the lock.
#[gpui::test]
async fn a_raw_read_finishes_without_its_waiter(cx: &mut TestAppContext) {
    let store = serve(talk(30), cx);
    seed(&store, Vec::new(), Some("50"));
    drop(cx.update(|cx| store.load_older_messages("s1", cx)));
    cx.run_until_parked();
    let state = store.state("s1").unwrap();
    assert_eq!(state.messages.len(), 50);
    assert!(!state.is_loading_more);
}
