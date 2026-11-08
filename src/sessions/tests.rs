use rusqlite::Connection;

use super::{filter::Filter, fold, group, meta, query, row, settings, sweep, write};

/// The columns of Cindy's schema this chapter reads.
pub(super) fn db() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE sessions (id text PRIMARY KEY, title text NOT NULL DEFAULT 'New Maker',
           status text NOT NULL DEFAULT 'active', source text NOT NULL DEFAULT 'desktop',
           pinned_at integer, hidden_at integer, user_send_at integer, last_activity_at integer,
           created_at integer NOT NULL, updated_at integer NOT NULL, workspace_kind text DEFAULT 'project',
           working_dir text, project_dir text, remote_host_id text, list_message_count integer,
           agent_kind text NOT NULL DEFAULT 'cc', orca_role text, total_token_usage integer NOT NULL DEFAULT 0,
           total_cost_usd real NOT NULL DEFAULT 0, total_cost_amount real NOT NULL DEFAULT 0, total_cost_currency text,
           notifications_muted integer DEFAULT false NOT NULL, summary text, cleared_at integer,
           list_preview text, list_preview_role text);
         CREATE TABLE messages (id text PRIMARY KEY, session_id text NOT NULL,
           role text NOT NULL DEFAULT 'user', content text NOT NULL DEFAULT '\"\"',
           created_at integer NOT NULL DEFAULT 0, rewind_at integer, agent_meta text);
         CREATE TABLE bot_profiles (id text PRIMARY KEY, status text NOT NULL, hidden_at integer);
         CREATE TABLE bot_session_links (session_id text, bot_id text, role text);",
    )
    .unwrap();
    db
}

pub(super) fn add(db: &Connection, id: &str, source: &str, status: &str, updated: i64) {
    db.execute(
        "INSERT INTO sessions (id, title, status, source, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
        (id, format!("task {id}"), status, source, updated, updated),
    )
    .unwrap();
}

