//! `historyView.ts`'s items and `historyViewProjection.ts`'s `projectHistoryView`.
#![cfg_attr(
    not(test),
    expect(dead_code, reason = "the history reader reads them in 9.1c")
)]

use std::collections::{HashMap, HashSet};

use crate::{
    js_date,
    message_rows::{content_to_preview, tool_use},
    rich_answer::js::{self, Js, Obj, truthy},
};

pub mod json;
mod source;

const LIVE: &str = "history-live:";

/// A row as `HistoryMessageSource` carries it, the whole object `messageToCamel` built.
pub type Row = Js;

/// `HistoryWorkSummary`; fields in the order Cindy's projection writes them.
#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub key: String,
    pub anchor_client_id: String,
    pub parent_tool_use_id: Option<String>,
    pub model: Option<String>,
    pub first_message_id: String,
    pub last_message_id: String,
    /// The stored bounds and live ids, set when a live row is in the range.
    pub stored: Option<Stored>,
    pub started_at_ms: f64,
    pub ended_at_ms: f64,
    pub artifacts: Option<Vec<Js>>,
    pub message_count: usize,
    pub tool_count: usize,
    pub is_streaming: bool,
    pub revision: String,
    pub preview: Option<Box<Summary>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stored {
    pub first: Option<String>,
    pub last: Option<String>,
    pub live: Vec<String>,
}

/// `HistoryViewItem`.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Messages {
        key: String,
        messages: Vec<Row>,
        deferred: Option<Summary>,
    },
    Work {
        key: String,
        summary: Summary,
        children: Option<Vec<Item>>,
    },
}

pub fn field<'a>(row: &'a Row, key: &str) -> Option<&'a Js> {
    row.obj().and_then(|o| o.get(key))
}

pub fn text<'a>(row: &'a Row, key: &str) -> &'a str {
    field(row, key).and_then(Js::str).unwrap_or("")
}

static NULL: Js = Js::Null;

pub fn content(row: &Row) -> &Js {
    field(row, "content").unwrap_or(&NULL)
}

pub fn meta(row: &Row) -> Option<&Obj> {
    field(row, "agentMeta").and_then(Js::obj)
}

/// `Date.parse(row.createdAt)`.
pub fn created_ms(row: &Row) -> f64 {
    field(row, "createdAt")
        .and_then(Js::str)
        .map_or(f64::NAN, js_date::parse)
}

/// The tool id `parseMessageToolUse` reads.
pub fn tool_id(row: &Row) -> Option<String> {
    tool_use(field(row, "toolUseId").and_then(Js::str), content(row)).tool_use_id
}

/// `historyRowsHash`: FNV-1a over the UTF-16 units of each row's `JSON.stringify`.
pub fn rows_hash<'a>(rows: impl IntoIterator<Item = &'a Row>) -> u32 {
    let mut hash: u32 = 2_166_136_261;
    for row in rows {
        for unit in js::stringify(row, "").encode_utf16() {
            hash = (hash ^ u32::from(unit)).wrapping_mul(16_777_619);
        }
    }
    hash
}

/// The live ids of a range and its stored bounds, when a live row is in it.
pub fn stored<'a>(ids: impl Iterator<Item = &'a str> + Clone) -> Option<Stored> {
    let live: Vec<String> = ids
        .clone()
        .filter(|i| i.starts_with(LIVE))
        .map(str::to_owned)
        .collect();
    if live.is_empty() {
        return None;
    }
    let mut kept = ids.filter(|i| !i.starts_with(LIVE));
    let first = kept.next().map(str::to_owned);
    let last = kept.last().map(str::to_owned).or_else(|| first.clone());
    Some(Stored { first, last, live })
}

