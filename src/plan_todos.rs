//! `messageRender.ts`'s plan timeline: the plan sessions a transcript holds and the latest one.
#![cfg_attr(
    not(test),
    expect(dead_code, reason = "`renderHistoryView` builds in 9.4e")
)]

use std::sync::LazyLock;

use indexmap::IndexMap;
use regex::Regex;

use crate::{
    message_rows::{is_agent_plan_tool_name, is_subagent_parent_tool_use_id},
    rich_answer::js::{Js, Obj},
    theme_import::color::js_trim,
};

mod parse;
mod tasks;
mod window;

use parse::extract_plan_todos;

/// `MessageRenderTodoItem['status']`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Status {
    Pending,
    InProgress,
    Completed,
}

/// `MessageRenderTodoItem`.
#[derive(Clone, Debug, PartialEq)]
pub struct Todo {
    pub content: String,
    pub status: Status,
    pub active_form: Option<String>,
}

/// `MessageRenderTodoSource`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    Todo,
    Codex,
    Task,
}

/// `MessageRenderTodoInsertion`.
#[derive(Clone, Debug, PartialEq)]
pub struct Insertion {
    pub key: String,
    pub todos: Vec<Todo>,
    pub source_client_ids: Vec<String>,
    pub created_at: Option<String>,
    pub updated_at_ms: Option<f64>,
    pub source: Source,
    pub sealed: bool,
    pub sealed_at_ms: Option<f64>,
    pub turn_failed: bool,
}

/// `MessageRenderLatestTodoState`; an index of -1 is none.
#[derive(Debug, PartialEq)]
pub struct LatestState {
    pub insertion: Option<Insertion>,
    pub has_plan_event: bool,
    pub is_resolved: bool,
    pub latest_plan_index: i64,
    pub latest_insertion_index: i64,
}

fn field<'a>(message: &'a Js, key: &str) -> Option<&'a Js> {
    message.obj()?.get(key)
}

/// A value that is neither `undefined` nor `null`.
fn present(value: Option<&Js>) -> Option<&Js> {
    value.filter(|v| **v != Js::Null)
}

/// `readRecord`.
fn record(value: Option<&Js>) -> Option<&Obj> {
    value?.obj()
}

fn content(message: &Js) -> Option<&Obj> {
    record(field(message, "content"))
}

fn text<'a>(fields: Option<&'a Obj>, key: &str) -> Option<&'a str> {
    fields?.get(key)?.str()
}

/// `toolNameOf`.
fn tool_name(message: &Js) -> &str {
    let content = content(message);
    text(message.obj(), "toolName")
        .or_else(|| text(content, "toolName"))
        .or_else(|| text(content, "name"))
        .unwrap_or("")
}

/// `toolInputOf`.
fn tool_input(message: &Js) -> Option<&Js> {
    field(message, "toolInput").or_else(|| content(message)?.get("input"))
}

fn filled(value: Option<&str>) -> Option<&str> {
    value.filter(|text| !text.is_empty())
}

/// `toolUseIdOf`.
fn tool_use_id(message: &Js) -> Option<&str> {
    let content = content(message);
    filled(text(message.obj(), "toolUseId"))
        .or_else(|| filled(text(content, "toolUseId")))
        .or_else(|| filled(text(content, "id")))
}

/// `isToolResultSource`.
fn is_tool_result(message: &Js) -> bool {
    let content = content(message);
    text(message.obj(), "role") == Some("tool_result")
        || ["role", "type", "kind"]
            .iter()
            .any(|key| text(content, key) == Some("tool_result"))
}

/// `toolResultTextOf`.
fn tool_result_text(message: &Js) -> Option<&str> {
    let content = content(message);
    text(message.obj(), "content")
        .or_else(|| text(content, "content"))
        .or_else(|| text(content, "result"))
        .or_else(|| text(content, "text"))
}

/// `sourceClientId`.
fn source_client_id(message: &Js) -> &str {
    let id = |key| filled(text(message.obj(), key));
    id("clientId").or_else(|| id("id")).unwrap_or("unknown")
}

