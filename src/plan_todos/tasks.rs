//! `Task*` plan tools: the task list they build up call by call.

use indexmap::IndexMap;

use super::{
    Status, Todo, parse::lines, record, source_client_id, tool_input, tool_name, tool_use_id,
};
use crate::{
    rich_answer::js::{self, Js, Obj},
    theme_import::color::{js_space, js_trim},
};

/// Task id to its row, in the order the tasks appeared.
pub type State = IndexMap<String, Todo>;

/// `normalizeTaskStatus`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TaskStatus {
    Live(Status),
    Deleted,
}

impl TaskStatus {
    pub fn done(self) -> bool {
        matches!(self, Self::Deleted | Self::Live(Status::Completed))
    }
}

pub fn status(value: Option<&Js>) -> Option<TaskStatus> {
    match value?.str()? {
        "pending" => Some(TaskStatus::Live(Status::Pending)),
        "completed" => Some(TaskStatus::Live(Status::Completed)),
        "in_progress" | "running" | "inProgress" => Some(TaskStatus::Live(Status::InProgress)),
        "deleted" => Some(TaskStatus::Deleted),
        _ => None,
    }
}

/// `firstString`: the first key holding text, trimmed.
fn first_string<'a>(fields: Option<&'a Obj>, keys: &[&str]) -> Option<&'a str> {
    let fields = fields?;
    keys.iter()
        .filter_map(|key| fields.get(*key)?.str())
        .map(js_trim)
        .find(|text| !text.is_empty())
}

/// `taskContent`.
pub fn task_content(fields: Option<&Obj>) -> Option<&str> {
    const KEYS: [&str; 7] = [
        "subject",
        "content",
        "description",
        "activeForm",
        "active_form",
        "title",
        "text",
    ];
    first_string(fields, &KEYS)
}

/// `taskId`.
pub fn task_id(fields: Option<&Obj>) -> Option<&str> {
    first_string(fields, &["taskId", "task_id", "id"])
}

/// `tryParseJsonRecord`.
fn parse_record(text: Option<&str>) -> Option<Obj> {
    match &mut js::parse(text.filter(|t| !t.is_empty())?).ok()? {
        Js::Obj(fields) => Some(std::mem::take(fields)),
        _ => None,
    }
}

fn tasks_of(parsed: &Obj) -> Option<&[Js]> {
    parsed.get("tasks")?.arr()
}

/// `taskListResultClearsPlan`: a task list whose every task is deleted.
pub fn list_clears(result: Option<&str>) -> bool {
    let Some(parsed) = parse_record(result) else {
        return false;
    };
    tasks_of(&parsed).is_some_and(|tasks| {
        tasks.iter().all(|task| {
            let deleted = |t: &Obj| status(t.get("status")) == Some(TaskStatus::Deleted);
            task.obj().is_some_and(deleted)
        })
    })
}

/// `taskListResultIsAuthoritative`.
pub fn list_is_authoritative(result: Option<&str>) -> bool {
    parse_record(result).is_some_and(|parsed| tasks_of(&parsed).is_some())
}

/// `taskRecordsFromResult`: the tasks a result names, as JSON or as the CLI prints them.
pub fn records(result: Option<&str>) -> Vec<Obj> {
    let Some(parsed) = parse_record(result) else {
        return plain_records(result);
    };
    if let Some(tasks) = tasks_of(&parsed) {
        return tasks.iter().filter_map(Js::obj).cloned().collect();
    }
    if let Some(task) = parsed.get("task").and_then(Js::obj) {
        return vec![task.clone()];
    }
    if task_id(Some(&parsed)).is_some() || task_content(Some(&parsed)).is_some() {
        return vec![parsed];
    }
    plain_records(result)
}

/// `taskRecordsFromPlainResult`.
fn plain_records(result: Option<&str>) -> Vec<Obj> {
    let text = result.unwrap_or("");
    if js_trim(text).is_empty() {
        return Vec::new();
    }
    lines(text)
        .map(js_trim)
        .filter(|line| !line.is_empty())
        .filter_map(|line| created_line(line).or_else(|| snapshot_line(line)))
        .collect()
}

/// `isWhitespaceCode`.
fn plain_space(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ')
}

fn task(id: &str, status: &str, subject: &str) -> Option<Obj> {
    let pair = |key: &str, value: &str| (key.to_owned(), Js::Str(value.to_owned()));
    (!subject.is_empty()).then(|| {
        Obj::from_iter([
            pair("id", id),
            pair("status", status),
            pair("subject", subject),
        ])
    })
}

/// The id before the first plain space, and what follows it.
fn split_id(text: &str) -> Option<(&str, &str)> {
    let end = text.find(plain_space).filter(|end| *end > 0)?;
    Some((&text[..end], text[end..].trim_start_matches(js_space)))
}

/// `parsePlainTaskCreatedLine`: `Task #3 created successfully: subject`.
fn created_line(line: &str) -> Option<Obj> {
    // `toLowerCase` also folds the Kelvin sign to `k`.
    let mut chars = line.char_indices();
    let folded = |c: char| {
        if c == '\u{212a}' {
            'k'
        } else {
            c.to_ascii_lowercase()
        }
    };
    for expected in "task".chars() {
        if folded(chars.next()?.1) != expected {
            return None;
        }
    }
    let rest = chars.as_str();
    if !rest.starts_with(plain_space) {
        return None;
    }
    let after_hash = rest.trim_start_matches(js_space).strip_prefix('#')?;
    let (id, rest) = split_id(after_hash)?;
    const MARKER: &str = "created successfully:";
    let head = rest.get(..MARKER.len())?;
    if !head.eq_ignore_ascii_case(MARKER) {
        return None;
    }
    task(id, "pending", js_trim(&rest[MARKER.len()..]))
}