/// `completesHistoryTurn`: a turn's end stamp, interrupted turns included.
pub fn completes_turn(row: &Row) -> bool {
    let Some(meta) = meta(row) else {
        return false;
    };
    let positive = |v: Option<&Js>| matches!(v, Some(Js::Num(n)) if n.is_finite() && *n > 0.0);
    let money = |v: Option<&Js>| positive(v.and_then(Js::obj).and_then(|o| o.get("amount")));
    meta.get("turnCompleted") == Some(&Js::Bool(true))
        || money(meta.get("userTurnCost"))
        || money(meta.get("turnCost"))
        || positive(meta.get("userTurnCostUsd"))
        || positive(meta.get("turnCostUsd"))
        || matches!(meta.get("turnUsageDetails"), Some(Js::Obj(_) | Js::Arr(_)))
}

/// `historySubagentScopes`: each row inside a complete subagent tree, to its root call.
pub fn subagent_scopes(rows: &[Row], root: Option<&str>) -> HashMap<String, String> {
    let mut parents: HashMap<String, Js> = HashMap::new();
    let mut agents: HashSet<String> = root.map(str::to_owned).into_iter().collect();
    for row in rows.iter().filter(|r| text(r, "role") == "tool_use") {
        let tool = tool_use(field(row, "toolUseId").and_then(Js::str), content(row));
        let Some(id) = tool.tool_use_id else { continue };
        if tool.tool_name == "Agent" {
            agents.insert(id.clone());
        }
        if let Some(parent) = meta(row)
            .and_then(|m| m.get("parentUuid"))
            .filter(|p| truthy(Some(p)))
        {
            parents.insert(id, parent.clone());
        }
    }
    fn agent<'a>(agents: &HashSet<String>, p: Option<&'a Js>) -> Option<&'a str> {
        p.and_then(Js::str).filter(|p| agents.contains(*p))
    }
    let mut out = HashMap::new();
    for row in rows {
        let own = meta(row)
            .and_then(|m| m.get("parentUuid"))
            .filter(|p| truthy(Some(p)));
        let by_tool = field(row, "toolUseId")
            .and_then(Js::str)
            .filter(|t| !t.is_empty())
            .and_then(|t| parents.get(t));
        let Some(mut parent) = agent(&agents, own.or(by_tool)) else {
            continue;
        };
        let mut seen = HashSet::new();
        while let Some(next) = agent(&agents, parents.get(parent)) {
            if !seen.insert(parent) {
                break;
            }
            parent = next;
        }
        out.insert(text(row, "id").to_owned(), parent.to_owned());
    }
    out
}

/// `projectHistoryView`.
pub fn project(rows: &[Row], streaming: bool, lazy_details: bool) -> Vec<Item> {
    if !lazy_details {
        return source::project(rows.to_vec(), streaming, true);
    }
    let scopes = subagent_scopes(rows, None);
    let mut visible_tools: HashSet<String> = rows
        .iter()
        .filter(|r| text(r, "role") == "tool_result")
        .filter(|r| crate::message_rows::has_visible_history_result(content(r)))
        .filter_map(|r| field(r, "toolUseId").and_then(Js::str).map(str::to_owned))
        .collect();
    visible_tools.extend(
        rows.iter()
            .filter(|r| source::has_file_delivery(r))
            .filter_map(tool_id),
    );
    let mut children: HashMap<String, Vec<Row>> = HashMap::new();
    let mut visible = Vec::new();
    for row in rows {
        let id = match text(row, "role") {
            "tool_use" => tool_id(row),
            _ => field(row, "toolUseId").and_then(Js::str).map(str::to_owned),
        };
        let shown = id.is_some_and(|id| !id.is_empty() && visible_tools.contains(&id));
        match scopes.get(text(row, "id")) {
            Some(parent) if !shown => children
                .entry(parent.clone())
                .or_default()
                .push(row.clone()),
            _ => visible.push(row.clone()),
        }
    }
    let mut results: HashMap<&str, &Row> = HashMap::new();
    for row in rows.iter().filter(|r| text(r, "role") == "tool_result") {
        if let Some(id) = field(row, "toolUseId").and_then(Js::str) {
            results.insert(id, row);
        }
    }
    let artifacts = |row: &Row| with_artifacts(row, &results);
    let projected = source::project(visible.iter().map(artifacts).collect(), streaming, false);
    attach(projected, &children, streaming, &artifacts)
}

