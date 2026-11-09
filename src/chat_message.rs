//! `makerChatStore.ts`'s `mapServerMessages`: stored rows as the chat's `ChatMessage`s.
#![cfg_attr(
    not(test),
    expect(dead_code, reason = "the chat route reads the store in 9.3")
)]

use std::collections::HashMap;

use indexmap::IndexMap;

use crate::{
    message_rows,
    rich_answer::js::{self, Js, Obj},
    theme_import::color::{js_space, js_trim},
};

mod assistant;
mod bot;
mod cards;
pub mod merge;
mod meta;
mod money;
mod references;
mod source;
mod tool;
pub mod transport;
mod usage;
mod user;
mod user_content;

/// A `ChatMessage`: `None` is a key Cindy's literal sets to `undefined`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Out(IndexMap<String, Option<Js>>);

impl Out {
    fn new() -> Self {
        Out::default()
    }

    fn insert(&mut self, key: String, value: Js) {
        self.0.insert(key, Some(value));
    }

    /// `key: value`, where `None` is `undefined`.
    fn set(&mut self, key: &str, value: Option<Js>) {
        self.0.insert(key.into(), value);
    }

    fn get(&self, key: &str) -> Option<&Js> {
        self.0.get(key)?.as_ref()
    }

    fn extend(&mut self, fields: impl IntoIterator<Item = (String, Js)>) {
        self.0.extend(fields.into_iter().map(|(k, v)| (k, Some(v))));
    }

    fn append(&mut self, other: Out) {
        self.0.extend(other.0);
    }

    pub fn into_fields(self) -> IndexMap<String, Option<Js>> {
        self.0
    }

    /// The keys set to `undefined`, as a JSON array.
    #[cfg(test)]
    pub fn undefined_keys(&self) -> Js {
        let keys = self.0.iter().filter(|(_, v)| v.is_none());
        Js::Arr(keys.map(|(k, _)| Js::Str(k.clone())).collect())
    }

    /// The object `JSON.stringify` sees: `undefined` keys left out.
    #[cfg(test)]
    pub fn json(&self) -> Js {
        let fields = self
            .0
            .iter()
            .filter_map(|(k, v)| Some((k.clone(), v.clone()?)));
        Js::Obj(fields.collect())
    }
}

fn str_of<'a>(o: &'a Obj, key: &str) -> Option<&'a str> {
    o.get(key)?.str()
}

/// `clientId`, `role` and `content`, the head of every branch.
fn base(row: &Obj, role: &Js, content: Js) -> Out {
    let mut out = Out::new();
    out.set("clientId", row.get("clientId").cloned());
    out.insert("role".into(), role.clone());
    out.insert("content".into(), content);
    out
}

/// `/\s+/g` as one space, then `trim`.
fn one_line(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut space = false;
    for c in value.chars() {
        match js_space(c) {
            true if space => {}
            true => {
                out.push(' ');
                space = true;
            }
            false => {
                out.push(c);
                space = false;
            }
        }
    }
    js_trim(&out).to_owned()
}

/// The subagent parent, lifted only in its SDK tool-parent form.
fn parent(meta: Option<&Obj>, out: &mut Out) {
    if let Some(p) = meta.and_then(|m| str_of(m, "parentUuid"))
        && message_rows::is_subagent_parent_tool_use_id(p)
    {
        out.insert("parentToolUseId".into(), Js::Str(p.into()));
    }
}

/// `subagent-model-chip`: the persisted model and parent.
fn subagent(meta: Option<&Obj>, out: &mut Out) {
    if let Some(model) = meta
        .and_then(|m| str_of(m, "model"))
        .filter(|m| !m.is_empty())
    {
        out.insert("model".into(), Js::Str(model.into()));
    }
    parent(meta, out);
}

/// `new Date(value).getTime()` for the values a row's `createdAt` holds.
fn time(value: Option<&Js>) -> f64 {
    match value {
        Some(Js::Str(s)) => crate::js_date::parse(s),
        Some(Js::Num(n)) if n.abs() <= 8.64e15 => n.trunc(),
        Some(Js::Null) => 0.0,
        Some(Js::Bool(b)) => f64::from(u8::from(*b)),
        _ => f64::NAN,
    }
}

