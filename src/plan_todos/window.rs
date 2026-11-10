//! What a loaded window proves about its latest `Task*` plan event.

use indexmap::IndexMap;

use super::{
    Source, Status, Todo, is_plan_user_boundary, plan_source, record, result_lookup, result_of,
    tasks::{self, TaskStatus},
    tool_input, tool_name,
};
use crate::rich_answer::js::Js;

fn all_done(todos: &[Todo]) -> bool {
    todos.iter().all(|todo| todo.status == Status::Completed)
}

/// `latestTaskEventClearsPlan`: the latest task event leaves no task standing.
pub fn latest_task_event_clears(messages: &[&Js], latest: usize) -> bool {
    let results = result_lookup(messages);
    let last = &messages[latest];
    let last_result = result_of(&results, last);
    match tool_name(last) {
        "TaskList" => return tasks::list_clears(last_result),
        "TaskUpdate" | "TaskGet" => {}
        _ => return false,
    }
    if tasks::tool_status(last, last_result) != Some(TaskStatus::Deleted) {
        return false;
    }
    let mut state = tasks::State::new();
    let mut previous: Option<Vec<Todo>> = None;
    let mut resolved = false;
    for (index, message) in messages.iter().enumerate().take(latest + 1) {
        if plan_source(tool_name(message)) != Some(Source::Task) {
            continue;
        }
        let result = result_of(&results, message);
        let starts_new = previous.as_deref().is_none_or(|todos| {
            all_done(todos) && !tasks::targets_existing(message, result, &state)
        });
        if starts_new {
            state.clear();
        }
        let had_context = !state.is_empty();
        if let Some(todos) = tasks::apply(&mut state, message, result) {
            resolved = true;
            previous = Some(todos);
            continue;
        }
        if index == latest {
            return resolved && had_context && state.is_empty();
        }
    }
    false
}

/// `isTaskPlanWindowResolved`: every task event ties back to a visible create or list.
pub fn is_resolved(messages: &[&Js], latest: usize, has_earlier: bool) -> bool {
    let results = result_lookup(messages);
    let mut state = tasks::State::new();
    let mut unresolved: IndexMap<String, TaskStatus> = IndexMap::new();
    let mut previous: Option<Vec<Todo>> = None;
    let mut saw_task = false;
    let mut boundary_known = !has_earlier;
    let mut last_user: Option<usize> = None;
    let mut session_user: Option<usize> = None;
    for (index, message) in messages.iter().enumerate().take(latest + 1) {
        if is_plan_user_boundary(message) {
            last_user = Some(index);
            continue;
        }
        let name = tool_name(message);
        if plan_source(name) != Some(Source::Task) {
            continue;
        }
        let result = result_of(&results, message);
        let records = tasks::records(result);
        let input = record(tool_input(message));
        let target = tasks::task_id(input).or_else(|| tasks::task_id(records.first()));
        let has_context = previous.is_some() || !unresolved.is_empty();
        let previous_done = has_context
            && previous.as_deref().is_none_or(all_done)
            && unresolved.values().all(|status| status.done());
        let targets = tasks::targets_existing(message, result, &state);
        let continues_completed =
            previous_done && (targets || target.is_some_and(|id| unresolved.contains_key(id)));
        let crosses = saw_task && last_user > session_user && !targets;
        let starts_new = !saw_task || crosses || (previous_done && !continues_completed);
        if starts_new {
            let after_completed = saw_task && previous_done && !continues_completed;
            let create_after_user =
                name == "TaskCreate" && last_user.is_some() && (!saw_task || crosses);
            state.clear();
            unresolved.clear();
            boundary_known = !has_earlier || after_completed || create_after_user;
            session_user = last_user;
        }
        saw_task = true;
        if name == "TaskList" {
            if tasks::list_is_authoritative(result) {
                boundary_known = true;
            }
            if !records.is_empty() {
                unresolved.clear();
                for task in &records {
                    let status = tasks::status(task.get("status"))
                        .unwrap_or(TaskStatus::Live(Status::Pending));
                    let Some(id) = tasks::task_id(Some(task)) else {
                        continue;
                    };
                    if status == TaskStatus::Deleted {
                        continue;
                    }
                    if tasks::task_content(Some(task)).is_none() && !state.contains_key(id) {
                        unresolved.insert(id.to_owned(), status);
                    }
                }
            } else if tasks::list_clears(result) {
                unresolved.clear();
            }
        } else if name != "TaskCreate"
            && let Some(id) = target.filter(|id| !state.contains_key(*id))
        {
            let status = tasks::tool_status(message, result);
            unresolved.insert(
                id.to_owned(),
                status.unwrap_or(TaskStatus::Live(Status::Pending)),
            );
        }
        let parsed = tasks::apply(&mut state, message, result);
        unresolved.retain(|id, _| !state.contains_key(id));
        previous = parsed.or_else(|| tasks::current(&state));
        if previous.is_none() && unresolved.is_empty() && state.is_empty() {
            previous = Some(Vec::new());
        }
    }
    boundary_known && unresolved.is_empty()
}
