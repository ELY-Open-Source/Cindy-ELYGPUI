use rusqlite::Connection;

use super::*;
use crate::{i18n::Translator, rich_answer::js::stringify};

/// The columns `messagesListCursor.test.ts` builds from Cindy's schema.
pub(crate) fn db() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE sessions (id TEXT PRIMARY KEY, cleared_at INTEGER);
         CREATE TABLE messages (id TEXT PRIMARY KEY, client_id TEXT NOT NULL, session_id TEXT NOT NULL,
           role TEXT NOT NULL, content TEXT NOT NULL, tool_use_id TEXT, agent_meta TEXT,
           agent_kind TEXT, created_at INTEGER NOT NULL, rewind_at INTEGER);
         INSERT INTO sessions (id, cleared_at) VALUES ('s1', NULL);",
    )
    .unwrap();
    db
}

/// `insertMessage`: an assistant row whose client id is its id.
pub(crate) fn insert(db: &Connection, id: &str, created: i64, content: &str) {
    db.execute(
        "INSERT INTO messages (id, client_id, session_id, role, content, created_at)
         VALUES (?, ?, 's1', 'assistant', ?, ?)",
        (id, id, js::quote(content), created),
    )
    .unwrap();
}

fn labels() -> TrustLabels {
    TrustLabels::new(&Translator::new("en").unwrap())
}

fn read(db: &Connection, opts: Opts) -> Vec<Row> {
    list(db, "s1", &opts, false, &labels()).unwrap()
}

fn ids(rows: &[Row]) -> Vec<&str> {
    rows.iter().map(|r| super::super::text(r, "id")).collect()
}

fn rowids(rows: &[Row]) -> Vec<f64> {
    rows.iter()
        .map(|r| match r.obj().unwrap()["rowid"] {
            Js::Num(n) => n,
            _ => panic!("a numeric rowid"),
        })
        .collect()
}

fn before(id: &str) -> Opts {
    Opts {
        limit: Some(10.0),
        before: Some(id.into()),
        ..Opts::default()
    }
}

#[test]
fn a_before_cursor_continues_through_one_timestamp_in_insertion_order() {
    let db = db();
    insert(&db, "row-z", 1_000, "same timestamp oldest");
    insert(&db, "row-a", 1_000, "same timestamp cursor");
    insert(&db, "row-m", 1_000, "same timestamp newest");
    insert(&db, "row-old", 999, "older row");
    let rows = read(&db, before("row-a"));
    assert_eq!(ids(&rows), ["row-z", "row-old"]);
    assert_eq!(rowids(&rows), [1.0, 4.0]);
    let both = Opts {
        limit: Some(1.0),
        after: Some("row-old".into()),
        ..before("row-m")
    };
    assert_eq!(ids(&read(&db, both)), ["row-a"]);
}

#[test]
fn an_after_cursor_lists_newer_rows_newest_first() {
    let db = db();
    insert(&db, "row-z", 1_000, "cursor");
    insert(&db, "row-a", 1_000, "same timestamp newer");
    insert(&db, "row-new", 1_001, "newest");
    let rows = read(
        &db,
        Opts {
            limit: Some(10.0),
            after: Some("row-z".into()),
            ..Opts::default()
        },
    );
    assert_eq!(ids(&rows), ["row-new", "row-a"]);
}

#[test]
fn unknown_cursors_read_the_latest_page() {
    let db = db();
    insert(&db, "row-old", 999, "old");
    insert(&db, "row-new", 1_000, "new");
    let after = Opts {
        limit: Some(1.0),
        after: Some("missing".into()),
        ..Opts::default()
    };
    assert_eq!(ids(&read(&db, after)), ["row-new"]);
    assert_eq!(ids(&read(&db, before("missing"))), ["row-new", "row-old"]);
    let time = Opts {
        before_ts: Some(1_000.0),
        ..Opts::default()
    };
    assert_eq!(ids(&read(&db, time)), ["row-old"]);
}

#[test]
fn cleared_and_rewound_rows_stay_out() {
    let db = db();
    db.execute("UPDATE sessions SET cleared_at = 500", [])
        .unwrap();
    for (id, created) in [
        ("cleared", 100),
        ("before", 1000),
        ("anchor", 1000),
        ("after", 1000),
        ("rewound", 1100),
    ] {
        insert(&db, id, created, "text");
    }
    db.execute(
        "UPDATE messages SET rewind_at = 1200 WHERE id = 'rewound'",
        [],
    )
    .unwrap();
    let rows = read(
        &db,
        Opts {
            limit: Some(10.0),
            ..Opts::default()
        },
    );
    assert_eq!(ids(&rows), ["after", "anchor", "before"]);
    assert_eq!(rowids(&rows), [4.0, 3.0, 2.0]);
}

#[test]
fn limits_clamp_as_cindys() {
    for (raw, want) in [
        (None, 50),
        (Some(0.0), 50),
        (Some(-1.0), 50),
        (Some(f64::NAN), 50),
        (Some(2.7), 2),
        (Some(500.0), 100),
    ] {
        assert_eq!(clamp_limit(raw), want, "{raw:?}");
    }
}

