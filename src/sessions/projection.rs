//! `sessionListProjection.sql.ts`: the latest visible message's text for the list, and
//! `scheduleSessionListProjectionBackfill`, which caches it and the count in `sessions`.

use rusqlite::Connection;

use super::preview;

/// `LIST_PREVIEW_EXTRACT_SQL`.
const EXTRACT: &str = "CASE
  WHEN json_valid(m.content) = 0 THEN substr(m.content, 1, 512)
  WHEN json_type(m.content) = 'text' THEN substr(json_extract(m.content, '$'), 1, 512)
  WHEN m.role = 'user' AND json_type(m.content) = 'object' THEN substr(json_extract(m.content, '$.text'), 1, 512)
  ELSE NULL
END";

/// `LATEST_VISIBLE_PREVIEW_FILTER_SQL`.
const VISIBLE: &str = "m.role IN ('user', 'assistant')
    AND m.rewind_at IS NULL
    AND (m.role != 'user' OR m.agent_meta IS NULL OR CASE WHEN json_valid(m.agent_meta) THEN json_extract(m.agent_meta, '$.autoResume') END IS NOT 1)";

const LATEST: &str = "FROM messages m
      WHERE m.session_id = sessions.id
        AND VISIBLE
        AND (sessions.cleared_at IS NULL OR m.created_at > sessions.cleared_at)
      ORDER BY m.created_at DESC, m.rowid DESC LIMIT 1";

/// `SESSION_LIST_MESSAGE_COUNT_CAP`.
const COUNT_CAP: i64 = 1001;

fn latest() -> String {
    LATEST.replace("VISIBLE", VISIBLE)
}

/// `LATEST_MSG_EXTRACT_SQL`, `LATEST_MSG_ROLE_SQL`, and whether each cache column is empty.
pub fn columns() -> String {
    let latest = latest();
    format!(
        "CASE WHEN sessions.list_preview IS NOT NULL THEN NULL ELSE (SELECT {EXTRACT} {latest}) END,
         CASE WHEN sessions.list_preview_role IS NOT NULL THEN sessions.list_preview_role
              ELSE (SELECT m.role {latest}) END,
         sessions.list_preview IS NULL, sessions.list_message_count IS NULL"
    )
}

/// `SessionListProjectionBackfillItem`.
#[derive(Debug, PartialEq)]
pub struct Item {
    id: String,
    preview: Option<(String, Option<String>)>,
    count: Option<i64>,
}

impl Item {
    pub fn id(&self) -> &str {
        &self.id
    }
}

/// One row's backfill: its preview while uncached, its count while uncached.
pub fn item(
    id: &str,
    extract: Option<&str>,
    role: Option<&str>,
    uncached: (bool, bool),
    messages: i64,
) -> Option<Item> {
    let preview = extract
        .filter(|_| uncached.0)
        .and_then(|text| preview::finalize(Some(text), role))
        .map(|p| (p, role.map(str::to_owned)));
    let count = uncached.1.then_some(messages);
    (preview.is_some() || count.is_some()).then(|| Item {
        id: id.to_owned(),
        preview,
        count,
    })
}

/// `serializeSessionListProjectionBackfill`.
fn payload(items: &[Item]) -> String {
    let items: Vec<serde_json::Value> = items
        .iter()
        .map(|item| {
            let (preview, role) = item.preview.clone().unzip();
            serde_json::json!({
                "id": item.id,
                "preview": preview,
                "role": role.flatten(),
                "count": item.count.map(|c| c.clamp(0, COUNT_CAP)),
                "hasPreview": u8::from(item.preview.is_some()),
                "hasCount": u8::from(item.count.is_some()),
            })
        })
        .collect();
    serde_json::Value::Array(items).to_string()
}

/// `SESSION_LIST_PROJECTION_BACKFILL_SQL`: the columns still empty, filled from `messages`.
pub fn backfill(db: &Connection, items: &[Item]) {
    if items.is_empty() {
        return;
    }
    let latest = latest();
    let sql = format!(
        "UPDATE sessions
SET
  list_preview = CASE
    WHEN sessions.list_preview IS NULL THEN (
      SELECT {EXTRACT} {latest}
    )
    ELSE sessions.list_preview
  END,
  list_preview_role = CASE
    WHEN sessions.list_preview IS NULL THEN (
      SELECT m.role {latest}
    )
    ELSE sessions.list_preview_role
  END,
  list_message_count = CASE
    WHEN sessions.list_message_count IS NULL THEN (
      SELECT count(*) FROM messages m WHERE m.session_id = sessions.id
    )
    ELSE sessions.list_message_count
  END
FROM (
  SELECT json_extract(value, '$.id') AS id FROM json_each(?)
) AS payload
WHERE sessions.id = payload.id"
    );
    match db.execute(&sql, [payload(items)]) {
        Ok(rows) => log::info!(target: "sessions", "list projection backfilled for {rows} tasks"),
        Err(error) => log::warn!(target: "sessions",
            "session list projection backfill failed: {} tasks, {error}", items.len()),
    }
}