fn ids(rows: &[group::Session]) -> Vec<&str> {
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
    let active = query::list(&db, query::Status::Active).unwrap().0;
    assert_eq!(
        ids(&active),
        ["g", "b", "a", "routine"],
        "same updated_at: id DESC"
    );
    assert_eq!(
        ids(&query::list(&db, query::Status::Archived).unwrap().0),
        ["e"]
    );
    assert_eq!(
        ids(&query::list(&db, query::Status::All).unwrap().0),
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
        query::list(&db, query::Status::Active).unwrap().0[0].activity_at,
        15
    );
    db.execute(
        "UPDATE sessions SET last_activity_at = 25 WHERE id = 'a'",
        [],
    )
    .unwrap();
    assert_eq!(
        query::list(&db, query::Status::Active).unwrap().0[0].activity_at,
        25
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
            row::relative(1_000_000_000_000 - ago, 1_000_000_000_000, &t),
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
    db.execute(
        "INSERT INTO messages (id, session_id) VALUES ('m', 'chatty')",
        [],
    )
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

#[test]
fn reads_what_grouping_needs() {
    let db = db();
    add(&db, "cached", "desktop", "active", 2);
    add(&db, "counted", "desktop", "active", 1);
    db.execute_batch(
        "UPDATE sessions SET list_message_count = 7, workspace_kind = 'dialogue' WHERE id = 'cached';
         UPDATE sessions SET working_dir = '/w/app', remote_host_id = 'h' WHERE id = 'counted';
         INSERT INTO messages (id, session_id) VALUES ('m1', 'counted'), ('m2', 'counted');
         CREATE TABLE recent_workdirs (path text PRIMARY KEY, last_used_at integer NOT NULL);
         INSERT INTO recent_workdirs VALUES ('/old', 1), ('/new', 2);
         CREATE TABLE project_aliases (project_key text PRIMARY KEY, alias text NOT NULL, updated_at integer NOT NULL);
         INSERT INTO project_aliases VALUES ('local:/a', 'older', 1), ('local:/a/', 'newer', 2);",
    )
    .unwrap();
    let rows = query::list(&db, query::Status::Active).unwrap().0;
    assert_eq!(
        (rows[0].messages, rows[0].dialogue),
        (7, true),
        "the cached count wins"
    );
    assert_eq!(
        (rows[1].messages, rows[1].dialogue),
        (2, false),
        "else the messages are counted"
    );
    assert_eq!(rows[1].working_dir.as_deref(), Some("/w/app"));
    assert_eq!(rows[1].remote_host.as_deref(), Some("h"));
    db.execute_batch(
        "INSERT INTO recent_workdirs VALUES ('/w/app/', 0);
         INSERT INTO sessions (id, created_at, updated_at, working_dir, agent_kind)
           VALUES ('c', 1, 1, '/w/app/.worktrees/x', 'codex'), ('w', 1, 1, '/w/app', 'pi');
         UPDATE sessions SET orca_role = 'worker' WHERE id = 'w';",
    )
    .unwrap();
    let recent = query::recent(&db, "linux").unwrap();
    let paths: Vec<&str> = recent.iter().map(|r| r.path.as_str()).collect();
    assert_eq!(paths, ["/new", "/old", "/w/app/"]);
    assert_eq!(
        recent[2].kinds,
        ["codex"],
        "worktrees count, Orca workers and remote tasks do not"
    );
    let aliases = group::aliases(&query::aliases(&db).unwrap(), "linux");
    assert_eq!(
        aliases["local:/a"], "newer",
        "one alias per project, newest first"
    );
}

#[test]
fn folds_keep_the_first_and_the_recently_active() {
    for (raw, want) in [
        (None, 5),
        (Some("3"), 3),
        (Some(" 2px"), 2),
        (Some("+4"), 4),
        (Some("0"), 0),
        (Some("-0"), 0),
        (Some("-1"), 5),
        (Some("x"), 5),
    ] {
        assert_eq!(fold::limit(raw, 5), want, "{raw:?}");
    }
    let activity = [50, 40, 30, 99, 10];
    let (visible, hidden) = fold::view(&activity, 2, false, Some(90), |a| *a);
    assert_eq!((visible, hidden), (vec![&50, &40, &99], 2));
    assert_eq!(fold::view(&activity, 2, false, None, |a| *a).1, 3);
    assert_eq!(fold::view(&activity, 2, true, None, |a| *a).1, 0);
}

#[test]
fn reads_sidebar_settings_as_cindy_normalizes_them() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(settings::FILE);
    assert_eq!(
        settings::read(dir.path(), "linux"),
        settings::Settings::default(),
        "no file"
    );
    let long = "x".repeat(4097);
    std::fs::write(
        &file,
        format!(
            r#"{{"pinnedOrder":["a","",7,"a","{long}","project:local:/w/"],"other":1e400,
               "hiddenProjectKeys":["C:\\W\\App","local:c:/w/app","",3,"/srv/"]}}"#
        ),
    )
    .unwrap();
    let read = settings::read(dir.path(), "win32");
    assert_eq!(
        read.pinned_order,
        ["a", "project:local:/w/"],
        "pins keep their spelling"
    );
    assert_eq!(
        read.hidden,
        ["local:C:/W/App", "local:3", "local:/srv"],
        "one per comparison key"
    );
    std::fs::write(
        &file,
        r#"{"hiddenProjectKeys":[["local:/work/app"],null,1e400,false]}"#,
    )
    .unwrap();
    assert_eq!(
        settings::read(dir.path(), "linux").hidden,
        ["local:/work/app", "local:Infinity", "local:false"],
        "String() of every item but null"
    );
    for bad in ["[1]", "{", ""] {
        std::fs::write(&file, bad).unwrap();
        assert_eq!(
            settings::read(dir.path(), "linux"),
            settings::Settings::default(),
            "{bad:?}"
        );
        assert!(file.exists(), "an unreadable file stays for Cindy");
    }
}

#[test]
fn reads_the_filter_as_use_sidebar_filter_loads_it() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(crate::preferences::FILE);
    let read = |json: &str| {
        std::fs::write(&file, json).unwrap();
        Filter::read(&crate::preferences::Preferences::load(dir.path()).unwrap())
    };
    let plain = read("{}");
    assert_eq!(
        (
            plain.status,
            plain.vendor,
            plain.last_activity,
            plain.sort_by,
            plain.project_order
        ),
        ("active", "all", "all", "recency", "activity")
    );
    assert!(!plain.flat && plain.group_dialogue);
    let legacy =
        read(r#"{"cc-agent.sidebar.filter.sortBy":"manual","cc-agent.sidebar.filter.status":"x"}"#);
    assert_eq!(
        (legacy.sort_by, legacy.project_order, legacy.status),
        ("recency", "custom", "active")
    );
    let set = read(
        r#"{"cc-agent.sidebar.filter.sortBy":"manual","cc-agent.sidebar.filter.projectOrder":"activity",
            "cc-agent.sidebar.filter.lastActivity":"3d","cc-agent.sidebar.filter.vendor":"pi",
            "cc-agent.sidebar.filter.groupBy":"flat","cc-agent.sidebar.filter.groupDialogue":"false"}"#,
    );
    assert_eq!(
        set.project_order, "activity",
        "a stored order wins over the old sort"
    );
    assert!(set.flat && !set.group_dialogue);
    assert_eq!(set.cutoff(10 * 86_400_000), Some(7 * 86_400_000));
    assert_eq!(
        (plain.cutoff(1), plain.active_count(), set.active_count()),
        (None, 0, 2)
    );
}

