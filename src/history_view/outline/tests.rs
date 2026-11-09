use rusqlite::{Connection, types::Value};

use super::*;
use crate::rich_answer::js::{self, Js};

/// `outline` in Cindy's test: one row through the column, read back as `JSON.parse` would.
fn outline(role: &str, content: &str, agent_meta: &str) -> Js {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE messages (role TEXT, content TEXT, agent_meta TEXT)")
        .unwrap();
    db.execute(
        "INSERT INTO messages VALUES (?, ?, ?)",
        (role, content, agent_meta),
    )
    .unwrap();
    let text: String = db
        .query_row(
            &format!("SELECT {CONTENT} AS content FROM messages"),
            [],
            |r| r.get(0),
        )
        .unwrap();
    js::parse(&text).unwrap_or(Js::Str(text))
}

fn json(role: &str, content: &str) -> Js {
    outline(
        role,
        &js::stringify(&js::parse(content).unwrap(), ""),
        "null",
    )
}

fn raw(role: &str, content: &str) -> Js {
    outline(role, content, "null")
}

fn stamp(content: Value, sql: &str) -> Option<String> {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE messages (content TEXT)")
        .unwrap();
    db.execute("INSERT INTO messages VALUES (?)", [content])
        .unwrap();
    db.query_row(&format!("SELECT {sql} AS stamp FROM messages"), [], |r| {
        r.get(0)
    })
    .unwrap()
}

const LEGACY: &str = r#"length("messages"."content") || ':' || substr("messages"."content", -24)"#;

fn text(s: &str) -> Js {
    Js::Str(s.into())
}

#[test]
fn the_columns_are_the_sql_cindy_renders() {
    let golden = js::parse(include_str!(
        "../../../tests/fixtures/history-outline/sql.json"
    ))
    .unwrap();
    for (name, sql) in [("content", CONTENT), ("stamp", STAMP)] {
        let query = golden.obj().unwrap()[name].obj().unwrap();
        assert_eq!(query["sql"], text(sql), "{name}");
        assert_eq!(query["params"], Js::Arr(vec![]), "{name}");
    }
}

