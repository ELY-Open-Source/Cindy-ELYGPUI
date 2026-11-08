use rusqlite::Connection;

use super::{flat, query, sweep};

/// The columns of Cindy's schema this chapter reads.
fn db() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE sessions (id text PRIMARY KEY, title text NOT NULL DEFAULT 'New Maker',
           status text NOT NULL DEFAULT 'active', source text NOT NULL DEFAULT 'desktop',
           pinned_at integer, hidden_at integer, user_send_at integer, last_activity_at integer,
           created_at integer NOT NULL, updated_at integer NOT NULL);
         CREATE TABLE messages (id text PRIMARY KEY, session_id text NOT NULL);
         CREATE TABLE bot_profiles (id text PRIMARY KEY, status text NOT NULL, hidden_at integer);
         CREATE TABLE bot_session_links (session_id text, bot_id text, role text);",
    )
    .unwrap();
    db
}

fn add(db: &Connection, id: &str, source: &str, status: &str, updated: i64) {
    db.execute(
        "INSERT INTO sessions (id, title, status, source, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
        (id, format!("task {id}"), status, source, updated, updated),
    )
    .unwrap();
}

fn ids(rows: &[query::Row]) -> Vec<&str> {
    rows.iter().map(|r| r.id.as_str()).collect()
}

#[test]
fn lists_ordinary_tasks_newest_first_per_bucket() {
    let db = db();
    add(&db, "a", "desktop", "active", 10);
    add(&db, "b", "telegram", "active", 30);
    add(&db, "c", "review", "active", 40);
    add(&db, "d", "side-chat", "active", 50);
    add(&db, "e", "desktop", "archived", 20);
    add(&db, "f", "desktop", "deleted", 60);
    add(&db, "g", "desktop", "active", 30);
    add(&db, "h", "desktop", "active", 70);
    db.execute("UPDATE sessions SET hidden_at = 1 WHERE id = 'h'", [])
        .unwrap();
    add(&db, "routine", "bot", "active", 5);
    add(&db, "canonical", "bot", "active", 6);
    db.execute_batch(
        "INSERT INTO bot_profiles VALUES ('bot', 'active', NULL);
         INSERT INTO bot_session_links VALUES ('routine', 'bot', 'routine'), ('canonical', 'bot', 'canonical');",
    )
    .unwrap();
    let active = query::list(&db, query::Status::Active).unwrap();
    assert_eq!(
        ids(&active),
        ["g", "b", "a", "routine"],
        "same updated_at: id DESC"
    );
    assert_eq!(
        ids(&query::list(&db, query::Status::Archived).unwrap()),
        ["e"]
    );
    assert_eq!(
        ids(&query::list(&db, query::Status::All).unwrap()),
        ["g", "b", "e", "a", "routine"]
    );
    assert_eq!(query::Status::parse(Some("bogus")), query::Status::Active);
}

#[test]
fn activity_falls_back_from_activity_to_send_to_update() {
    let db = db();
    add(&db, "a", "desktop", "active", 10);
    db.execute("UPDATE sessions SET user_send_at = 15 WHERE id = 'a'", [])
        .unwrap();
    assert_eq!(
        query::list(&db, query::Status::Active).unwrap()[0].activity_at,
        15
    );
    db.execute(
        "UPDATE sessions SET last_activity_at = 25 WHERE id = 'a'",
        [],
    )
    .unwrap();
    assert_eq!(
        query::list(&db, query::Status::Active).unwrap()[0].activity_at,
        25
    );
}

#[test]
fn the_flat_list_leaves_pins_out_and_sorts_as_asked() {
    let row = |id: &str, created: i64, activity: i64, pinned: bool| query::Row {
        id: id.into(),
        title: id.into(),
        pinned,
        created_at: created,
        activity_at: activity,
    };
    let rows = [
        row("a", 1, 30, false),
        row("b", 3, 10, false),
        row("c", 2, 20, true),
        row("d", 3, 30, false),
    ];
    let order = |sort| {
        flat::sorted(&rows, sort)
            .iter()
            .map(|r| r.id.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(order(None), ["a", "d", "b"], "recency, ties in list order");
    assert_eq!(
        order(Some("created")),
        ["b", "d", "a"],
        "created, ties by id"
    );
}

#[test]
fn relative_time_steps_as_cindy_does() {
    let t = |key: &str, count: i64| format!("{}:{count}", key.rsplit('.').next().unwrap());
    let m = 60_000;
    let cases = [
        (0, "now:0"),
        (59_999, "now:0"),
        (m, "minute:1"),
        (59 * m, "minute:59"),
        (60 * m, "hour:1"),
        (24 * 60 * m, "day:1"),
        (7 * 24 * 60 * m, "week:1"),
        (30 * 24 * 60 * m, "month:1"),
        (365 * 24 * 60 * m, "year:1"),
        (-5 * m, "now:0"),
    ];
    for (ago, want) in cases {
        assert_eq!(
            flat::relative(1_000_000_000_000 - ago, 1_000_000_000_000, &t),
            want,
            "{ago}"
        );
    }
}

#[test]
fn the_sweep_removes_only_unsent_empty_leftovers() {
    let dir = tempfile::tempdir().unwrap();
    let db = db();
    for id in ["unsent", "sent", "chatty"] {
        add(&db, id, "desktop", "active", 1);
    }
    db.execute("UPDATE sessions SET user_send_at = 5 WHERE id = 'sent'", [])
        .unwrap();
    db.execute("INSERT INTO messages VALUES ('m', 'chatty')", [])
        .unwrap();
    let managed = dir.path().join("unsent");
    std::fs::create_dir(&managed).unwrap();
    let now = 1_000_000_000_000i64;
    let week = 7 * 24 * 60 * 60_000i64;
    let entries = serde_json::json!({ "version": 1, "entries": [
        { "id": "unsent", "createdAt": now, "managedDir": managed.to_str().unwrap() },
        { "id": "sent", "createdAt": now },
        { "id": "chatty", "createdAt": now },
        { "id": "other-owner", "createdAt": now - 1000 },
        { "id": "stale", "createdAt": now - week },
        { "id": 7, "createdAt": now },
    ]});
    let file = dir.path().join("provisional-sessions.json");
    std::fs::write(&file, entries.to_string()).unwrap();
    assert_eq!(sweep::sweep(&db, dir.path(), now), ["unsent"]);
    let left: Vec<String> = db
        .prepare("SELECT id FROM sessions ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(left, ["chatty", "sent"]);
    assert!(!managed.exists(), "its empty folder goes too");
    let kept: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(
        kept["entries"].as_array().unwrap().len(),
        1,
        "only the other owner's fresh entry"
    );
    assert_eq!(kept["entries"][0]["id"], "other-owner");
    std::fs::write(
        &file,
        serde_json::json!({ "version": 1, "entries": [{ "id": "x", "createdAt": now - week }] })
            .to_string(),
    )
    .unwrap();
    sweep::sweep(&db, dir.path(), now);
    assert!(!file.exists(), "no entries, no file");
}
