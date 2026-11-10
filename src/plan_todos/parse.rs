//! The todos a `TodoWrite` or `update_plan` call carries.

use std::sync::LazyLock;

use regex::Regex;

use super::{Status, Todo, present, record, tool_input, tool_name};
use crate::{
    rich_answer::js::{self, Js, Obj},
    theme_import::color::{JS_SPACE, js_trim},
};

static LINE_BREAK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\r?\n").expect("valid pattern"));
static BULLET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"^{JS_SPACE}*(?:[-*+]|[0-9]+[.)]){JS_SPACE}+")).expect("valid pattern")
});
static CHECKBOX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"^{JS_SPACE}*\[[ xX-]\]{JS_SPACE}+")).expect("valid pattern")
});

/// `split(/\r?\n/)`.
pub(super) fn lines(text: &str) -> impl Iterator<Item = &str> {
    LINE_BREAK.split(text)
}

/// `normalizeTodoStatus`.
fn status(value: Option<&Js>) -> Option<Status> {
    match value?.str()? {
        "pending" => Some(Status::Pending),
        "completed" => Some(Status::Completed),
        "in_progress" | "inProgress" | "running" => Some(Status::InProgress),
        _ => None,
    }
}

/// `typeof v === 'string' ? v : String(v ?? '')`, which throws as V8 does.
fn stringified(value: Option<&Js>) -> Result<String, String> {
    present(value).map_or_else(|| Ok(String::new()), |v| js::try_string(Some(v)))
}

fn active_form(fields: &Obj) -> Option<String> {
    fields.get("activeForm")?.str().map(str::to_owned)
}

/// `extractTodos`: a `TodoWrite` input's list.
fn extract_todos(input: Option<&Js>) -> Result<Option<Vec<Todo>>, String> {
    let list = record(input).and_then(|input| input.get("todos")?.arr());
    let Some(todos) = list else { return Ok(None) };
    let mut out = Vec::new();
    for item in todos.iter().filter_map(Js::obj) {
        out.push(Todo {
            content: stringified(item.get("content"))?,
            status: status(item.get("status")).unwrap_or(Status::Pending),
            active_form: active_form(item),
        });
    }
    Ok((!out.is_empty()).then_some(out))
}

/// `extractStructuredPlanItems`.
fn structured(items: Option<&Js>) -> Result<Option<Vec<Todo>>, String> {
    let Some(items) = items.and_then(Js::arr) else {
        return Ok(None);
    };
    let mut todos = Vec::new();
    for item in items.iter().filter_map(Js::obj) {
        let raw = ["content", "text", "step", "title"]
            .iter()
            .find_map(|key| present(item.get(*key)));
        let content = js_trim(&stringified(raw)?).to_owned();
        if !content.is_empty() {
            todos.push(Todo {
                content,
                status: status(item.get("status")).unwrap_or(Status::Pending),
                active_form: None,
            });
        }
    }
    Ok((!todos.is_empty()).then_some(todos))
}

/// `normalizePlanLine`: a line less its bullet and checkbox.
fn plan_line(line: &str) -> &str {
    let line = &line[BULLET.find(line).map_or(0, |m| m.end())..];
    js_trim(&line[CHECKBOX.find(line).map_or(0, |m| m.end())..])
}

/// `extractPlanTodos`: `TodoWrite`'s list, or `update_plan`'s items, plan or text lines.
pub(super) fn extract_plan_todos(
    name: &str,
    input: Option<&Js>,
) -> Result<Option<Vec<Todo>>, String> {
    if name == "TodoWrite" {
        return extract_todos(input);
    }
    if name != "update_plan" {
        return Ok(None);
    }
    let input = record(input);
    let get = |key| input.and_then(|i| i.get(key));
    let todos = match structured(get("items"))? {
        Some(todos) => Some(todos),
        None => structured(get("plan"))?,
    };
    if todos.is_some() {
        return Ok(todos);
    }
    let text = get("text").and_then(Js::str).unwrap_or("");
    let todos: Vec<Todo> = lines(text)
        .map(plan_line)
        .filter(|line| !line.is_empty())
        .enumerate()
        .map(|(index, line)| Todo {
            content: line.to_owned(),
            status: match index {
                0 => Status::InProgress,
                _ => Status::Pending,
            },
            active_form: None,
        })
        .collect();
    Ok((!todos.is_empty()).then_some(todos))
}

/// `isExplicitPlanClearEvent`.
pub(super) fn is_explicit_clear(message: &Js) -> bool {
    let input = record(tool_input(message));
    let empty_list = |key| {
        input
            .and_then(|i| i.get(key))
            .and_then(Js::arr)
            .is_some_and(<[Js]>::is_empty)
    };
    match tool_name(message) {
        "TodoWrite" => empty_list("todos"),
        "update_plan" => {
            let blank = input
                .and_then(|i| i.get("text"))
                .and_then(Js::str)
                .is_some_and(|text| js_trim(text).is_empty());
            empty_list("items") || empty_list("plan") || blank || input.is_some_and(Obj::is_empty)
        }
        _ => false,
    }
}
