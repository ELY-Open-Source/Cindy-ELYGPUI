//! `searchConversations` for this computer's own search: candidate tasks, title matches, the
//! content pages, then the merged results with their message counts.

use std::collections::{HashMap, HashSet};

use rusqlite::{Connection, Row, types::Value};

use super::{
    PREVIEW_CHARS, fts,
    fuzzy::{TitleMatch, title_match},
    history::{self, Args},
    mcp_app::TrustLabels,
    merge::{Companion, ContentHit, Item, Sort, Summary, merge, sign},
    snippet::content_preview,
    visible::search_text,
};
use crate::{
    sessions::{paths, title::DRAFT},
    theme_import::color::js_trim,
};

const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT: usize = 50;
const PAGE_LIMIT: usize = 50;
const MAX_PAGES: usize = 3;
const POOL_LIMIT: usize = PAGE_LIMIT * MAX_PAGES;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;

/// `ConversationSearchRequest` as this computer's search sends it.
pub struct Request<'a> {
    pub query: &'a str,
    pub limit: Option<f64>,
    pub sort: &'a str,
    /// `semanticMode === 'keyword'`.
    pub keyword_only: bool,
    pub status: &'a str,
    pub agent_kind: &'a str,
    pub last_activity: &'a str,
    pub session_ids: Option<&'a [String]>,
    pub working_dirs: Option<&'a [String]>,
    pub unnamed_label: Option<&'a str>,
}

/// `ConversationSearchResponse`; `vectorUsed` stays false until 11.0.
#[derive(Debug, Default)]
pub struct Response {
    pub query: String,
    pub results: Vec<Item>,
    pub vector_skip_reason: Option<&'static str>,
    pub pool_capped: bool,
}

fn empty(query: &str) -> Response {
    Response {
        query: query.to_owned(),
        ..Response::default()
    }
}

/// `clampLimit`.
fn clamp_limit(value: Option<f64>) -> usize {
    match value {
        Some(v) if v.is_finite() => v.floor().clamp(1.0, MAX_LIMIT as f64) as usize,
        _ => DEFAULT_LIMIT,
    }
}

fn sort_by(value: &str) -> Sort {
    match value {
        "activityDesc" => Sort::ActivityDesc,
        "activityAsc" => Sort::ActivityAsc,
        _ => Sort::Relevance,
    }
}

/// `normalizeStatusFilter` without the retired `includeArchived`.
fn status(value: &str) -> &'static str {
    match value {
        "active" => "active",
        "archived" => "archived",
        _ => "all",
    }
}

fn agent(value: &str) -> Option<&'static str> {
    ["cc", "codex", "pi"].into_iter().find(|k| *k == value)
}

/// `cutoffForLastActivity`.
fn cutoff(value: &str, now: i64) -> Option<i64> {
    let days = match value {
        "1d" => 1,
        "3d" => 3,
        "7d" => 7,
        "30d" => 30,
        _ => return None,
    };
    Some(now - days * DAY_MS)
}

/// `normalizeSessionIds`: trimmed, unique, blanks dropped.
fn session_ids(value: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    value
        .iter()
        .map(|id| js_trim(id).to_owned())
        .filter(|id| !id.is_empty() && seen.insert(id.clone()))
        .collect()
}

/// `normalizeWorkingDirs`: the grouping form, unique; none left is no filter.
fn working_dirs(value: &[String]) -> Option<HashSet<String>> {
    let mut seen = Vec::new();
    for dir in value.iter().filter_map(|d| paths::grouping(Some(d))) {
        if !seen.contains(&dir) {
            seen.push(dir);
        }
    }
    let allowed: HashSet<String> = seen
        .iter()
        .filter_map(|d| paths::comparison(Some(d)))
        .collect();
    (!seen.is_empty()).then_some(allowed)
}

/// `sessionStatusesForFilter`.
fn statuses(status: &str) -> &'static [&'static str] {
    match status {
        "active" => &["active"],
        "archived" => &["archived"],
        _ => &["active", "archived"],
    }
}