#[test]
fn folded_bodies_never_leave_the_database() {
    let body = "private heavy body ".repeat(100_000);
    let thinking = json(
        "thinking",
        &format!(r#"{{"text":"{body}","durationMs":1000}}"#),
    );
    assert!(js::stringify(&thinking, "").len() < 100);
    assert_eq!(
        js::stringify(&json("thinking", r#"{"text":"","durationMs":0}"#), ""),
        r#"{"isRedacted":false,"durationMs":0}"#
    );
    assert_eq!(json("tool_result", &js::quote(&body)), text(""));
    let write = json(
        "tool_use",
        &format!(
            r#"{{"toolName":"Write","input":{{"file_path":"/work/report.md","content":"{body}"}}}}"#
        ),
    );
    assert!(!js::stringify(&write, "").contains("private heavy"));
}

#[test]
fn plain_text_output_folds_but_keeps_deliveries_and_failures() {
    let body = "Checking original UI image catalogs: 55\n".repeat(50_000);
    assert_eq!(raw("tool_result", &body), text(""));
    assert_eq!(
        raw("tool_result", &format!("<tool_use_error>{body}")),
        text("<tool_use_error>")
    );
    for kept in [
        "saved cindy-media://blobs/a.png",
        "xdt-file:///work/report.pdf",
    ] {
        assert_eq!(raw("tool_result", kept), text(kept));
    }
    assert_eq!(
        raw("tool_result", &format!("const xdt_helper = 1;\n{body}")),
        text("")
    );
    assert_eq!(
        raw(
            "tool_result",
            &format!("{body}see xdt-file:///work/report.pdf")
        ),
        text("")
    );
}

#[test]
fn prose_artifacts_and_malformed_bodies_stay() {
    assert_eq!(json("assistant", r#""Final answer""#), text("Final answer"));
    assert_eq!(
        outline(
            "assistant",
            r#""Internal prose""#,
            r#"{"parentUuid":"agent"}"#
        ),
        text("")
    );
    let media = r#"{"xdt_image_url":"cindy-media://blobs/test.png"}"#;
    assert_eq!(raw("tool_result", &js::quote(media)), text(media));
    assert_eq!(
        json("tool_result", r#""xdt-file://report.pdf""#),
        text("xdt-file://report.pdf")
    );
    assert_eq!(raw("tool_use", "{broken"), text("{broken"));
    let changes = json(
        "tool_use",
        r#"{"toolName":"file_change","input":{"changes":["malformed"]}}"#,
    );
    assert!(changes.obj().is_some());
}

#[test]
fn commands_keep_their_full_text_for_artifact_reads() {
    let command = format!("echo '{}' > /work/trailing-report.txt", "x".repeat(300_000));
    let content = json(
        "tool_use",
        &format!(r#"{{"toolName":"Bash","input":{{"command":"{command}"}}}}"#),
    );
    let input = content.obj().unwrap()["input"].obj().unwrap();
    assert_eq!(input["command"], text(&command));
    assert_eq!(input["file_path"], Js::Null);
}

#[test]
fn structured_failures_fold_to_the_error_mark() {
    for result in [
        r#"{"ok":false}"#,
        r#"{"success":false}"#,
        r#"{"status":"FAILED"}"#,
    ] {
        assert_eq!(
            json("tool_result", &js::quote(result)),
            text("<tool_use_error>"),
            "{result}"
        );
    }
    assert_eq!(json("tool_result", &js::quote(r#"{"ok":true}"#)), text(""));
}

#[test]
fn document_deliveries_keep_their_whole_call() {
    let delivery = r#"{"toolName":"cindy_mcp_call_tool","input":{"server":"documents","tool":"render_pdf","args":{"outPath":"/work/report.pdf","htmlPath":"/work/source.html"}}}"#;
    assert_eq!(js::stringify(&json("tool_use", delivery), ""), delivery);
    let edit = json(
        "tool_use",
        r#"{"toolName":"Edit","input":{"file_path":"/work/code.ts","old_string":"heavy","new_string":"heavy"}}"#,
    );
    assert!(!js::stringify(&edit, "").contains("heavy"));
}

#[test]
fn stamps_change_with_characters_not_bytes() {
    let stamped = |s: &str| stamp(Value::Text(s.into()), STAMP);
    let legacy = |s: &str| stamp(Value::Text(s.into()), LEGACY);
    let summary = js::quote("build summary");
    let full = js::quote(&format!(
        "build summary\n{}",
        "full log line\n".repeat(50_000)
    ));
    assert_ne!(stamped(&summary), stamped(&full));
    assert!(stamped(&full).unwrap().len() < 256);
    let tail = "q".repeat(120);
    let pairs = [
        (format!("ab{tail}"), format!("é{tail}")),
        (format!("{tail}中文"), format!("{tail}中义")),
        (format!("{}a", "😀".repeat(23)), "😀".repeat(24)),
        ("😀".repeat(24), format!("{}😁", "😀".repeat(23))),
        (
            format!("{}{tail}", "e\u{301}".repeat(30)),
            format!("{}e{tail}", "e\u{301}".repeat(29)),
        ),
        (format!("plain text {tail}a"), format!("plain text {tail}b")),
    ];
    for (before, after) in pairs {
        assert_ne!(legacy(&before), legacy(&after));
        assert_ne!(stamped(&before), stamped(&after), "{before}");
    }
    for content in [
        Value::Null,
        Value::Text(String::new()),
        Value::Text(format!("prefix\0{}", "😀".repeat(30))),
    ] {
        assert_eq!(stamp(content.clone(), STAMP), stamp(content, LEGACY));
    }
}
