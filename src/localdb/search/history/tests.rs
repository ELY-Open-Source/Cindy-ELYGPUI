use rusqlite::Connection;

use super::{Args, Pool, prepare};
use crate::localdb::search::mcp_app::TrustLabels;
use crate::{localdb::cjk, rich_answer::js::Js};

/// The columns Cindy's schema gives the search, with `messages_fts` as migration 0100 builds it.
fn db() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE sessions (id text PRIMARY KEY, status text NOT NULL DEFAULT 'active',
           agent_kind text NOT NULL DEFAULT 'cc', updated_at integer NOT NULL DEFAULT 0,
           user_send_at integer, cleared_at integer);
         CREATE TABLE messages (id text PRIMARY KEY, client_id text NOT NULL, session_id text NOT NULL,
           role text NOT NULL, content text NOT NULL, created_at integer NOT NULL,
           rewind_at integer, agent_meta text);
         CREATE VIRTUAL TABLE messages_fts USING fts5(message_id UNINDEXED, session_id UNINDEXED,
           role UNINDEXED, content, tokenize='porter unicode61');
         CREATE TABLE messages_fts_rows (fts_rowid integer PRIMARY KEY AUTOINCREMENT, message_id text UNIQUE);",
    )
    .unwrap();
    cjk::register(&db).unwrap();
    db
}

fn session(db: &Connection, id: &str, status: &str, kind: &str, updated: i64) {
    db.execute(
        "INSERT INTO sessions (id, status, agent_kind, updated_at) VALUES (?, ?, ?, ?)",
        (id, status, kind, updated),
    )
    .unwrap();
}

fn message(db: &Connection, id: &str, session: &str, role: &str, content: &str, at: i64) {
    db.execute(
        "INSERT INTO messages (id, client_id, session_id, role, content, created_at) VALUES (?, ?, ?, ?, ?, ?)",
        (id, format!("c-{id}"), session, role, content, at),
    )
    .unwrap();
}

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

