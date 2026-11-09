use rusqlite::Connection;

use super::{Interrupted, Pending, ToolUse, candidates, shadowed, tool_use};
use crate::rich_answer::js::Js;

const BOOT: i64 = 10_000;

fn db() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE sessions (id text PRIMARY KEY, status text NOT NULL DEFAULT 'active',
           source text NOT NULL DEFAULT 'desktop', hidden_at integer, cleared_at integer,
           active_turn_started_at integer, last_turn_ended_at integer);
         CREATE TABLE bot_profiles (id text PRIMARY KEY, status text NOT NULL);
         CREATE TABLE bot_session_links (session_id text, bot_id text, role text);
         CREATE TABLE messages (id text PRIMARY KEY, session_id text NOT NULL, role text NOT NULL,
           content text NOT NULL, created_at integer NOT NULL, rewind_at integer);",
    )
    .unwrap();
    db
}

/// A task whose turn started at `started` and never ended.
fn task(db: &Connection, id: &str, source: &str, started: i64) {
    db.execute(
        "INSERT INTO sessions (id, source, active_turn_started_at) VALUES (?, ?, ?)",
        (id, source, started),
    )
    .unwrap();
}

fn say(db: &Connection, session: &str, role: &str, content: &str, at: i64) {
    let id = format!("{session}-{at}-{role}");
    db.execute(
        "INSERT INTO messages (id, session_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)",
        (id, session, role, content, at),
    )
    .unwrap();
}

fn ids(db: &Connection, shadowed: &[String]) -> Vec<String> {
    let mut ids: Vec<String> = candidates(db, BOOT, shadowed)
        .unwrap()
        .into_iter()
        .map(|c| c.session_id)
        .collect();
    ids.sort();
    ids
}

