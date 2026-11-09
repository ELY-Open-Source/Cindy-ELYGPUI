use rusqlite::Connection;

use super::{Request, Response, clamp_limit, search};
use crate::{
    i18n::Translator,
    localdb::{
        cjk,
        search::{mcp_app::TrustLabels, merge::MatchKind},
    },
};

const DAY: i64 = 24 * 60 * 60 * 1000;
const NOW: i64 = 100 * DAY;

fn db() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    schema(&db);
    db
}

/// The columns `searchConversations` reads, with `messages_fts` as migration 0100 builds it.
pub(crate) fn schema(db: &Connection) {
    db.execute_batch(
        "CREATE TABLE sessions (id text PRIMARY KEY, title text NOT NULL DEFAULT 'New Maker',
           working_dir text, project_dir text, workspace_kind text NOT NULL DEFAULT 'project',
           agent_kind text NOT NULL DEFAULT 'cc', status text NOT NULL DEFAULT 'active',
           source text NOT NULL DEFAULT 'desktop', orca_role text, parent_session_id text,
           user_send_at integer, updated_at integer NOT NULL, created_at integer NOT NULL DEFAULT 0,
           cleared_at integer);
         CREATE TABLE bot_profiles (id text PRIMARY KEY, display_name text NOT NULL, status text NOT NULL);
         CREATE TABLE bot_session_links (id text PRIMARY KEY, bot_id text NOT NULL,
           session_id text NOT NULL, role text NOT NULL);
         CREATE TABLE messages (id text PRIMARY KEY, client_id text NOT NULL, session_id text NOT NULL,
           role text NOT NULL, content text NOT NULL, created_at integer NOT NULL,
           rewind_at integer, agent_meta text);
         CREATE VIRTUAL TABLE messages_fts USING fts5(message_id UNINDEXED, session_id UNINDEXED,
           role UNINDEXED, content, tokenize='porter unicode61');
         CREATE TABLE messages_fts_rows (fts_rowid integer PRIMARY KEY AUTOINCREMENT, message_id text UNIQUE);",
    )
    .unwrap();
    cjk::register(db).unwrap();
}

pub(crate) fn task(db: &Connection, id: &str, title: &str, updated: i64) {
    db.execute(
        "INSERT INTO sessions (id, title, updated_at) VALUES (?, ?, ?)",
        (id, title, updated),
    )
    .unwrap();
}

pub(crate) fn say(db: &Connection, id: &str, session: &str, content: &str) {
    db.execute(
        "INSERT INTO messages (id, client_id, session_id, role, content, created_at)
         VALUES (?, ?, ?, 'user', ?, 1)",
        (id, format!("c-{id}"), session, content),
    )
    .unwrap();
}

fn request(query: &str) -> Request<'_> {
    Request {
        query,
        limit: None,
        sort: "relevance",
        keyword_only: true,
        status: "all",
        agent_kind: "all",
        last_activity: "all",
        session_ids: None,
        working_dirs: None,
        unnamed_label: None,
    }
}

fn run(db: &Connection, request: &Request) -> Response {
    let labels = TrustLabels::new(&Translator::new("en").unwrap());
    search(db, request, &labels, NOW).unwrap()
}

fn ids(response: &Response) -> Vec<&str> {
    response
        .results
        .iter()
        .map(|r| r.session.id.as_str())
        .collect()
}

#[test]
fn titles_rank_over_content_and_both_merge() {
    let db = db();
    task(&db, "t", "Fix login flow", 10);
    task(&db, "c", "Other work", 20);
    task(&db, "b", "login notes", 5);
    say(&db, "m1", "c", "\"the login page broke\"");
    say(&db, "m2", "b", "\"login again\"");
    let response = run(&db, &request("  login "));
    assert_eq!(response.query, "login");
    assert_eq!(ids(&response), ["b", "t", "c"]);
    let kinds: Vec<MatchKind> = response.results.iter().map(|r| r.kind).collect();
    assert_eq!(
        kinds,
        [MatchKind::Both, MatchKind::Title, MatchKind::Content]
    );
    let hit = &response.results[2].hits[0];
    assert_eq!(
        (hit.client_id.as_str(), hit.role.as_str()),
        ("c-m1", "user")
    );
    assert_eq!(hit.preview, "the login page broke");
    assert_eq!(hit.snippet.as_deref(), Some("the login page broke"));
    assert_eq!(
        (hit.fts_rank, hit.created_at),
        (Some(2), 1),
        "bm25 ranks m2 first"
    );
    assert!(hit.score > 0.0);
    assert_eq!(
        response.vector_skip_reason,
        Some("本次请求已禁用语义检索, 仅用 FTS。")
    );
    assert!(!response.pool_capped);
}

#[test]
fn hidden_fields_do_not_count_as_hits() {
    let db = db();
    task(&db, "a", "Plain", 1);
    say(
        &db,
        "m",
        "a",
        r#"{"text":"see attached","images":[{"name":"secret.png"}]}"#,
    );
    say(&db, "t", "a", "\"[UI_ACTION_TRIGGER] secret rerun\"");
    assert!(run(&db, &request("secret")).results.is_empty());
    assert_eq!(ids(&run(&db, &request("attached"))), ["a"]);
}