fn args<'a>(query: &'a str, sessions: &'a [String], statuses: &'a [&'a str]) -> Args<'a> {
    Args {
        query,
        session_ids: sessions,
        agent_kind: None,
        statuses,
        activity_from: None,
        skip_vector: false,
        fts_pool: 151,
        fuse_pool: 150,
    }
}

fn labels() -> TrustLabels {
    TrustLabels::new(&crate::i18n::Translator::new("en").unwrap())
}

fn found(db: &Connection, pool: &Pool) -> Vec<String> {
    let page = pool.page(db, (0, 50), &labels()).unwrap();
    page.hits.into_iter().map(|h| h.message_id).collect()
}

#[test]
fn the_fts_arm_keeps_to_visible_rows_of_the_allowed_tasks() {
    let db = db();
    session(&db, "a", "active", "cc", 10);
    session(&db, "b", "archived", "codex", 10);
    session(&db, "c", "active", "cc", 10);
    message(&db, "m1", "a", "user", "\"the login bug again\"", 1);
    message(&db, "m2", "a", "tool_use", "\"login bug\"", 2);
    message(&db, "m3", "b", "assistant", "\"login fixed\"", 3);
    message(&db, "m4", "c", "user", "\"login elsewhere\"", 4);
    message(&db, "m5", "a", "user", "\"rewound login\"", 5);
    db.execute("UPDATE messages SET rewind_at = 9 WHERE id = 'm5'", [])
        .unwrap();
    let allowed = ids(&["a", "b"]);
    let pool = prepare(&db, &args("login", &allowed, &["active", "archived"]));
    assert_eq!(
        found(&db, &pool),
        ["m3", "m1"],
        "bm25 puts the shorter row first"
    );
    let pool = prepare(&db, &args("login", &allowed, &["active"]));
    assert_eq!(found(&db, &pool), ["m1"]);
    let mut codex = args("login", &allowed, &["active", "archived"]);
    codex.agent_kind = Some("codex");
    assert_eq!(found(&db, &prepare(&db, &codex)), ["m3"]);
}

#[test]
fn cleared_history_and_old_tasks_drop_out() {
    let db = db();
    session(&db, "a", "active", "cc", 100);
    session(&db, "old", "active", "cc", 5);
    message(&db, "before", "a", "user", "\"plan one\"", 10);
    message(&db, "after", "a", "user", "\"plan two\"", 30);
    message(&db, "stale", "old", "user", "\"plan three\"", 1);
    db.execute("UPDATE sessions SET cleared_at = 20 WHERE id = 'a'", [])
        .unwrap();
    let allowed = ids(&["a", "old"]);
    let mut recent = args("plan", &allowed, &["active"]);
    recent.activity_from = Some(50);
    assert_eq!(found(&db, &prepare(&db, &recent)), ["after"]);
    db.execute("UPDATE sessions SET user_send_at = 60 WHERE id = 'old'", [])
        .unwrap();
    let mut found_ids = found(&db, &prepare(&db, &recent));
    found_ids.sort();
    assert_eq!(
        found_ids,
        ["after", "stale"],
        "user_send_at counts as activity"
    );
}

#[test]
fn han_runs_match_as_adjacent_characters() {
    let db = db();
    session(&db, "a", "active", "cc", 1);
    message(&db, "joined", "a", "user", "\"foo登录bar\"", 1);
    message(&db, "apart", "a", "user", "\"登 xx 录\"", 2);
    message(&db, "stem", "a", "user", "\"the runners keep running\"", 3);
    let allowed = ids(&["a"]);
    assert_eq!(
        found(&db, &prepare(&db, &args("登录", &allowed, &["active"]))),
        ["joined"]
    );
    assert_eq!(
        found(&db, &prepare(&db, &args("runs", &allowed, &["active"]))),
        ["stem"]
    );
    assert!(found(&db, &prepare(&db, &args("!!!", &allowed, &["active"]))).is_empty());
}

#[test]
fn the_pool_caps_and_pages() {
    let db = db();
    session(&db, "a", "active", "cc", 1);
    for i in 0..3 {
        message(&db, &format!("m{i}"), "a", "user", "\"deploy\"", i);
    }
    let allowed = ids(&["a"]);
    let mut capped = args("deploy", &allowed, &["active"]);
    capped.fuse_pool = 2;
    let pool = prepare(&db, &capped);
    assert!(pool.capped);
    let first = pool.page(&db, (0, 1), &labels()).unwrap();
    assert_eq!((first.hits.len(), first.next_offset), (1, Some(1)));
    assert_eq!(first.hits[0].fts_rank, 1);
    let hit = &first.hits[0];
    assert_eq!((hit.session_id.as_str(), hit.role.as_str()), ("a", "user"));
    assert!((0..3).contains(&hit.created_at));
    assert_eq!(hit.score, 1.0 / 61.0, "RRF at rank 1");
    let rest = pool.page(&db, (1, 5), &labels()).unwrap();
    assert_eq!((rest.hits.len(), rest.next_offset), (1, None));
    assert!(pool.page(&db, (9, 5), &labels()).unwrap().hits.is_empty());
}

#[test]
fn hit_rows_read_as_message_to_camel_reads_them() {
    let db = db();
    session(&db, "a", "active", "cc", 1);
    message(&db, "plain", "a", "user", "not json find", 1);
    message(
        &db,
        "blocks",
        "a",
        "assistant",
        r#"[{"type":"text","text":"find "},{"type":"text","text":"me"}]"#,
        2,
    );
    message(
        &db,
        "cited",
        "a",
        "assistant",
        "\"find \u{E200}cite\u{E202}x\u{E201}it\"",
        3,
    );
    db.execute(
        "UPDATE messages SET client_id = 'cloud-history:1', agent_meta = '{}' WHERE id = 'blocks'",
        [],
    )
    .unwrap();
    let allowed = ids(&["a"]);
    let pool = prepare(&db, &args("find", &allowed, &["active"]));
    db.execute("DELETE FROM messages WHERE id = 'cited'", [])
        .unwrap();
    let page = pool.page(&db, (0, 50), &labels()).unwrap();
    let by_id = |id: &str| page.hits.iter().find(|h| h.message_id == id).unwrap();
    let plain = by_id("plain").message.as_ref().unwrap();
    assert_eq!(plain.content, Js::Str("not json find".into()));
    assert_eq!(plain.client_id, "c-plain");
    let blocks = by_id("blocks").message.as_ref().unwrap();
    assert_eq!(blocks.content, Js::Str("find me".into()));
    assert_eq!(blocks.agent_meta.as_deref(), Some("{}"));
    assert!(
        by_id("cited").message.is_none(),
        "a row gone since is an empty context"
    );
}

#[test]
fn citations_leave_assistant_strings_and_a_broken_index_finds_nothing() {
    assert_eq!(
        super::camel_content(
            "\"a \u{E200}cite\u{E202}x\u{E201}b\"".into(),
            ("assistant", "c", None),
            &labels()
        ),
        Js::Str("a b".into())
    );
    let db = db();
    session(&db, "a", "active", "cc", 1);
    message(&db, "m", "a", "user", "\"find\"", 1);
    db.execute_batch("DROP TABLE messages_fts").unwrap();
    let allowed = ids(&["a"]);
    let pool = prepare(&db, &args("find", &allowed, &["active"]));
    assert!(found(&db, &pool).is_empty());
    assert_eq!(
        pool.vector_skip_reason,
        "聊天记录语义索引未启用, 本次仅用 FTS 全文检索。"
    );
    let mut keyword = args("find", &allowed, &["active"]);
    keyword.skip_vector = true;
    assert_eq!(
        prepare(&db, &keyword).vector_skip_reason,
        "本次请求已禁用语义检索, 仅用 FTS。"
    );
}

#[test]
fn an_app_row_reads_as_its_public_history() {
    let db = db();
    session(&db, "a", "active", "cc", 1);
    let card = r#"{"mcpApp":{"version":1,"serverId":"s","serverRevision":"r","resourceUri":"ui://x",
        "tool":{"name":"chart","inputSchema":{"type":"object"}},"arguments":{"secret":"token"},
        "result":{"content":[{"type":"text","text":"quarterly chart"}],"structuredContent":{"k":"hidden"}}}}"#;
    message(&db, "app", "a", "assistant", card, 1);
    message(&db, "plain", "a", "assistant", card, 2);
    db.execute(
        "UPDATE messages SET agent_meta = '{\"mcpApp\":true}' WHERE id = 'app'",
        [],
    )
    .unwrap();
    let allowed = ids(&["a"]);
    let pool = prepare(&db, &args("quarterly", &allowed, &["active"]));
    let page = pool.page(&db, (0, 50), &labels()).unwrap();
    let content = |id: &str| {
        &page
            .hits
            .iter()
            .find(|h| h.message_id == id)
            .unwrap()
            .message
            .as_ref()
            .unwrap()
            .content
    };
    let public = crate::rich_answer::js::parse(
        r#"{"mcpAppHistory":{"version":1,"title":"chart","text":"quarterly chart"}}"#,
    )
    .unwrap();
    assert_eq!(content("app"), &public);
    assert_ne!(
        content("plain"),
        &public,
        "only the marker makes a row an App row"
    );
}