#[test]
fn a_turn_from_before_boot_carries_its_last_message_and_tool() {
    let db = db();
    task(&db, "t", "desktop", 1_000);
    say(&db, "t", "user", "\"go\"", 900);
    say(
        &db,
        "t",
        "tool_use",
        r#"{"toolName":"Bash","input":{"command":"ls"}}"#,
        1_200,
    );
    say(&db, "t", "assistant", "\"ok\"", 1_500);
    say(
        &db,
        "t",
        "tool_use",
        r#"{"toolName":"Read","input":{"b":1,"a":2}}"#,
        1_400,
    );
    db.execute(
        "UPDATE messages SET rewind_at = 1 WHERE created_at = 1500",
        (),
    )
    .unwrap();
    let got = candidates(&db, BOOT, &[]).unwrap();
    let input = crate::rich_answer::js::parse(r#"{"b":1,"a":2}"#).unwrap();
    assert_eq!(
        got,
        [Pending {
            session_id: "t".into(),
            interrupted_at: 1_400,
            tool: Some(ToolUse {
                name: "Read".into(),
                input: Some(input),
            }),
        }]
    );
    task(&db, "quiet", "feishu", 2_000);
    say(&db, "quiet", "tool_use", r#"{"toolName":"Bash"}"#, 1_900);
    let quiet = candidates(&db, BOOT, &[]).unwrap();
    let quiet = quiet.iter().find(|c| c.session_id == "quiet").unwrap();
    assert_eq!(
        (quiet.interrupted_at, &quiet.tool),
        (2_000, &None),
        "none since the start: the start"
    );
}

#[test]
fn ended_cleared_running_closed_and_unlisted_turns_stay_out() {
    let db = db();
    for id in [
        "kept", "ended", "cleared", "running", "archived", "hidden", "review", "none",
    ] {
        task(
            &db,
            id,
            if id == "review" { "review" } else { "desktop" },
            1_000,
        );
    }
    let set = |sql: &str| db.execute(sql, ()).unwrap();
    set("UPDATE sessions SET last_turn_ended_at = 1000 WHERE id = 'ended'");
    set("UPDATE sessions SET cleared_at = 1001 WHERE id = 'cleared'");
    set("UPDATE sessions SET active_turn_started_at = 10000 WHERE id = 'running'");
    set("UPDATE sessions SET status = 'archived' WHERE id = 'archived'");
    set("UPDATE sessions SET hidden_at = 1 WHERE id = 'hidden'");
    set("UPDATE sessions SET active_turn_started_at = NULL WHERE id = 'none'");
    assert_eq!(ids(&db, &[]), ["kept"]);
}

#[test]
fn companion_tasks_follow_the_send_path_guards() {
    let db = db();
    let link = |id: &str, bot: &str, role: &str| {
        task(&db, id, "bot", 1_000);
        db.execute(
            "INSERT INTO bot_session_links VALUES (?, ?, ?)",
            (id, bot, role),
        )
        .unwrap();
    };
    db.execute_batch(
        "INSERT INTO bot_profiles VALUES ('on', 'active'), ('off', 'disabled'), ('far', 'active');",
    )
    .unwrap();
    link("main", "on", "canonical");
    link("chan", "on", "channel");
    link("group", "on", "group");
    link("paused", "off", "delegation");
    link("shadow", "far", "routine");
    link("shadow-task", "far", "delegation");
    link("orphan", "gone", "canonical");
    task(&db, "loose", "bot", 1_000);
    let far = ["far".to_owned()];
    assert_eq!(
        ids(&db, &far),
        ["chan", "main", "orphan", "shadow-task"],
        "a link without its profile is kept, as Cindy sees no companion task"
    );
}

#[test]
fn a_tool_use_is_an_object_with_a_named_tool() {
    assert_eq!(tool_use(None), None);
    assert_eq!(tool_use(Some("not json")), None);
    assert_eq!(tool_use(Some(r#"{"toolName":""}"#)), None);
    assert_eq!(tool_use(Some(r#"["toolName"]"#)), None);
    assert_eq!(tool_use(Some(r#"{"toolName":7}"#)), None);
    assert_eq!(
        tool_use(Some(r#"{"toolName":"Glob"}"#)),
        Some(ToolUse {
            name: "Glob".into(),
            input: None,
        })
    );
    let null = tool_use(Some(r#"{"toolName":"Glob","input":null}"#)).unwrap();
    assert_eq!(null.input, Some(Js::Null));
}

#[test]
fn shadowed_companions_come_from_the_owners_file() {
    let dir = tempfile::tempdir().unwrap();
    assert!(shadowed(dir.path()).is_empty(), "no file");
    let folder = dir.path().join("companion-cloud");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("shadowed.json"), r#"["b1", 2, "b2"]"#).unwrap();
    assert_eq!(shadowed(dir.path()), ["b1", "b2"]);
    std::fs::write(folder.join("shadowed.json"), "{oops").unwrap();
    assert!(shadowed(dir.path()).is_empty(), "a bad file reads as none");
}

#[gpui::test]
fn the_owners_load_runs_once_when_the_database_is_ready(cx: &mut gpui::TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    crate::sessions::view_tests::install(cx, dir.path());
    let db = db();
    task(&db, "t", "desktop", 1_000);
    let call = r#"{"toolName":"Bash","input":{"command":"pnpm test"}}"#;
    say(&db, "t", "tool_use", call, 1_100);
    task(&db, "quiet", "desktop", 1_000);
    cx.update(|cx| {
        cx.global_mut::<crate::sessions::Sessions>().owner_dir = Some(dir.path().to_owned());
        crate::localdb::serve(db, cx);
        super::load(cx);
    });
    cx.run_until_parked();
    let found = cx.update(|cx| {
        let store = cx.global::<Interrupted>();
        let mut lines: Vec<_> = store
            .candidates
            .as_ref()
            .unwrap()
            .iter()
            .map(|c| c.activity.clone())
            .collect();
        lines.sort();
        lines
    });
    assert_eq!(
        found,
        [None, Some("Ran tests".to_owned())],
        "the island's line, in English"
    );
    cx.update(|cx| {
        cx.global_mut::<Interrupted>().candidates = None;
        super::load(cx);
    });
    cx.run_until_parked();
    let again = cx.update(|cx| cx.global::<Interrupted>().candidates.is_none());
    assert!(again, "the same owner loads once");
}
