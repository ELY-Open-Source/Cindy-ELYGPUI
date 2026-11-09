use super::{finalize, truncate};
use crate::sessions::{
    projection,
    query::{self, Status},
    tests::{add, db},
};

#[test]
fn finalizes_previews_as_finalize_plain_preview_does() {
    let raw = include_str!("../../../tests/fixtures/preview/expected.json");
    let golden: serde_json::Value = serde_json::from_str(raw).expect("golden JSON");
    let cases = golden["finalize"].as_array().expect("finalize cases");
    assert!(cases.len() >= 20);
    for case in cases {
        let (text, role) = (case[0].as_str(), case[1].as_str());
        assert_eq!(
            finalize(text, role).as_deref(),
            case[2].as_str(),
            "{text:?} {role:?}"
        );
        assert_eq!(
            finalize(text, role).as_deref(),
            case[2].as_str(),
            "cached {text:?}"
        );
    }
}

#[test]
fn a_cut_pair_shows_one_replacement_character() {
    let text = format!("{}😀", "a".repeat(139));
    assert_eq!(truncate(&text), format!("{}\u{FFFD}", "a".repeat(139)));
    assert_eq!(truncate(&"a".repeat(140)), "a".repeat(140));
}

fn message(db: &rusqlite::Connection, id: &str, session: &str, role: &str, content: &str, at: i64) {
    db.execute(
        "INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)",
        (id, session, role, content, at),
    )
    .unwrap();
}

#[test]
fn backfills_the_latest_visible_preview_and_the_count() {
    let db = db();
    add(&db, "a", "desktop", "active", 10);
    add(&db, "cached", "desktop", "active", 20);
    add(&db, "empty", "desktop", "active", 30);
    message(&db, "a1", "a", "user", r#"{"text":"**first**"}"#, 1);
    message(&db, "a2", "a", "assistant", r#""the *answer*""#, 2);
    message(&db, "a3", "a", "system", r#""hidden""#, 3);
    message(&db, "c1", "cached", "user", r#"{"text":"ignored"}"#, 1);
    db.execute(
        "UPDATE sessions SET list_preview = 'kept', list_preview_role = 'user', list_message_count = 7
         WHERE id = 'cached'",
        [],
    )
    .unwrap();
    let (rows, items) = query::list(&db, Status::Active).unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(items.len(), 2, "{items:?}");
    projection::backfill(&db, &items);
    let cache = |id: &str| -> (Option<String>, Option<String>, Option<i64>) {
        db.query_row(
            "SELECT list_preview, list_preview_role, list_message_count FROM sessions WHERE id = ?",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap()
    };
    assert_eq!(
        cache("a"),
        (
            Some("the *answer*".into()),
            Some("assistant".into()),
            Some(3)
        )
    );
    assert_eq!(
        cache("cached"),
        (Some("kept".into()), Some("user".into()), Some(7))
    );
    assert_eq!(cache("empty"), (None, None, Some(0)));
    let (_, again) = query::list(&db, Status::Active).unwrap();
    assert!(again.is_empty(), "{again:?}");
}

#[test]
fn skips_rewound_auto_resume_and_cleared_messages() {
    let db = db();
    add(&db, "s", "desktop", "active", 10);
    message(&db, "m1", "s", "user", r#"{"text":"kept"}"#, 1);
    message(&db, "m2", "s", "user", r#"{"text":"resumed"}"#, 5);
    message(&db, "m3", "s", "assistant", r#""rewound""#, 6);
    message(&db, "m4", "s", "assistant", "not json", 4);
    db.execute_batch(
        "UPDATE messages SET agent_meta = '{\"autoResume\":true}' WHERE id = 'm2';
         UPDATE messages SET rewind_at = 9 WHERE id = 'm3';",
    )
    .unwrap();
    let (_, items) = query::list(&db, Status::Active).unwrap();
    projection::backfill(&db, &items);
    let preview: String = db
        .query_row("SELECT list_preview FROM sessions", [], |r| r.get(0))
        .unwrap();
    assert_eq!(preview, "not json");
    db.execute_batch("UPDATE sessions SET list_preview = NULL, cleared_at = 4")
        .unwrap();
    let (_, items) = query::list(&db, Status::Active).unwrap();
    assert!(items.is_empty(), "nothing after the clear: {items:?}");
}

#[test]
fn the_list_reads_its_preview_as_session_to_camel_does() {
    let db = db();
    add(&db, "cached", "desktop", "active", 10);
    add(&db, "fresh", "desktop", "active", 20);
    add(&db, "none", "desktop", "active", 30);
    message(&db, "c1", "cached", "assistant", "\"x\"", 1);
    message(&db, "f1", "fresh", "assistant", "\"Tail \u{e200}ci\"", 1);
    db.execute(
        "UPDATE sessions SET list_preview = 'Tail \u{e200}ci' WHERE id = 'cached'",
        [],
    )
    .unwrap();
    let rows = query::list(&db, Status::Active).unwrap().0;
    let preview = |id: &str| {
        let s = rows.iter().find(|s| s.id == id).unwrap();
        s.preview.clone()
    };
    assert_eq!(
        preview("cached").as_deref(),
        Some("Tail \u{e200}ci"),
        "the cache's own role, here none, not the latest message's"
    );
    assert_eq!(
        preview("fresh").as_deref(),
        Some("Tail"),
        "an assistant answer"
    );
    assert_eq!(preview("none"), None);
    db.execute("UPDATE sessions SET summary = 'done' WHERE id = 'none'", [])
        .unwrap();
    let rows = query::list(&db, Status::Active).unwrap().0;
    let summary = rows
        .iter()
        .find(|s| s.id == "none")
        .unwrap()
        .summary
        .clone();
    assert_eq!(summary.as_deref(), Some("done"));
}