/// `buildToolResultLookup`.
fn result_lookup<'a>(messages: &[&'a Js]) -> IndexMap<&'a str, &'a str> {
    let mut out = IndexMap::new();
    for &message in messages {
        let Some(id) = tool_use_id(message).filter(|_| is_tool_result(message)) else {
            continue;
        };
        if let Some(result) = tool_result_text(message) {
            out.insert(id, result);
        }
    }
    out
}

fn result_of<'a>(lookup: &IndexMap<&str, &'a str>, message: &Js) -> Option<&'a str> {
    lookup.get(tool_use_id(message).unwrap_or("")).copied()
}

/// `agentPlanSource`.
fn plan_source(name: &str) -> Option<Source> {
    match name {
        "TodoWrite" => Some(Source::Todo),
        "update_plan" => Some(Source::Codex),
        "TaskCreate" | "TaskUpdate" | "TaskList" | "TaskGet" => Some(Source::Task),
        _ => None,
    }
}

static LEGACY_UUID: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")
        .expect("valid pattern")
});

/// `hasSubagentParent`.
fn has_subagent_parent(message: &Js) -> bool {
    let meta = record(field(message, "agentMeta"));
    let explicit = present(field(message, "parentToolUseId"))
        .or_else(|| present(meta?.get("parentToolUseId")))
        .or_else(|| present(meta?.get("parent_tool_use_id")));
    if let Some(explicit) = explicit
        .and_then(Js::str)
        .filter(|id| !js_trim(id).is_empty())
    {
        return is_subagent_parent_tool_use_id(explicit)
            || !LEGACY_UUID.is_match(js_trim(explicit));
    }
    let nested = record(record(field(message, "source")).and_then(|s| s.get("agentMeta")));
    [text(meta, "parentUuid"), text(nested, "parentUuid")]
        .into_iter()
        .flatten()
        .any(is_subagent_parent_tool_use_id)
}

/// `isSyntheticUserRow`.
fn is_synthetic_user_row(message: &Js) -> bool {
    let meta = record(field(message, "agentMeta"));
    let meta_field = |key| present(meta.and_then(|m| m.get(key)));
    if present(field(message, "hookSource")).is_some() || meta_field("hookSource").is_some() {
        return false;
    }
    let steer = Js::Str("steer".into());
    field(message, "isSyntheticTrigger") == Some(&Js::Bool(true))
        || present(field(message, "automationOrigin")).is_some()
        || meta_field("autoResume") == Some(&Js::Bool(true))
        || meta_field("origin").is_some()
        || field(message, "delivery") == Some(&steer)
        || meta_field("delivery") == Some(&steer)
        || has_subagent_parent(message)
}

/// `isPlanUserBoundary`: a user row that opens a new turn for plans.
pub fn is_plan_user_boundary(message: &Js) -> bool {
    text(message.obj(), "role") == Some("user") && !is_synthetic_user_row(message)
}

/// `planRowSealOf`: the host's seal, on the row or in its stored content.
fn seal(message: &Js) -> Option<Option<f64>> {
    let on = Js::Bool(true);
    let content = content(message);
    let sealed = match field(message, "terminalPlanSnapshot") {
        Some(top) => *top == on,
        None => content.and_then(|c| c.get("terminalPlanSnapshot")) == Some(&on),
    };
    let number = |value: Option<&Js>| match value {
        Some(Js::Num(n)) => Some(*n),
        _ => None,
    };
    sealed.then(|| {
        number(field(message, "terminalPlanAtMs"))
            .or_else(|| number(content.and_then(|c| c.get("terminalPlanAtMs"))))
    })
}

/// `planRowTurnFailed`.
fn turn_failed(message: &Js) -> bool {
    let off = Js::Bool(false);
    match field(message, "turnCompleted") {
        Some(top) => *top == off,
        None => content(message).and_then(|c| c.get("turnCompleted")) == Some(&off),
    }
}

struct Session {
    todos: Vec<Todo>,
    source_client_ids: Vec<String>,
    first: usize,
    last: usize,
    source: Source,
    /// The latest user boundary before the session's first call.
    user_boundary: Option<usize>,
}

