//! The plan cards `buildRenderItems` draws, checked against a paged window.

use std::collections::HashMap;

use indexmap::{IndexMap, IndexSet};

use super::{filled, role, text};
use crate::{
    chat_message::merge::Fields,
    message_rows::is_agent_plan_tool_name,
    plan_todos::{self, Insertion, Source},
};

/// The plan cards by row; with older pages unread, a `Task*` card must prove itself whole.
pub(super) fn cards(
    messages: &[&Fields],
    incomplete: bool,
) -> Result<IndexMap<usize, Insertion>, String> {
    let mut plans = plan_todos::insertions(messages, "todo")?;
    if !incomplete {
        return Ok(plans);
    }
    let mut plan_ids: Vec<&str> = Vec::new();
    let mut ids_at: HashMap<usize, usize> = HashMap::new();
    let mut results: HashMap<&str, Vec<usize>> = HashMap::new();
    for (index, row) in messages.iter().enumerate() {
        if let Some(id) = text(row, "toolUseId").filter(|_| role(row) == "tool_result") {
            results.entry(id).or_default().push(index);
        }
        let plan_tool = is_agent_plan_tool_name(text(row, "toolName").unwrap_or_default());
        plan_ids.extend(filled(row, "toolUseId").filter(|_| plan_tool));
        if plans.contains_key(&index) {
            ids_at.insert(index, plan_ids.len());
        }
    }
    let tasks = plans.iter().filter(|(_, plan)| plan.source == Source::Task);
    for index in tasks.map(|(index, _)| *index).collect::<Vec<_>>() {
        let ids: IndexSet<&str> = plan_ids[..ids_at[&index]].iter().copied().collect();
        let answers = ids.iter().filter_map(|id| results.get(id)).flatten();
        let mut late: Vec<usize> = answers.copied().filter(|at| *at > index).collect();
        late.sort_unstable();
        let mut rows = messages[..=index].to_vec();
        rows.extend(late.into_iter().map(|at| messages[at]));
        let state = plan_todos::latest_state(&rows, "todo", true)?;
        if !state.is_resolved || state.latest_insertion_index != index as i64 {
            plans.shift_remove(&index);
        }
    }
    Ok(plans)
}