/// `new Date(ms).toISOString()`, which throws past its range.
fn iso(ms: f64) -> Result<String, String> {
    let ms = ms.trunc();
    let valid = ms.is_finite() && ms.abs() <= 8.64e15;
    valid
        .then(|| crate::js_date::iso(ms as i64))
        .flatten()
        .ok_or_else(|| "Invalid time value".into())
}

/// `compareMessageTimeline`: time, then rowid; a missing one never decides.
fn timeline(a: &Obj, b: &Obj) -> std::cmp::Ordering {
    let at = |o: &Obj| match o.get("createdAt").filter(|v| js::truthy(Some(v))) {
        None => f64::INFINITY,
        v => Some(time(v))
            .filter(|t| t.is_finite())
            .unwrap_or(f64::INFINITY),
    };
    let rowid = |o: &Obj| match o.get("rowid") {
        Some(Js::Num(n)) if n.is_finite() => *n,
        _ => f64::NAN,
    };
    for diff in [at(a) - at(b), rowid(a) - rowid(b)] {
        if !diff.is_nan() && diff != 0.0 {
            return diff.total_cmp(&0.0);
        }
    }
    std::cmp::Ordering::Equal
}

/// `isHiddenThinkingRow`: redacted, Pi's old redacted placeholder, or the omitted-display stub.
fn hidden_thinking(row: &Obj) -> bool {
    let Some(c) = row
        .get("content")
        .filter(|c| matches!(c, Js::Obj(_) | Js::Arr(_)))
    else {
        return false;
    };
    if row.get("role") != Some(&Js::Str("thinking".into())) {
        return false;
    }
    let get = |key: &str| c.obj().and_then(|c| c.get(key));
    if get("isRedacted") == Some(&Js::Bool(true)) {
        return true;
    }
    let text = get("text").and_then(Js::str).unwrap_or("");
    let pi = row.get("agentKind") == Some(&Js::Str("pi".into()));
    let duration = match get("durationMs") {
        Some(Js::Num(n)) => *n,
        _ => 0.0,
    };
    (pi && js_trim(text) == "[Reasoning redacted]") || (text.is_empty() && duration == 0.0)
}

fn meta_of(row: &Obj) -> Option<&Obj> {
    row.get("agentMeta")?.obj()
}

/// `projectLegacyUserTurnCosts`: a running user-turn total for peers that never stored one.
fn legacy_user_turn_costs<'a>(rows: &[&'a Obj]) -> Result<HashMap<Option<&'a str>, Obj>, String> {
    let mut projected = HashMap::new();
    let (mut boundary, mut cost, mut estimated) = (false, 0.0, false);
    for row in rows {
        let role = str_of(row, "role");
        let meta = meta_of(row);
        let field = |key: &str| meta.and_then(|m| m.get(key));
        if role == Some("user") && field("autoResume") != Some(&Js::Bool(true)) {
            (boundary, cost, estimated) = (true, 0.0, false);
            continue;
        }
        if role != Some("assistant") || !boundary {
            continue;
        }
        let Some(Js::Num(usd)) =
            field("turnCostUsd").filter(|v| matches!(v, Js::Num(n) if n.is_finite() && *n > 0.0))
        else {
            continue;
        };
        cost += usd;
        estimated |= field("turnCostIsEstimate") == Some(&Js::Bool(true));
        let stored_usd = matches!(field("userTurnCostUsd"), Some(Js::Num(n)) if *n > 0.0);
        if money::normalize(field("userTurnCost")).is_none() && !stored_usd {
            let patch = Obj::from([
                ("userTurnMoney".into(), Js::Obj(money::legacy_usd(cost)?)),
                ("userTurnCostUsd".into(), Js::Num(cost)),
                ("userTurnCostIsEstimate".into(), Js::Bool(estimated)),
            ]);
            projected.insert(str_of(row, "clientId"), patch);
        }
    }
    Ok(projected)
}