/// `findMessageTodoInsertions`: each plan session's card by its last row's index; `Err` is a throw.
pub fn insertions(
    messages: &[&Js],
    key_prefix: &str,
) -> Result<IndexMap<usize, Insertion>, String> {
    let results = result_lookup(messages);
    let mut sessions: Vec<Session> = Vec::new();
    let mut last_by_source: IndexMap<Source, usize> = IndexMap::new();
    let mut task_state = tasks::State::new();
    let mut last_user: Option<usize> = None;
    for (index, message) in messages.iter().enumerate() {
        if text(message.obj(), "role") == Some("user") {
            if is_plan_user_boundary(message) {
                last_user = Some(index);
            }
            continue;
        }
        let Some(source) = plan_source(tool_name(message)) else {
            continue;
        };
        if has_subagent_parent(message) {
            continue;
        }
        let result = result_of(&results, message);
        let previous = last_by_source.get(&source).map(|at| &sessions[*at]);
        let all_done =
            previous.is_some_and(|p| p.todos.iter().all(|t| t.status == Status::Completed));
        let targets = || tasks::targets_existing(message, result, &task_state);
        let continues_completed = source == Source::Task && all_done && targets();
        let sealed = previous.is_some_and(|p| seal(messages[p.last]).is_some());
        let crosses = previous.is_some_and(|p| last_user > p.user_boundary)
            && !(source == Source::Task && targets());
        let starts_new =
            previous.is_none() || sealed || crosses || (all_done && !continues_completed);
        if source == Source::Task && starts_new {
            task_state.clear();
        }
        let parsed = match extract_plan_todos(tool_name(message), tool_input(message))? {
            Some(todos) => Some(todos),
            None => tasks::apply(&mut task_state, message, result),
        };
        let Some(todos) = parsed else { continue };
        let id = source_client_id(message).to_owned();
        match last_by_source.get(&source).filter(|_| !starts_new) {
            Some(at) => {
                let session = &mut sessions[*at];
                session.todos = todos;
                session.source_client_ids.push(id);
                session.last = index;
            }
            None => {
                last_by_source.insert(source, sessions.len());
                sessions.push(Session {
                    todos,
                    source_client_ids: vec![id],
                    first: index,
                    last: index,
                    source,
                    user_boundary: last_user,
                });
            }
        }
    }
    let mut out = IndexMap::new();
    for session in sessions {
        let last = &messages[session.last];
        let seal = seal(last);
        let updated = match field(last, "planUpdatedAtMs") {
            Some(Js::Num(n)) => Some(*n),
            _ => None,
        };
        let key = format!("{key_prefix}-{}", source_client_id(messages[session.first]));
        let insertion = Insertion {
            key,
            todos: session.todos,
            source_client_ids: session.source_client_ids,
            created_at: text(last.obj(), "createdAt").map(str::to_owned),
            updated_at_ms: updated,
            source: session.source,
            sealed: seal.is_some(),
            sealed_at_ms: seal.flatten(),
            turn_failed: turn_failed(last),
        };
        out.insert(session.last, insertion);
    }
    Ok(out)
}

/// `getLatestMessageTodoState`; `incomplete` is `taskHistoryMayBeIncomplete`.
pub fn latest_state(
    messages: &[&Js],
    key_prefix: &str,
    incomplete: bool,
) -> Result<LatestState, String> {
    let latest_plan = messages.iter().rposition(|message| {
        is_agent_plan_tool_name(tool_name(message)) && !has_subagent_parent(message)
    });
    let (latest_index, latest) = insertions(messages, key_prefix)?
        .into_iter()
        .max_by_key(|(index, _)| *index)
        .unzip();
    let task_window_resolved = latest_plan.is_none_or(|at| {
        plan_source(tool_name(messages[at])) != Some(Source::Task)
            || window::is_resolved(messages, at, incomplete)
    });
    let belongs = latest_index == latest_plan && task_window_resolved;
    let clears = latest_plan.is_some_and(|at| {
        parse::is_explicit_clear(messages[at]) || window::latest_task_event_clears(messages, at)
    });
    let index = |at: Option<usize>| at.map_or(-1, |at| at as i64);
    Ok(LatestState {
        insertion: latest.filter(|_| belongs),
        has_plan_event: latest_plan.is_some(),
        is_resolved: latest_plan.is_none() || belongs || (task_window_resolved && clears),
        latest_plan_index: index(latest_plan),
        latest_insertion_index: index(latest_index),
    })
}

#[cfg(test)]
pub(crate) mod tests;
