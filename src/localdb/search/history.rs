//! `prepareChatHistorySearch` as conversation search runs it: the FTS arm, fusion, the pool cap,
//! and each page's hit rows (`contextRadius: 0`). The vector arm waits for 11.0.

use std::collections::HashMap;

use rusqlite::{Connection, types::Value};

use super::{
    fts::{self, Fused},
    mcp_app::{self, TrustLabels},
};
use crate::rich_answer::js::{self, Js};

/// `chatHistorySearch`'s logger scope.
const SCOPE: &str = "chat-history-search";
const ROLES: [&str; 4] = ["user", "assistant", "ask_user", "plan_review"];

/// The engine arguments conversation search sets; the rest are `null` there.
pub struct Args<'a> {
    pub query: &'a str,
    pub session_ids: &'a [String],
    pub agent_kind: Option<&'a str>,
    pub statuses: &'a [&'a str],
    /// `sessionActivityFromMs`.
    pub activity_from: Option<i64>,
    pub skip_vector: bool,
    pub fts_pool: usize,
    pub fuse_pool: usize,
}

/// One invocation's ranked pool.
pub struct Pool {
    entries: Vec<Fused>,
    meta: HashMap<String, Meta>,
    pub capped: bool,
    pub vector_skip_reason: &'static str,
}

struct Meta {
    session_id: String,
    role: String,
    created_at: i64,
}

/// The stored message behind a hit, as `messageToCamel` reads it.
pub struct Message {
    pub content: Js,
    pub client_id: String,
    pub agent_meta: Option<String>,
}

pub struct Hit {
    pub message_id: String,
    pub session_id: String,
    pub role: String,
    pub created_at: i64,
    pub score: f64,
    pub fts_rank: usize,
    /// None once the row is gone, as an empty `context`.
    pub message: Option<Message>,
}

pub struct Page {
    pub hits: Vec<Hit>,
    pub next_offset: Option<usize>,
}

/// `buildFilterClause` for the fields conversation search sets.
fn filter(args: &Args) -> (String, Vec<Value>) {
    let marks = |n: usize| vec!["?"; n].join(",");
    let mut conds = vec!["m.rewind_at IS NULL".to_owned()];
    let mut params: Vec<Value> = Vec::new();
    if !args.session_ids.is_empty() {
        conds.push(format!(
            "m.session_id IN ({})",
            marks(args.session_ids.len())
        ));
        params.extend(args.session_ids.iter().cloned().map(Value::Text));
    }
    if let Some(kind) = args.agent_kind {
        conds.push("s.agent_kind = ?".into());
        params.push(Value::Text(kind.into()));
    }
    if !args.statuses.is_empty() {
        conds.push(format!("s.status IN ({})", marks(args.statuses.len())));
        params.extend(args.statuses.iter().map(|s| Value::Text((*s).into())));
    }
    if let Some(from) = args.activity_from {
        conds.push(
            "(s.updated_at >= ? OR (s.user_send_at IS NOT NULL AND s.user_send_at >= ?))".into(),
        );
        params.extend([Value::Integer(from), Value::Integer(from)]);
    }
    conds.push("(s.cleared_at IS NULL OR m.created_at > s.cleared_at)".into());
    conds.push(format!("m.role IN ({})", marks(ROLES.len())));
    params.extend(ROLES.iter().map(|r| Value::Text((*r).into())));
    (conds.join(" AND "), params)
}

