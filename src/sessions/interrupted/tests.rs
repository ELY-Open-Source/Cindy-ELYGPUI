use rusqlite::Connection;

use super::{Interrupted, Pending, ToolUse, candidates, shadowed, tool_use};
use crate::rich_answer::js::Js;

const BOOT: i64 = 10_000;

pub(super) fn db() -> Connection {
    let db = crate::sessions::tests::db();
    db.execute_batch(
        "ALTER TABLE sessions ADD COLUMN active_turn_started_at integer;
         ALTER TABLE sessions ADD COLUMN last_turn_ended_at integer;
         ALTER TABLE bot_profiles ADD COLUMN display_name text NOT NULL DEFAULT '';
         ALTER TABLE bot_profiles ADD COLUMN updated_at integer NOT NULL DEFAULT 0;
         ALTER TABLE bot_session_links ADD COLUMN created_at integer NOT NULL DEFAULT 0;",
    )
    .unwrap();
    db
}

/// A task whose turn started at `started` and never ended.
pub(super) fn task(db: &Connection, id: &str, source: &str, started: i64) {
    db.execute(
        "INSERT INTO sessions (id, source, active_turn_started_at, created_at, updated_at)
         VALUES (?1, ?2, ?3, 0, 0)",
        (id, source, started),
    )
    .unwrap();
}

pub(super) fn say(db: &Connection, session: &str, role: &str, content: &str, at: i64) {
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
            "INSERT INTO bot_session_links (session_id, bot_id, role) VALUES (?, ?, ?)",
            (id, bot, role),
        )
        .unwrap();
    };
    db.execute_batch(
        "INSERT INTO bot_profiles (id, status) VALUES ('on', 'active'), ('off', 'disabled'), ('far', 'active');",
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
            .tasks
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
        cx.global_mut::<Interrupted>().tasks = None;
        super::load(cx);
    });
    cx.run_until_parked();
    let again = cx.update(|cx| cx.global::<Interrupted>().tasks.is_none());
    assert!(again, "the same owner loads once");
}

#[test]
fn the_dialogs_tasks_keep_existing_rows_newest_first_with_companion_names() {
    let db = db();
    task(&db, "old", "desktop", 1_000);
    task(&db, "new", "desktop", 2_000);
    task(&db, "bot", "bot", 1_500);
    let link = |session: &str, bot: &str| {
        let sql =
            "INSERT INTO bot_session_links (session_id, bot_id, role) VALUES (?, ?, 'channel')";
        db.execute(sql, (session, bot)).unwrap();
    };
    db.execute_batch(
        "INSERT INTO bot_profiles (id, status, display_name, updated_at)
           VALUES ('fresh', 'active', 'Fresh', 20), ('stale', 'active', 'Stale', 10);",
    )
    .unwrap();
    link("bot", "fresh");
    link("bot", "stale");
    let read = super::tasks::read(&db, BOOT, &[]).unwrap();
    let t = crate::i18n::Translator::new("en").unwrap();
    let got: Vec<_> = super::tasks::assemble(read, &t)
        .into_iter()
        .map(|t| (t.session.id, t.companion))
        .collect();
    let stale = Some("Stale".to_owned());
    assert_eq!(
        got,
        [
            ("new".to_owned(), None),
            ("bot".to_owned(), stale.clone()),
            ("bot".to_owned(), stale),
            ("old".to_owned(), None),
        ],
        "the oldest profile's name wins; Cindy's join gives a row per link"
    );
}

#[test]
fn names_wait_for_a_companion_task_and_a_failed_read_gives_none() {
    let db = db();
    task(&db, "t", "desktop", 1_000);
    let rename = "ALTER TABLE bot_profiles RENAME COLUMN display_name TO name";
    db.execute_batch(rename).unwrap();
    let t = crate::i18n::Translator::new("en").unwrap();
    let read = super::tasks::read(&db, BOOT, &[]).unwrap();
    assert_eq!(
        super::tasks::assemble(read, &t).len(),
        1,
        "no companion task, no names read"
    );
    task(&db, "bot", "bot", 1_000);
    let sql =
        "INSERT INTO bot_session_links (session_id, bot_id, role) VALUES ('bot', 'b', 'channel')";
    db.execute(sql, ()).unwrap();
    let read = super::tasks::read(&db, BOOT, &[]).unwrap();
    let names: Vec<_> = super::tasks::assemble(read, &t)
        .into_iter()
        .map(|t| t.companion)
        .collect();
    assert_eq!(names, [None, None], "a failed name read gives none");
}

#[test]
fn get_many_reads_each_id_once_in_its_order() {
    let db = db();
    task(&db, "a", "desktop", 1_000);
    task(&db, "b", "desktop", 1_000);
    let ids = ["b", "x", "a", "b"].map(String::from);
    let rows = crate::sessions::query::by_ids(&db, &ids).unwrap();
    let got: Vec<_> = rows.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(got, ["b", "a"], "a missing id is left out");
    // One query per 256 ids: a 300-variable limit fails a single IN of 600.
    let limit = rusqlite::limits::Limit::SQLITE_LIMIT_VARIABLE_NUMBER;
    db.set_limit(limit, 300).unwrap();
    let many: Vec<String> = (0..600).map(|i| format!("m{i:03}")).rev().collect();
    for id in &many {
        task(&db, id, "desktop", 1_000);
    }
    let rows = crate::sessions::query::by_ids(&db, &many).unwrap();
    let got: Vec<_> = rows.iter().map(|s| s.id.clone()).collect();
    assert_eq!(got, many, "three batches, in the asked order");
}