#[test]
fn rows_read_as_message_to_camel_builds_them() {
    let db = db();
    let huge = "x".repeat(40_000);
    insert(&db, "huge", 1_000, &huge);
    db.execute(
        "INSERT INTO messages (id, client_id, session_id, role, content, tool_use_id, agent_meta, agent_kind, created_at)
         VALUES ('t', 'c-t', 's1', 'tool_use', '{broken', 'tu', '{\"model\":\"m\"}', 'codex', 1767225600123)",
        [],
    )
    .unwrap();
    let rows = read(&db, Opts::default());
    assert_eq!(
        stringify(&rows[0], ""),
        r#"{"id":"t","clientId":"c-t","sessionId":"s1","role":"tool_use","content":"{broken","toolUseId":"tu","agentMeta":{"model":"m"},"agentKind":"codex","createdAt":"2026-01-01T00:00:00.123Z","rowid":2}"#
    );
    assert_eq!(rows[1].obj().unwrap()["content"], Js::Str(huge));
    assert_eq!(rows[1].obj().unwrap()["agentMeta"], Js::Null);
}

#[test]
fn outline_rows_fold_bodies_and_carry_their_stamp() {
    let db = db();
    insert(&db, "a", 1, "Final answer");
    db.execute(
        "INSERT INTO messages (id, client_id, session_id, role, content, created_at)
         VALUES ('r', 'r', 's1', 'tool_result', '\"long log\"', 2)",
        [],
    )
    .unwrap();
    let rows = list(&db, "s1", &Opts::default(), true, &labels()).unwrap();
    let field = |i: usize, k: &str| rows[i].obj().unwrap()[k].clone();
    assert_eq!(field(0, "content"), Js::Str(String::new()));
    assert_eq!(
        field(0, "historyContentStamp"),
        Js::Str("10:226C6F6E67206C6F6722".into())
    );
    assert_eq!(field(1, "content"), Js::Str("Final answer".into()));
    assert_eq!(ids(&rows), ["r", "a"]);
}

#[test]
fn anchors_resolve_visible_rows_only() {
    let db = db();
    insert(&db, "a", 1, "kept");
    insert(&db, "gone", 2, "rewound");
    db.execute("UPDATE messages SET rewind_at = 3 WHERE id = 'gone'", [])
        .unwrap();
    let labels = labels();
    let row = anchor(&db, "s1", "a", false, &labels).unwrap();
    assert_eq!(super::super::text(&row, "id"), "a");
    db.execute(
        "INSERT INTO messages (id, client_id, session_id, role, content, created_at)
         VALUES ('stored', 'client', 's1', 'user', '\"hi\"', 1)",
        [],
    )
    .unwrap();
    let live = anchor(&db, "s1", "history-live:client", true, &labels).unwrap();
    assert_eq!(super::super::text(&live, "id"), "stored");
    assert!(live.obj().unwrap().get("historyContentStamp").is_none());
    let missing = |id: &str, session: &str| {
        anchor(&db, session, id, false, &labels)
            .unwrap_err()
            .to_string()
    };
    assert_eq!(missing("gone", "s1"), "[NOT_FOUND] History range changed");
    db.execute("UPDATE sessions SET cleared_at = 1", [])
        .unwrap();
    assert_eq!(missing("a", "s1"), "[NOT_FOUND] History range changed");
    assert_eq!(missing("a", "s2"), "[NOT_FOUND] History is unavailable");
}

#[test]
fn hydrate_reads_every_batch_or_fails() {
    let db = db();
    let all: Vec<String> = (0..150).map(|n| format!("m{n:03}")).collect();
    for (n, id) in all.iter().enumerate() {
        insert(&db, id, n as i64, "body");
    }
    let labels = labels();
    let rows = hydrate(&db, "s1", &all, &labels).unwrap();
    assert_eq!(rows.len(), 150);
    let mut asked = all.clone();
    asked.push("missing".into());
    assert_eq!(
        hydrate(&db, "s1", &asked, &labels).unwrap_err().to_string(),
        "[NOT_FOUND] History range changed"
    );
}

#[test]
fn validate_checks_stored_and_live_ids() {
    let db = db();
    insert(&db, "a", 1, "x");
    insert(&db, "b", 2, "x");
    validate(&db, "s1", &["a".into(), "history-live:b".into()]).unwrap();
    db.execute("UPDATE messages SET rewind_at = 3 WHERE id = 'b'", [])
        .unwrap();
    let error = validate(&db, "s1", &["a".into(), "history-live:b".into()]).unwrap_err();
    assert_eq!(error.to_string(), "[NOT_FOUND] History range changed");
    assert_eq!(
        validate(&db, "s2", &[]).unwrap_err().to_string(),
        "[NOT_FOUND] History is unavailable"
    );
}

#[test]
fn revisions_follow_writes() {
    let db = db();
    let first = revision(&db, 7).unwrap();
    assert!(first.starts_with("7:"), "{first}");
    insert(&db, "a", 1, "x");
    assert_ne!(revision(&db, 7).unwrap(), first);
}
