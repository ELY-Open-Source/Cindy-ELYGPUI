//! `historyViewReader.ts`: history pages and work details, read in budgets.

use std::collections::{HashMap, HashSet};

use indexmap::IndexMap;

use super::{
    Item, LIVE, Row, Summary, field, json, meta, rows::Error, rows::Opts, rows::Result,
    subagent_scopes, text, tool_id,
};
use crate::{
    message_rows::is_delivery_prose_text,
    rich_answer::js::{self, Js, truthy},
};

pub mod db;
mod details;
mod page;

/// `MAX_HISTORY_SCAN_ROWS` and `MAX_HISTORY_SCAN_BYTES`.
const MAX_SCAN_ROWS: usize = 2000;
const MAX_SCAN_BYTES: usize = 8 * 1024 * 1024;
/// `HISTORY_VIEW_PAGE_ITEMS`, `HISTORY_VIEW_PAGE_BYTES` and `HISTORY_DETAIL_PAGE_BYTES`.
const PAGE_ITEMS: usize = 20;
const PAGE_BYTES: usize = 1024 * 1024;
const DETAIL_PAGE_BYTES: usize = 256 * 1024;

/// `HistoryViewReaderDependencies`.
pub trait Deps {
    fn list(&mut self, session: &str, opts: &Opts) -> Result<Vec<Row>>;
    /// Both `outline` and `hydrate` are there.
    fn outlines(&self) -> bool;
    fn outline(&mut self, session: &str, opts: &Opts) -> Result<Vec<Row>>;
    fn hydrate(&mut self, session: &str, ids: &[String]) -> Result<Vec<Row>>;
    fn revision(&mut self) -> Option<Result<String>>;
    fn validate(&mut self, session: &str, ids: &[String]) -> Option<Result<()>>;
    fn anchor(&mut self, session: &str, id: &str, outline: bool) -> Result<Row>;
    fn running(&self, session: &str) -> bool;
    fn live(&self, session: &str) -> Vec<Row>;
}

/// `HistoryViewPage`.
#[derive(Debug, PartialEq)]
pub struct Page<T = Row> {
    pub items: Vec<Item<T>>,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}

/// `HistoryDetailPage`.
#[derive(Debug, PartialEq)]
pub struct DetailPage<T = Row> {
    pub messages: Vec<T>,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}

/// `HistoryWorkReference`.
#[derive(Clone, Debug, Default)]
pub struct Ref {
    pub key: String,
    pub first: String,
    pub last: String,
    pub first_stored: Option<String>,
    pub last_stored: Option<String>,
    pub live: Vec<String>,
    pub parent: Option<String>,
}

impl From<&Summary> for Ref {
    fn from(s: &Summary) -> Self {
        let stored = s.stored.as_ref();
        Ref {
            key: s.key.clone(),
            first: s.first_message_id.clone(),
            last: s.last_message_id.clone(),
            first_stored: stored.and_then(|s| s.first.clone()),
            last_stored: stored.and_then(|s| s.last.clone()),
            live: stored.map(|s| s.live.clone()).unwrap_or_default(),
            parent: s.parent_tool_use_id.clone(),
        }
    }
}

/// `createHistoryViewReader`'s state: the epoch and the detail cache.
pub struct Reader {
    epoch: String,
    read_revision: u64,
    detail_ranges: IndexMap<DetailKey, (Vec<Row>, usize)>,
    detail_revision: Option<String>,
    detail_bytes: usize,
}

type DetailKey = (String, String, String, String, Option<String>);

impl Default for Reader {
    fn default() -> Self {
        Reader {
            epoch: uuid::Uuid::new_v4().to_string(),
            read_revision: 0,
            detail_ranges: IndexMap::new(),
            detail_revision: None,
            detail_bytes: 0,
        }
    }
}

fn ipc(code: &'static str, message: &'static str) -> Error {
    Error::Ipc(code, message)
}

/// `Buffer.byteLength(JSON.stringify(value))`.
fn bytes(value: &Js) -> usize {
    js::stringify(value, "").len()
}

fn rows_bytes(rows: &[Row]) -> usize {
    2 + rows.iter().map(bytes).sum::<usize>() + rows.len().saturating_sub(1)
}

fn id(row: &Row) -> &str {
    text(row, "id")
}

fn is_live(id: &str) -> bool {
    id.starts_with(LIVE)
}

