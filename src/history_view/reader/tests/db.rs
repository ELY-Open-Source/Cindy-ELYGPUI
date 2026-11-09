use rusqlite::Connection;

use super::*;
use crate::{i18n::Translator, localdb::search::mcp_app::TrustLabels};

fn put(db: &Connection, n: i64, role: &str, content: &str, tool: Option<&str>, meta: Option<&str>) {
    db.execute(
        "INSERT INTO messages (id, client_id, session_id, role, content, tool_use_id, agent_meta, created_at)
         VALUES (?, ?, 's1', ?, ?, ?, ?, ?)",
        (n.to_string(), format!("c{n}"), role, content, tool, meta, 1_700_000_000_000 + n),
    )
    .unwrap();
}

#[test]
fn the_database_outline_carries_artifacts_and_details_read_whole_rows() {
    let db = crate::history_view::rows::tests::db();
    put(&db, 0, "user", "\"Write it\"", None, None);
    let call =
        r#"{"toolName":"Bash","toolUseId":"t1","input":{"command":"echo ok > /work/report.txt"}}"#;
    put(&db, 1, "tool_use", call, Some("t1"), None);
    put(&db, 2, "tool_result", "\"ok\"", Some("t1"), None);
    put(
        &db,
        3,
        "assistant",
        "\"Done\"",
        None,
        Some(r#"{"turnCompleted":true}"#),
    );
    let labels = TrustLabels::new(&Translator::new("en").unwrap());
    let mut deps = Db {
        db: &db,
        open: 7,
        labels: &labels,
    };
    let mut reader = Reader::default();
    let page = reader.page(&mut deps, "s1", None, true).unwrap();
    assert_eq!(kinds(&page.items), ["messages", "work", "messages"]);
    assert!(!page.has_more);
    let work = summary(&page.items[1]);
    let paths: Vec<&str> = work
        .artifacts
        .iter()
        .flatten()
        .filter_map(|a| a.obj()?.get("path")?.str())
        .collect();
    assert_eq!(paths, ["/work/report.txt"]);
    let revision = crate::history_view::rows::revision(&db, 7).unwrap();
    assert!(
        work.revision
            .ends_with(&format!(":{}:{revision}", reader.epoch))
    );
    let r: Ref = work.into();
    let details = reader.details(&mut deps, "s1", &r, None).unwrap();
    assert_eq!(ids(&details.messages), ["1", "2"]);
    assert_eq!(
        crate::history_view::content(&details.messages[0]),
        &js(call)
    );
    db.execute("UPDATE messages SET rewind_at = 1 WHERE id = '2'", [])
        .unwrap();
    assert_eq!(
        code(reader.details(&mut deps, "s1", &r, None)).0,
        "NOT_FOUND"
    );
}
