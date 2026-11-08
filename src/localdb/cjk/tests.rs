use rusqlite::Connection;

use super::{ensure_triggers, register, seg};

/// Outputs of Cindy's `cjkSeg` for `tests/fixtures/cjk-seg/input.json`.
#[test]
fn segments_as_cindy_does() {
    let inputs: Vec<String> =
        serde_json::from_str(include_str!("../../../tests/fixtures/cjk-seg/input.json")).unwrap();
    let expected: Vec<String> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/cjk-seg/expected.json"
    ))
    .unwrap();
    for (input, want) in inputs.iter().zip(&expected) {
        assert_eq!(&seg(input), want, "{input}");
    }
}

/// Cindy's FTS tables and persistent triggers, which skip once `cjk_seg` is registered.
fn messages_db() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    let guard = "NOT EXISTS (SELECT 1 FROM pragma_function_list WHERE name='cjk_seg')";
    db.execute_batch(&format!(
        "CREATE TABLE messages (id text PRIMARY KEY, session_id text, role text, content text, rewind_at integer);
         CREATE VIRTUAL TABLE messages_fts USING fts5(message_id UNINDEXED, session_id UNINDEXED, role UNINDEXED, content);
         CREATE TABLE messages_fts_rows (fts_rowid integer PRIMARY KEY AUTOINCREMENT, message_id text UNIQUE);
         CREATE TRIGGER messages_fts_insert AFTER INSERT ON messages
         WHEN new.rewind_at IS NULL AND new.role IN ('user', 'assistant', 'ask_user', 'plan_review') AND {guard}
         BEGIN
           INSERT OR IGNORE INTO messages_fts_rows(message_id) VALUES (new.id);
           INSERT OR REPLACE INTO messages_fts(rowid, message_id, session_id, role, content)
             SELECT fts_rowid, new.id, new.session_id, new.role, new.content FROM messages_fts_rows WHERE message_id = new.id;
         END;"
    ))
    .unwrap();
    db
}

fn indexed(db: &Connection, id: &str) -> String {
    db.query_row(
        "SELECT content FROM messages_fts WHERE message_id = ?",
        [id],
        |r| r.get(0),
    )
    .unwrap()
}

#[test]
fn registered_connections_index_segmented_text() {
    let db = messages_db();
    db.execute(
        "INSERT INTO messages VALUES ('raw', 's', 'user', '登录报错', NULL)",
        [],
    )
    .unwrap();
    assert_eq!(
        indexed(&db, "raw"),
        "登录报错",
        "an old client indexes raw text"
    );
    register(&db).unwrap();
    db.execute(
        "INSERT INTO messages VALUES ('seg', 's', 'user', '登录报错', NULL)",
        [],
    )
    .unwrap();
    assert_eq!(
        indexed(&db, "seg"),
        "登 录 报 错",
        "the guard skips, the TEMP trigger segments"
    );
    let hits: i64 = db
        .query_row(
            "SELECT count(*) FROM messages_fts WHERE messages_fts MATCH '\"登 录\"'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(hits, 1);
}

#[test]
fn temp_store_drops_the_triggers_and_ensure_puts_them_back() {
    let db = messages_db();
    register(&db).unwrap();
    db.execute_batch("PRAGMA temp_store = MEMORY;").unwrap();
    ensure_triggers(&db).unwrap();
    db.execute(
        "INSERT INTO messages VALUES ('m', 's', 'assistant', 'foo登录bar', NULL)",
        [],
    )
    .unwrap();
    assert_eq!(indexed(&db, "m"), "foo 登 录 bar");
}