/// `runFtsArm`: best `bm25` first; a failed query is logged and finds nothing, as Cindy's does.
fn fts_arm(db: &Connection, args: &Args) -> Vec<(String, Meta)> {
    let Some(matched) = fts::messages_match(args.query) else {
        return Vec::new();
    };
    let (clause, params) = filter(args);
    let sql = format!(
        "SELECT m.id, m.session_id, m.role, m.created_at
           FROM messages_fts
           JOIN messages m ON m.id = messages_fts.message_id
           JOIN sessions s ON s.id = m.session_id
          WHERE messages_fts MATCH ? AND {clause}
          ORDER BY bm25(messages_fts)
          LIMIT ?"
    );
    let mut bound = vec![Value::Text(matched)];
    bound.extend(params);
    bound.push(Value::Integer(args.fts_pool as i64));
    let rows = db.prepare(&sql).and_then(|mut statement| {
        let rows = statement.query_map(rusqlite::params_from_iter(bound), |row| {
            let meta = Meta {
                session_id: row.get(1)?,
                role: row.get(2)?,
                created_at: row.get(3)?,
            };
            Ok((row.get(0)?, meta))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
    });
    rows.unwrap_or_else(|error| {
        log::warn!(target: SCOPE, "{{\"event\":\"chatHistorySearch.fts.error\",\"error\":{}}}", js::quote(&error.to_string()));
        Vec::new()
    })
}

/// `runVectorArm`'s first gates: no chat embedding runs before 11.0.
fn vector_skip_reason(skip: bool) -> &'static str {
    if skip {
        "本次请求已禁用语义检索, 仅用 FTS。"
    } else {
        "聊天记录语义索引未启用, 本次仅用 FTS 全文检索。"
    }
}

pub fn prepare(db: &Connection, args: &Args) -> Pool {
    let rows = fts_arm(db, args);
    let ranked: Vec<String> = rows.iter().map(|(id, _)| id.clone()).collect();
    let mut entries = fts::fuse(&ranked);
    let capped = entries.len() > args.fuse_pool;
    if capped {
        log::info!(target: SCOPE, "{{\"event\":\"chatHistorySearch.poolCapped\",\"total\":{},\"cap\":{}}}", entries.len(), args.fuse_pool);
        entries.truncate(args.fuse_pool);
    }
    log::debug!(target: SCOPE, "fts {} hits, pool {}", ranked.len(), entries.len());
    Pool {
        entries,
        meta: rows.into_iter().collect(),
        capped,
        vector_skip_reason: vector_skip_reason(args.skip_vector),
    }
}

/// `agentMeta.mcpApp === true` over `JSON.parse`; a meta that does not parse is `null`.
fn mcp_app_marked(agent_meta: Option<&str>) -> bool {
    agent_meta
        .and_then(|m| js::parse(m).ok())
        .is_some_and(|m| m.obj().and_then(|o| o.get("mcpApp")) == Some(&Js::Bool(true)))
}

/// `messageToCamel`'s content: parsed JSON, else the text; App rows as their public history.
fn camel_content(raw: String, row: (&str, &str, Option<&str>), labels: &TrustLabels) -> Js {
    let (role, client_id, agent_meta) = row;
    let mut content = js::parse(&raw).unwrap_or(Js::Str(raw));
    if role == "assistant" && client_id.starts_with("cloud-history:") {
        content = imported_reply_text(content);
    }
    let content = match &content {
        Js::Str(text) if role == "assistant" => {
            Js::Str(crate::sessions::preview::strip_internal_web_citations(text))
        }
        _ => content,
    };
    mcp_app::project(content, mcp_app_marked(agent_meta), labels)
}

/// `importedReplyText`: text blocks joined, anything else left as it is.
fn imported_reply_text(content: Js) -> Js {
    let joined = match &content {
        Js::Arr(blocks) if !blocks.is_empty() => blocks
            .iter()
            .map(|b| {
                let b = b.obj()?;
                (b.get("type")?.str()? == "text").then_some(())?;
                b.get("text")?.str()
            })
            .collect::<Option<Vec<_>>>()
            .map(|texts| texts.concat()),
        _ => None,
    };
    joined.map_or(content, Js::Str)
}

/// `fetchAnchorContexts`: the page's rows in one read.
fn messages(
    db: &Connection,
    ids: &[&str],
    labels: &TrustLabels,
) -> rusqlite::Result<HashMap<String, Message>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let sql = format!(
        "SELECT id, role, content, client_id, agent_meta FROM messages WHERE id IN ({})",
        vec!["?"; ids.len()].join(",")
    );
    let mut statement = db.prepare(&sql)?;
    let rows = statement.query_map(rusqlite::params_from_iter(ids), |row| {
        let (role, client_id): (String, String) = (row.get(1)?, row.get(3)?);
        let agent_meta: Option<String> = row.get(4)?;
        let shape = (role.as_str(), client_id.as_str(), agent_meta.as_deref());
        let message = Message {
            content: camel_content(row.get(2)?, shape, labels),
            client_id,
            agent_meta,
        };
        Ok((row.get(0)?, message))
    })?;
    rows.collect()
}

impl Pool {
    /// One page of the pool from `offset`.
    pub fn page(
        &self,
        db: &Connection,
        (offset, limit): (usize, usize),
        labels: &TrustLabels,
    ) -> rusqlite::Result<Page> {
        let start = offset.min(self.entries.len());
        let end = (start + limit).min(self.entries.len());
        let entries = &self.entries[start..end];
        let ids: Vec<&str> = entries.iter().map(|e| e.message_id.as_str()).collect();
        let mut rows = messages(db, &ids, labels)?;
        let hits = entries
            .iter()
            .map(|entry| {
                let meta = &self.meta[&entry.message_id];
                Hit {
                    message_id: entry.message_id.clone(),
                    session_id: meta.session_id.clone(),
                    role: meta.role.clone(),
                    created_at: meta.created_at,
                    score: entry.score,
                    fts_rank: entry.fts_rank,
                    message: rows.remove(&entry.message_id),
                }
            })
            .collect();
        let next_offset = (start + limit < self.entries.len()).then_some(start + limit);
        Ok(Page { hits, next_offset })
    }
}

#[cfg(test)]
mod tests;