#[test]
fn an_untitled_task_matches_by_its_shown_label() {
    let db = db();
    task(&db, "u", "New Maker", 1);
    let mut labeled = request("untitled");
    labeled.unnamed_label = Some("Untitled task");
    let found = run(&db, &labeled);
    assert_eq!(ids(&found), ["u"]);
    assert_eq!(found.results[0].title_indices, [0, 1, 2, 3, 4, 5, 6, 7]);
    let mut sentinel = request("new maker");
    sentinel.unnamed_label = Some("Untitled task");
    assert!(run(&db, &sentinel).results.is_empty());
    assert_eq!(ids(&run(&db, &request("new maker"))), ["u"]);
}

#[test]
fn filters_narrow_the_candidates() {
    let db = db();
    for (id, status, kind) in [
        ("a", "active", "cc"),
        ("r", "archived", "codex"),
        ("d", "deleted", "cc"),
    ] {
        task(&db, id, &format!("plan {id}"), NOW - 2 * DAY);
        db.execute(
            "UPDATE sessions SET status = ?, agent_kind = ? WHERE id = ?",
            (status, kind, id),
        )
        .unwrap();
    }
    task(&db, "old", "plan old", NOW - 10 * DAY);
    db.execute(
        "UPDATE sessions SET project_dir = '/repo', working_dir = '/elsewhere' WHERE id = 'a'",
        [],
    )
    .unwrap();
    let sorted = |r: Response| {
        let mut found: Vec<String> = r.results.iter().map(|i| i.session.id.clone()).collect();
        found.sort();
        found
    };
    assert_eq!(sorted(run(&db, &request("plan"))), ["a", "old", "r"]);
    let mut archived = request("plan");
    archived.status = "archived";
    assert_eq!(sorted(run(&db, &archived)), ["r"]);
    let mut codex = request("plan");
    codex.agent_kind = "codex";
    assert_eq!(sorted(run(&db, &codex)), ["r"]);
    let mut week = request("plan");
    week.last_activity = "7d";
    assert_eq!(sorted(run(&db, &week)), ["a", "r"]);
    db.execute(
        "UPDATE sessions SET user_send_at = ? WHERE id = 'old'",
        [NOW - DAY],
    )
    .unwrap();
    assert_eq!(
        sorted(run(&db, &week)),
        ["a", "old", "r"],
        "a recent send counts"
    );
    let dirs = ["/repo/".to_owned()];
    let mut project = request("plan");
    project.working_dirs = Some(&dirs);
    assert_eq!(
        sorted(run(&db, &project)),
        ["a"],
        "project_dir before working_dir"
    );
    let picked = [" r ".to_owned(), "r".to_owned(), String::new()];
    let mut only = request("plan");
    only.session_ids = Some(&picked);
    assert_eq!(sorted(run(&db, &only)), ["r"]);
    let none: [String; 1] = [" ".to_owned()];
    only.session_ids = Some(&none);
    let empty = run(&db, &only);
    assert_eq!((empty.query.as_str(), empty.results.len()), ("plan", 0));
    assert_eq!(run(&db, &request("   ")).query, "");
}

#[test]
fn summaries_carry_companions_and_live_message_counts() {
    let db = db();
    task(&db, "c", "chat canonical", 3);
    task(&db, "h", "chat history", 2);
    task(&db, "x", "chat leaving", 1);
    db.execute_batch(
        "INSERT INTO bot_profiles VALUES ('b', 'Ada', 'active'), ('gone', 'Old', 'deleting');
         INSERT INTO bot_session_links VALUES ('l1', 'b', 'c', 'canonical'), ('l2', 'b', 'h', 'history'),
           ('l3', 'gone', 'x', 'canonical');",
    )
    .unwrap();
    say(&db, "m1", "c", "\"hi\"");
    say(&db, "m2", "c", "\"hi again\"");
    db.execute("UPDATE messages SET rewind_at = 5 WHERE id = 'm2'", [])
        .unwrap();
    let response = run(&db, &request("chat"));
    assert_eq!(
        ids(&response),
        ["h", "c"],
        "a bot being deleted hides its tasks"
    );
    let summary = &response.results[1].session;
    let companion = summary.companion.as_ref().unwrap();
    assert_eq!(
        (companion.name.as_str(), companion.read_only),
        ("Ada", false)
    );
    assert!(
        response.results[0]
            .session
            .companion
            .as_ref()
            .unwrap()
            .read_only
    );
    assert_eq!((summary.messages, summary.agent_kind), (1, "cc"));
    assert_eq!(summary.workspace_kind, "project");
    assert_eq!(
        (summary.status.as_str(), summary.source.as_str()),
        ("active", "desktop")
    );
    assert_eq!(
        (
            summary.title.as_str(),
            summary.created_at,
            summary.updated_at
        ),
        ("chat canonical", 0, 3)
    );
    assert!(summary.working_dir.is_none() && summary.orca_role.is_none());
    assert!(summary.parent_session_id.is_none() && summary.user_send_at.is_none());
}

#[test]
fn the_activity_sort_and_the_limit_apply() {
    let db = db();
    for (id, at) in [("a", 3), ("b", 1), ("c", 2)] {
        task(&db, id, "report", at);
    }
    let mut newest = request("report");
    newest.sort = "activityDesc";
    assert_eq!(ids(&run(&db, &newest)), ["a", "c", "b"]);
    newest.sort = "activityAsc";
    newest.limit = Some(2.9);
    assert_eq!(ids(&run(&db, &newest)), ["b", "c"]);
    assert_eq!(
        [None, Some(f64::NAN), Some(0.0), Some(80.0)].map(clamp_limit),
        [20, 20, 1, 50]
    );
}