/// `sessionSummaryFromRow` with no messages counted yet.
fn summary(row: &Row) -> rusqlite::Result<Summary> {
    let status: String = row.get(6)?;
    let link: (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = (row.get(13)?, row.get(14)?, row.get(15)?, row.get(16)?);
    let companion = match link {
        (Some(id), Some(name), bot_status, Some(role))
            if ["canonical", "history", "channel"].contains(&role.as_str()) =>
        {
            let read_only = role != "canonical"
                || status != "active"
                || bot_status.as_deref() != Some("active");
            Some(Companion {
                id,
                name,
                role,
                read_only,
            })
        }
        _ => None,
    };
    let kind: Option<String> = row.get(5)?;
    Ok(Summary {
        id: row.get(0)?,
        title: row.get(1)?,
        working_dir: row.get(2)?,
        project_dir: row.get(3)?,
        workspace_kind: row.get(4)?,
        agent_kind: match kind.as_deref() {
            Some("codex") => "codex",
            Some("pi") => "pi",
            _ => "cc",
        },
        status,
        source: row.get(7)?,
        orca_role: row.get(8)?,
        parent_session_id: row.get(9)?,
        companion,
        user_send_at: row.get(10)?,
        updated_at: row.get(11)?,
        created_at: row.get(12)?,
        messages: 0,
    })
}

/// `listSearchableSessions` for the owner's own search, newest first.
fn candidates(
    db: &Connection,
    status: &str,
    agent: Option<&str>,
    ids: Option<&[String]>,
    since: Option<i64>,
) -> rusqlite::Result<Vec<Summary>> {
    let mut conds = vec!["(p.id IS NULL OR p.status <> 'deleting')".to_owned()];
    let mut params: Vec<Value> = Vec::new();
    conds.push(match status {
        "active" => "s.status = 'active'".into(),
        "archived" => "s.status = 'archived'".into(),
        _ => "s.status <> 'deleted'".into(),
    });
    if let Some(kind) = agent {
        conds.push("s.agent_kind = ?".into());
        params.push(Value::Text(kind.into()));
    }
    if let Some(ids) = ids {
        conds.push(format!("s.id IN ({})", vec!["?"; ids.len()].join(",")));
        params.extend(ids.iter().cloned().map(Value::Text));
    }
    if let Some(since) = since {
        conds.push("(s.updated_at >= ? OR s.user_send_at >= ?)".into());
        params.extend([Value::Integer(since), Value::Integer(since)]);
    }
    let sql = format!(
        "SELECT s.id, s.title, s.working_dir, s.project_dir, s.workspace_kind, s.agent_kind,
                s.status, s.source, s.orca_role, s.parent_session_id, s.user_send_at,
                s.updated_at, s.created_at, p.id, p.display_name, p.status, l.role
           FROM sessions s
           LEFT JOIN bot_session_links l ON l.session_id = s.id
           LEFT JOIN bot_profiles p ON p.id = l.bot_id
          WHERE {}
          ORDER BY s.updated_at DESC",
        conds.join(" AND ")
    );
    let mut statement = db.prepare(&sql)?;
    let rows = statement.query_map(rusqlite::params_from_iter(params), summary)?;
    rows.collect()
}

/// `conversationSearchTitle`: the draft sentinel shows as the unnamed label.
pub fn shown_title<'a>(title: &'a str, unnamed: Option<&'a str>) -> &'a str {
    match unnamed {
        Some(label) if title == DRAFT && !label.is_empty() => label,
        _ => title,
    }
}

/// `activityMs` of the title order: the last send, else the update.
fn activity(s: &Summary) -> i64 {
    s.user_send_at.unwrap_or(s.updated_at)
}

/// `fetchMessageCounts`: rewound messages do not count.
fn message_counts(db: &Connection, ids: &[&str]) -> rusqlite::Result<HashMap<String, i64>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let sql = format!(
        "SELECT session_id, count(id) FROM messages
          WHERE session_id IN ({}) AND rewind_at IS NULL GROUP BY session_id",
        vec!["?"; ids.len()].join(",")
    );
    let mut statement = db.prepare(&sql)?;
    let rows = statement.query_map(rusqlite::params_from_iter(ids), |r| {
        Ok((r.get(0)?, r.get(1)?))
    })?;
    rows.collect()
}