#[test]
fn formats_tokens_and_money_as_cindy_does() {
    let path = format!("{}/tests/fixtures/meta.json", env!("CARGO_MANIFEST_DIR"));
    let golden: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    for case in golden["tokens"].as_array().unwrap() {
        let n = case[0].as_f64().unwrap() as i64;
        assert_eq!(meta::compact_tokens(n), case[1], "{n}");
    }
    for case in golden["money"].as_array().unwrap() {
        let (currency, amount) = (case[0].as_str().unwrap(), case[1].as_f64().unwrap());
        assert_eq!(
            meta::format_money((currency, amount)),
            case[2],
            "{currency} {amount}"
        );
    }
}

#[test]
fn joins_money_as_session_to_camel_does() {
    let db = db();
    add(&db, "s", "desktop", "active", 1);
    let mut s = query::list(&db, query::Status::Active).unwrap().0.remove(0);
    let at = |s: &group::Session| meta::money(s);
    assert_eq!(at(&s), None, "nothing spent");
    s.cost_usd = 2.0;
    assert_eq!(at(&s), Some(("USD", 2.0)), "legacy only");
    (s.cost_amount, s.cost_currency) = (3.0, Some("USD".into()));
    assert_eq!(at(&s), Some(("USD", 5.0)), "both USD add up");
    s.cost_currency = Some("CNY".into());
    assert_eq!(at(&s), Some(("CNY", 3.0)), "another currency wins");
    s.cost_currency = Some("EUR".into());
    assert_eq!(at(&s), Some(("USD", 2.0)), "an unknown currency is dropped");
    (s.cost_amount, s.cost_currency) = (f64::INFINITY, Some("CNY".into()));
    assert_eq!(at(&s), Some(("USD", 2.0)), "an infinite amount is dropped");
    (s.cost_usd, s.cost_amount, s.cost_currency) = (1e308, 1e308, Some("USD".into()));
    let sum = at(&s).unwrap();
    assert_eq!(meta::format_money(sum), "$Infinityk", "the sum overflows");
}

#[test]
fn reads_task_info_as_load_task_info_fields_does() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(crate::preferences::FILE);
    let read = |raw: Option<&str>| {
        let prefs = match raw {
            Some(raw) => serde_json::json!({ meta::TASK_INFO: raw }).to_string(),
            None => "{}".into(),
        };
        std::fs::write(&file, prefs).unwrap();
        meta::fields(&crate::preferences::Preferences::load(dir.path()).unwrap())
    };
    assert_eq!(read(None), ["time"]);
    assert_eq!(
        read(Some(r#"["cost","tags","time","cost",1]"#)),
        ["cost", "time"],
        "legacy array"
    );
    assert_eq!(
        read(Some(r#"{"version":1,"fields":["tokens"]}"#)),
        ["tokens"]
    );
    assert!(
        read(Some(r#"{"version":1,"fields":[]}"#)).is_empty(),
        "none is allowed"
    );
    assert_eq!(read(Some(r#"{"version":2,"fields":[]}"#)), ["time"]);
    assert_eq!(read(Some("{")), ["time"]);
}

#[test]
fn patches_metadata_as_update_session_in_db_does() {
    let db = db();
    add(&db, "s", "desktop", "active", 5);
    let read = |db: &Connection| {
        db.query_row(
            "SELECT title, notifications_muted, updated_at FROM sessions WHERE id = 's'",
            [],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, bool>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            },
        )
        .unwrap()
    };
    let before = read(&db).2;
    write::patch(&db, "s", &write::Meta::Title("Renamed".into())).unwrap();
    write::patch(&db, "s", &write::Meta::Muted(true)).unwrap();
    assert_eq!(
        read(&db),
        ("Renamed".into(), true, before),
        "no updatedAt bump"
    );
    assert!(write::patch(&db, "gone", &write::Meta::Muted(true)).is_err());
    assert_eq!(
        write::deep_link("a b/任"),
        "cindy://session/a%20b%2F%E4%BB%BB"
    );
}