/// `withArtifacts`: failed edits dropped, each file marked ready once its result exists.
fn with_artifacts(row: &Row, results: &HashMap<&str, &Row>) -> Row {
    let Some(files) = field(row, "historyArtifacts")
        .and_then(Js::arr)
        .filter(|f| !f.is_empty())
    else {
        return row.clone();
    };
    fn tool(file: &Js) -> Option<&str> {
        field(file, "toolUseId")
            .and_then(Js::str)
            .filter(|t| !t.is_empty())
    }
    let kept = files
        .iter()
        .filter(|file| {
            let result = tool(file).and_then(|t| results.get(t));
            result.is_none_or(|r| !content_to_preview(content(r)).contains("<tool_use_error>"))
        })
        .map(|file| {
            let mut file = file.clone();
            let ready = tool(&file).is_none_or(|t| results.contains_key(t));
            if let Js::Obj(fields) = &mut file {
                fields.insert("ready".into(), Js::Bool(ready));
            }
            file
        })
        .collect();
    let mut row = row.clone();
    if let Js::Obj(fields) = &mut row {
        fields.insert("historyArtifacts".into(), Js::Arr(kept));
    }
    row
}

/// `attach`: a subagent call's hidden body becomes its message item's deferred range.
fn attach(
    items: Vec<Item>,
    children: &HashMap<String, Vec<Row>>,
    streaming: bool,
    artifacts: &dyn Fn(&Row) -> Row,
) -> Vec<Item> {
    items
        .into_iter()
        .map(|item| match item {
            Item::Work {
                key,
                summary,
                children: nested,
            } => Item::Work {
                key,
                summary,
                children: nested.map(|n| attach(n, children, streaming, artifacts)),
            },
            Item::Messages {
                key,
                messages,
                deferred,
            } => {
                let header = messages.iter().find(|r| text(r, "role") == "tool_use");
                let id = header.and_then(tool_id);
                let body = id
                    .as_ref()
                    .and_then(|id| children.get(id))
                    .filter(|b| !b.is_empty());
                let deferred = match (header, id, body) {
                    (Some(header), Some(id), Some(body)) => {
                        Some(deferred_range(header, id, body, streaming, artifacts))
                    }
                    _ => deferred,
                };
                Item::Messages {
                    key,
                    messages,
                    deferred,
                }
            }
        })
        .collect()
}

fn deferred_range(
    header: &Row,
    id: String,
    body: &[Row],
    streaming: bool,
    artifacts: &dyn Fn(&Row) -> Row,
) -> Summary {
    let (first, last) = (&body[0], &body[body.len() - 1]);
    let model = body.iter().filter_map(meta).find_map(|m| {
        let child = m.get("parentUuid").and_then(Js::str) == Some(id.as_str());
        m.get("model")
            .and_then(Js::str)
            .filter(|s| child && !s.is_empty())
    });
    let collected: Vec<Js> = body
        .iter()
        .flat_map(|r| {
            field(&artifacts(r), "historyArtifacts")
                .and_then(Js::arr)
                .map(<[Js]>::to_vec)
                .unwrap_or_default()
        })
        .collect();
    Summary {
        key: format!("subagent-work-{}", text(header, "clientId")),
        anchor_client_id: text(header, "clientId").to_owned(),
        parent_tool_use_id: Some(id),
        model: model.map(str::to_owned),
        first_message_id: text(first, "id").to_owned(),
        last_message_id: text(last, "id").to_owned(),
        stored: stored(body.iter().map(|r| text(r, "id"))),
        started_at_ms: created_ms(first),
        ended_at_ms: created_ms(last),
        artifacts: Some(collected),
        message_count: body.len(),
        tool_count: body
            .iter()
            .filter(|r| text(r, "role") == "tool_use")
            .count(),
        is_streaming: streaming,
        revision: format!("{}:{}:{}", text(last, "id"), body.len(), rows_hash(body)),
        preview: None,
    }
}

#[cfg(test)]
mod tests;