/// `searchConversations(request)`; `now` is `Date.now()`.
pub fn search(
    db: &Connection,
    request: &Request,
    labels: &TrustLabels,
    now: i64,
) -> rusqlite::Result<Response> {
    let query = js_trim(request.query);
    if query.is_empty() {
        return Ok(empty(""));
    }
    let limit = clamp_limit(request.limit);
    let (status, agent) = (status(request.status), agent(request.agent_kind));
    let ids = request.session_ids.map(session_ids);
    if ids.as_ref().is_some_and(Vec::is_empty) {
        return Ok(empty(query));
    }
    let since = cutoff(request.last_activity, now);
    let mut rows = candidates(db, status, agent, ids.as_deref(), since)?;
    if let Some(allowed) = request.working_dirs.and_then(working_dirs) {
        rows.retain(|r| {
            let dir = r.project_dir.as_deref().or(r.working_dir.as_deref());
            paths::comparison(dir).is_some_and(|key| allowed.contains(&key))
        });
    }
    if rows.is_empty() {
        return Ok(empty(query));
    }
    let summaries: HashMap<&str, &Summary> = rows.iter().map(|r| (r.id.as_str(), r)).collect();
    let mut titles: Vec<(usize, &Summary, TitleMatch)> = rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| {
            let m = title_match(shown_title(&row.title, request.unnamed_label), query)?;
            Some((index, summaries[row.id.as_str()], m))
        })
        .collect();
    titles.sort_by(|a, b| {
        sign(b.2.score - a.2.score)
            .then(activity(b.1).cmp(&activity(a.1)))
            .then(a.0.cmp(&b.0))
    });
    let allowed: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
    let pool = history::prepare(
        db,
        &Args {
            query,
            session_ids: &allowed,
            agent_kind: agent,
            statuses: statuses(status),
            activity_from: since,
            skip_vector: request.keyword_only,
            fts_pool: POOL_LIMIT + 1,
            fuse_pool: POOL_LIMIT,
        },
    );
    // `collectContentHitsUntilUniqueSessions`.
    let target = (limit * 2).min(allowed.len());
    let (mut hits, mut unique, mut offset) = (Vec::new(), HashSet::new(), 0);
    for _ in 0..MAX_PAGES {
        let page = pool.page(db, (offset, PAGE_LIMIT), labels)?;
        unique.extend(page.hits.iter().map(|h| h.session_id.clone()));
        hits.extend(page.hits);
        match page.next_offset {
            Some(next) if unique.len() < target => offset = next,
            _ => break,
        }
    }
    let mut contents = Vec::new();
    for hit in hits {
        let Some(session) = summaries.get(hit.session_id.as_str()) else {
            continue;
        };
        let Some(message) = hit.message.filter(|m| !m.client_id.is_empty()) else {
            continue;
        };
        let meta = message.agent_meta.as_deref();
        let preview = content_preview(&hit.role, &message.content, query, PREVIEW_CHARS, meta);
        let visible = search_text(&hit.role, &message.content, meta);
        if !preview.keyword_matched && !fts::visible_matches(&visible, query) {
            continue;
        }
        let hit = ContentHit {
            message_id: hit.message_id,
            client_id: message.client_id,
            role: hit.role,
            created_at: hit.created_at,
            snippet: preview.snippet,
            preview: preview.preview,
            score: hit.score,
            fts_rank: Some(hit.fts_rank),
        };
        contents.push(((*session).clone(), hit));
    }
    let titles = titles.into_iter().map(|(_, s, m)| (s.clone(), m)).collect();
    let mut results = merge(titles, contents, limit, sort_by(request.sort));
    let ids: Vec<&str> = results.iter().map(|r| r.session.id.as_str()).collect();
    let counts = message_counts(db, &ids)?;
    for item in &mut results {
        item.session.messages = counts.get(&item.session.id).copied().unwrap_or(0);
    }
    log::info!(target: "conversation-search", "{} results, pool capped {}", results.len(), pool.capped);
    Ok(Response {
        query: query.to_owned(),
        results,
        vector_skip_reason: Some(pool.vector_skip_reason),
        pool_capped: pool.capped,
    })
}

#[cfg(test)]
pub(crate) mod tests;