/// `mapServerMessages`'s branches, in Cindy's order.
fn map_row(row: &Obj) -> Result<Out, String> {
    let meta = meta_of(row);
    if let Some(out) = cards::leading(row, meta)? {
        return Ok(out);
    }
    let role = str_of(row, "role").unwrap_or("");
    let content = &row["content"];
    if matches!(content, Js::Obj(_) | Js::Arr(_)) {
        match role {
            "tool_use" => return tool::tool_use(row, meta),
            "ask_user" => return tool::ask_user(row, content),
            "thinking" => return Ok(tool::thinking(row, content, meta)),
            "plan_review" => return Ok(tool::plan_review(row, content)),
            _ => {}
        }
    }
    if role == "error" {
        return Ok(tool::error(row));
    }
    if let Some(out) = cards::middle(row, meta) {
        return Ok(out);
    }
    if role == "user" {
        return Ok(user::user(row, meta));
    }
    match cards::trailing(row, meta) {
        Some(out) => Ok(out),
        None => assistant::other(row, meta),
    }
}

/// `mapServerCreatedAt`: a thinking block dates from its start, the rest from the row.
fn created_at(out: &Out, raw: Option<&Js>) -> Result<Option<String>, String> {
    let thinking = out.get("role") == Some(&Js::Str("thinking".into()));
    let persisted = match out.get("thinkingFinishedAtMs") {
        Some(Js::Num(at)) if thinking => *at,
        _ if raw.is_none() => f64::NAN,
        _ => time(raw),
    };
    if !persisted.is_finite() {
        return Ok(None);
    }
    let duration = match out.get("thinkingDurationMs") {
        Some(Js::Num(d)) if thinking && d.is_finite() => d.max(0.0),
        _ => 0.0,
    };
    iso(persisted - duration).map(Some)
}

/// `mapServerMessages`: hidden thinking left out, the timeline ordered, each row mapped.
pub fn map_server_messages(rows: &[Js]) -> Result<Vec<Out>, String> {
    let rows: Vec<&Obj> = rows.iter().filter_map(Js::obj).collect();
    let mut by_client: HashMap<Option<&str>, &Obj> = HashMap::new();
    for row in &rows {
        by_client.insert(str_of(row, "clientId"), row);
    }
    let mut ordered: Vec<&Obj> = rows
        .iter()
        .copied()
        .filter(|r| !hidden_thinking(r))
        .collect();
    crate::js_sort::sort(&mut ordered, |a, b| timeline(a, b).is_lt());
    let legacy = legacy_user_turn_costs(&ordered)?;
    let mut out = Vec::with_capacity(ordered.len());
    for row in ordered {
        let mut cm = map_row(row)?;
        let client = str_of(row, "clientId");
        let source = by_client.get(&client).copied();
        let field = |key: &str| source.and_then(|s| s.get(key));
        let flag =
            |key: &str| source.and_then(meta_of).and_then(|m| m.get(key)) == Some(&Js::Bool(true));
        let iso = created_at(&cm, field("createdAt"))?;
        let patch = legacy.get(&client);
        let flags = ["remoteContentTruncated", "remoteRowsTrimmed"];
        let untouched = iso.is_none()
            && field("rowid").is_none()
            && !flags.iter().any(|f| flag(f))
            && patch.is_none()
            && !js::truthy(field("id"));
        if !untouched {
            if let Some(patch) = patch {
                cm.extend(patch.clone());
            }
            if let Some(iso) = iso {
                cm.insert("createdAt".into(), Js::Str(iso));
            }
            if let Some(id @ Js::Str(s)) = field("id")
                && !s.is_empty()
            {
                cm.insert("id".into(), id.clone());
            }
            for key in ["rowid", "cacheReceipt"] {
                if let Some(value) = field(key) {
                    cm.insert(key.into(), value.clone());
                }
            }
            for key in flags.into_iter().filter(|f| flag(f)) {
                cm.insert(key.into(), Js::Bool(true));
            }
        }
        out.push(cm);
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