/// `compareRows`: time, then rowid.
fn compare(a: &Row, b: &Row) -> f64 {
    let rowid = |r: &Row| match field(r, "rowid") {
        Some(Js::Num(n)) => *n,
        _ => 0.0,
    };
    match super::created_ms(a) - super::created_ms(b) {
        d if d != 0.0 && !d.is_nan() => d,
        _ => rowid(a) - rowid(b),
    }
}

fn first_id(item: &Item) -> &str {
    match item {
        Item::Work { summary, .. } => &summary.first_message_id,
        Item::Messages { messages, .. } => id(&messages[0]),
    }
}

/// `isHistoryPageBoundary`: a user or system row, or a delivered answer.
fn is_boundary(row: &Row) -> bool {
    match text(row, "role") {
        "user" | "system" => true,
        "assistant" => {
            let meta = meta(row);
            !meta.is_some_and(|m| truthy(m.get("parentUuid")))
                && meta.and_then(|m| m.get("turnCompleted")) == Some(&Js::Bool(true))
                && field(row, "content")
                    .and_then(Js::str)
                    .is_some_and(is_delivery_prose_text)
        }
        _ => false,
    }
}

/// `cutsHistoryToolContext`: a suffix missing a tool header its rows name.
fn cuts_tool_context(rows: &[Row], start: usize) -> bool {
    let tools: HashSet<String> = rows[start..]
        .iter()
        .filter(|r| text(r, "role") == "tool_use")
        .filter_map(tool_id)
        .collect();
    rows[start..].iter().any(|row| {
        let parent = meta(row)
            .and_then(|m| m.get("parentUuid"))
            .and_then(Js::str);
        let result = field(row, "toolUseId").and_then(Js::str);
        parent.is_some_and(|p| !p.is_empty() && !tools.contains(p))
            || (text(row, "role") == "tool_result"
                && result.is_some_and(|t| !t.is_empty() && !tools.contains(t)))
    })
}

/// `historyPageSafeStarts`: cursors that keep each child with its Agent header.
fn safe_starts(rows: &[Row]) -> HashSet<String> {
    let scopes = subagent_scopes(rows, None);
    let mut headers = HashMap::new();
    for (index, row) in rows.iter().enumerate() {
        if text(row, "role") == "tool_use"
            && let Some(tool) = tool_id(row)
        {
            headers.insert(tool, index);
        }
    }
    let mut safe = HashSet::new();
    let mut required = rows.len();
    for (index, row) in rows.iter().enumerate().rev() {
        if let Some(parent) = scopes.get(id(row)) {
            required = required.min(headers.get(parent).copied().unwrap_or(index));
        }
        if required >= index {
            safe.insert(id(row).to_owned());
        }
    }
    safe
}

/// `historyPageStart`: twenty items, the byte limit over a whole tool span.
fn page_start(items: &[Item], safe: &HashSet<String>, measure: bool) -> Result<usize> {
    let mut start = items.len();
    let mut total = 1024;
    for index in (0..items.len()).rev() {
        if measure {
            total += bytes(&json::item(&items[index]));
            if index < items.len() - 1 && total > PAGE_BYTES {
                break;
            }
        }
        if !safe.contains(first_id(&items[index])) {
            continue;
        }
        start = index;
        if items.len() - index >= PAGE_ITEMS {
            break;
        }
    }
    if !items.is_empty() && start == items.len() {
        return Err(ipc(
            "UNSUPPORTED_CAPABILITY",
            "History page would split tool context",
        ));
    }
    Ok(start)
}

/// `mapHistoryViewMessages`, in place.
fn each_messages(items: &mut [Item], f: &mut impl FnMut(&mut Vec<Row>)) {
    for item in items {
        match item {
            Item::Messages { messages, .. } => f(messages),
            Item::Work {
                children: Some(children),
                ..
            } => each_messages(children, f),
            Item::Work { .. } => {}
        }
    }
}

/// `historyWorkSummaries`, in place: each leaf's summary, preview and deferred card.
fn each_summary(items: &mut [Item], f: &mut impl FnMut(&mut Summary)) {
    for item in items {
        match item {
            Item::Work {
                children: Some(children),
                ..
            } => each_summary(children, f),
            Item::Work { summary, .. } => {
                f(summary);
                if let Some(preview) = summary.preview.as_deref_mut() {
                    f(preview);
                }
            }
            Item::Messages {
                deferred: Some(deferred),
                ..
            } => f(deferred),
            Item::Messages { .. } => {}
        }
    }
}

#[cfg(test)]
mod fake;
#[cfg(test)]
mod tests;