/// `parsePlainTaskSnapshotLine`: `#3 [completed] subject [meta]`.
fn snapshot_line(line: &str) -> Option<Obj> {
    let (id, rest) = split_id(line.strip_prefix('#')?)?;
    if !rest.starts_with('[') {
        return None;
    }
    let status_end = rest.find(']').filter(|end| *end > 1)?;
    let status = js_trim(&rest[1..status_end]);
    let mut subject = js_trim(&rest[status_end + 1..]);
    if let Some(meta) = subject
        .rfind(" [")
        .filter(|at| *at > 0 && subject.ends_with(']'))
    {
        subject = js_trim(&subject[..meta]);
    }
    task(id, status, subject)
}

/// `taskToolStatus`.
pub fn tool_status(message: &Js, result: Option<&str>) -> Option<TaskStatus> {
    let records = records(result);
    let result_task = records.first();
    if tool_name(message) == "TaskGet"
        && let Some(task) = result_task
    {
        return status(task.get("status"));
    }
    let own = record(tool_input(message)).and_then(|input| super::present(input.get("status")));
    status(own.or_else(|| result_task?.get("status")))
}

/// `taskToolTargetsExistingTask`.
pub fn targets_existing(message: &Js, result: Option<&str>, state: &State) -> bool {
    let known = |fields: Option<&Obj>| task_id(fields).is_some_and(|id| state.contains_key(id));
    let records = records(result);
    match tool_name(message) {
        "TaskList" => records.iter().any(|task| known(Some(task))),
        "TaskUpdate" | "TaskGet" => {
            let input = record(tool_input(message));
            let id = task_id(input).or_else(|| task_id(records.first()));
            id.is_some_and(|id| state.contains_key(id))
        }
        _ => false,
    }
}

/// `currentTaskTodos`.
pub fn current(state: &State) -> Option<Vec<Todo>> {
    let todos: Vec<Todo> = state
        .values()
        .filter(|todo| !js_trim(&todo.content).is_empty())
        .cloned()
        .collect();
    (!todos.is_empty()).then_some(todos)
}

fn todo(content: &str, status: Status) -> Todo {
    Todo {
        content: content.to_owned(),
        status,
        active_form: None,
    }
}

/// `applyTaskPlanTool`: the list after this call, or none when the call cannot be placed.
pub fn apply(state: &mut State, message: &Js, result: Option<&str>) -> Option<Vec<Todo>> {
    let name = tool_name(message);
    if !matches!(name, "TaskCreate" | "TaskUpdate" | "TaskList" | "TaskGet") {
        return None;
    }
    let input = record(tool_input(message));
    let records = records(result);
    if name == "TaskList" {
        if records.is_empty() {
            if list_clears(result) {
                state.clear();
                return None;
            }
            return current(state);
        }
        let previous = std::mem::take(state);
        for task in &records {
            let Some(id) = task_id(Some(task)) else {
                continue;
            };
            let status = match status(task.get("status")) {
                Some(TaskStatus::Deleted) => continue,
                Some(TaskStatus::Live(status)) => status,
                None => Status::Pending,
            };
            let kept = || previous.get(id).map(|todo| todo.content.as_str());
            if let Some(content) = task_content(Some(task)).or_else(kept) {
                state.insert(id.to_owned(), todo(content, status));
            }
        }
        return current(state);
    }
    let result_task = records.first();
    if name == "TaskCreate" {
        let fallback = || {
            let call = tool_use_id(message).unwrap_or_else(|| source_client_id(message));
            format!("task-create:{call}")
        };
        let id = task_id(result_task)
            .or_else(|| task_id(input))
            .map_or_else(fallback, str::to_owned);
        let status = match result_task.and_then(|task| status(task.get("status"))) {
            Some(TaskStatus::Live(status)) => status,
            _ => Status::Pending,
        };
        if let Some(content) = task_content(input).or_else(|| task_content(result_task)) {
            state.insert(id, todo(content, status));
        }
        return current(state);
    }
    let Some(id) = task_id(input).or_else(|| task_id(result_task)) else {
        return current(state);
    };
    let existing = state.get(id).cloned();
    let supplied = task_content(input).or_else(|| task_content(result_task));
    // A status-only call needs its task rebuilt from an earlier create or list.
    if existing.is_none() && supplied.is_none() {
        return None;
    }
    let kept_status = || existing.as_ref().map(|todo| TaskStatus::Live(todo.status));
    let (status, content) = match result_task.filter(|_| name == "TaskGet") {
        Some(task) => (
            status(task.get("status")),
            task_content(Some(task)).or(existing.as_ref().map(|todo| todo.content.as_str())),
        ),
        None => {
            let own = input.and_then(|input| super::present(input.get("status")));
            (
                status(own.or_else(|| result_task?.get("status"))),
                supplied.or(existing.as_ref().map(|todo| todo.content.as_str())),
            )
        }
    };
    match status.or_else(kept_status) {
        Some(TaskStatus::Deleted) => {
            state.shift_remove(id);
        }
        live => {
            let status = match live {
                Some(TaskStatus::Live(status)) => status,
                _ => Status::Pending,
            };
            if let Some(content) = content {
                state.insert(id.to_owned(), todo(content, status));
            }
        }
    }
    current(state)
}
