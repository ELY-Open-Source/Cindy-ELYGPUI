//! `messageWorkGroups.ts`: items in work groups; `messageTs` is `render_items::created_ms`.
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the rows of 9.5 to 9.14 read the scans and place the fork marker"
    )
)]

use std::rc::Rc;

use indexmap::IndexSet;

use crate::{
    chat_message::merge::{Fields, Message},
    js_date,
    message_rows::{is_continuation_message, is_delivery_prose_text},
    render_items::{
        Item, client_id, content, created_ms, field, is_completed_assistant, prop, role, text,
    },
    rich_answer::js::{Js, truthy},
    theme_import::color::js_trim,
    work_runs::{self, Adapter},
};

/// `forkOrigin`: where a forked task left its parent.
pub struct ForkOrigin<'a> {
    pub parent_session_id: &'a str,
    pub forked_at_message_id: &'a str,
    pub forked_session_created_at: &'a str,
}

fn steer(row: &Fields) -> bool {
    let meta = field(row, "agentMeta").and_then(|meta| prop(meta, "delivery"));
    text(row, "delivery") == Some("steer") || meta.and_then(Js::str) == Some("steer")
}

fn card(row: &Fields) -> bool {
    truthy(field(row, "systemCardType"))
}

fn failed(row: &Fields) -> bool {
    truthy(field(row, "failedSamplingAttempt"))
}

/// A user row that opens a turn, as the stream's row scans read it.
fn opens_turn(row: &Fields) -> bool {
    role(row) == "user" && text(row, "delivery") != Some("steer")
}

fn spoken(row: &Fields) -> bool {
    role(row) == "assistant" && !card(row) && !js_trim(content(row)).is_empty()
}

/// `assistantHasFollowingUserBoundary`.
pub fn assistant_has_following_user_boundary(rows: &[&Fields], assistant: &str) -> bool {
    let at = rows
        .iter()
        .position(|row| text(row, "clientId") == Some(assistant));
    at.is_some_and(|at| rows[at + 1..].iter().any(|row| opens_turn(row)))
}

/// `collectTurnFinalAssistantClientIds`; `None` is a sealed answer with no `clientId`.
pub fn turn_final_assistant_client_ids(rows: &[&Fields]) -> IndexSet<Option<String>> {
    let mut out = IndexSet::new();
    let mut sealed = false;
    let mut fallback: Option<&str> = None;
    let unsealed = |sealed: bool, id: &&str| !sealed && !id.is_empty();
    for row in rows.iter().rev() {
        if opens_turn(row) {
            let last = fallback.take().filter(|id| unsealed(sealed, id));
            out.extend(last.map(|id| Some(id.to_owned())));
            sealed = false;
        } else if spoken(row) && is_completed_assistant(row) {
            out.insert(text(row, "clientId").map(str::to_owned));
            sealed = true;
        } else if spoken(row) && fallback.is_none() {
            fallback = text(row, "clientId");
        }
    }
    let last = fallback.filter(|id| unsealed(sealed, id));
    out.extend(last.map(|id| Some(id.to_owned())));
    out
}

fn typed(row: &&&Fields) -> bool {
    role(row) == "user" && !truthy(field(row, "isSyntheticTrigger"))
}

/// `findFirstUserMessageClientId`: unknown while older pages wait.
pub fn first_user_message_client_id<'a>(rows: &[&'a Fields], has_older: bool) -> Option<&'a Js> {
    let first = rows.iter().find(typed).filter(|_| !has_older)?;
    field(first, "clientId")
}

/// `findLastUserMessageClientId`.
pub fn last_user_message_client_id<'a>(rows: &[&'a Fields]) -> Option<&'a Js> {
    field(rows.iter().rfind(typed)?, "clientId")
}

/// The rows behind the items, for `groupWorkRuns`' adapter.
struct Rows<'a>(&'a [Message]);

fn latest(all: impl Iterator<Item = Option<f64>>) -> Option<f64> {
    all.flatten().reduce(f64::max)
}

impl<'a> Rows<'a> {
    fn message(&self, item: &Item) -> Option<&'a Fields> {
        match item {
            Item::Message { message, .. } => Some(&self.0[*message]),
            _ => None,
        }
    }

    fn says(&self, item: &Item, role_is: &str) -> Option<&'a Fields> {
        self.message(item).filter(|row| role(row) == role_is)
    }

    /// `renderItemStartMs`.
    fn start_ms(&self, item: &Item) -> Option<f64> {
        match item {
            Item::Message { message, .. } => created_ms(&self.0[*message]),
            Item::ToolSegment { tool_calls, .. } => created_ms(&self.0[*tool_calls.first()?]),
            Item::AgentTask { tool_call, .. } => created_ms(&self.0[*tool_call]),
            Item::AgentPlan { created_at, .. } => {
                let ms = js_date::parse(created_at.as_deref().unwrap_or_default());
                ms.is_finite().then_some(ms)
            }
            Item::WorkGroup { children, .. } => {
                children.iter().find_map(|child| self.start_ms(child))
            }
            _ => None,
        }
    }

    /// `renderItemEndMs`.
    fn end_ms(&self, item: &Item) -> Option<f64> {
        let start = self.start_ms(item);
        match item {
            Item::ToolSegment {
                tool_calls,
                result_ts_map,
                ..
            } => {
                let calls = tool_calls.iter().map(|call| &self.0[*call]);
                let ends = calls.flat_map(|call| {
                    let result = result_ts_map.get(&client_id(call)).copied();
                    [created_ms(call), result]
                });
                latest(ends)
            }
            Item::AgentTask { result_ts_ms, .. } => latest([start, *result_ts_ms].into_iter()),
            Item::WorkGroup { children, .. } => {
                latest(children.iter().map(|child| self.end_ms(child)))
            }
            Item::Message { .. } if self.says(item, "thinking").is_some() => {
                let took = self
                    .message(item)
                    .and_then(|row| field(row, "thinkingDurationMs"));
                let took = match took {
                    Some(Js::Num(ms)) if ms.is_finite() => ms.max(0.0),
                    _ => 0.0,
                };
                start.map(|start| start + took)
            }
            _ => start,
        }
    }

    /// `isRunningAgentTask` with no live update: no terminal status and no result.
    fn running_task(&self, item: &Item) -> bool {
        let Item::AgentTask {
            persisted_status,
            result,
            ..
        } = item
        else {
            return false;
        };
        let ended = matches!(
            persisted_status.as_deref(),
            Some("completed" | "failed" | "stopped")
        );
        !ended && result.as_deref().is_none_or(|r| js_trim(r).is_empty())
    }

    /// `isWorkflowTaskItem`.
    fn workflow(&self, item: &Item) -> bool {
        matches!(item, Item::AgentTask { tool_call, .. }
            if text(&self.0[*tool_call], "toolName") == Some("Workflow"))
    }

    /// `isDeliveryProseItem`.
    fn delivery_prose(&self, item: &Item) -> bool {
        let attached = |row: &Fields, key: &str| {
            field(row, key)
                .and_then(Js::arr)
                .is_some_and(|all| !all.is_empty())
        };
        self.says(item, "assistant").is_some_and(|row| {
            !failed(row)
                && !card(row)
                && (attached(row, "images")
                    || attached(row, "files")
                    || is_delivery_prose_text(content(row)))
        })
    }

    /// `isWorkChild`.
    fn work_child(&self, item: &Item) -> bool {
        matches!(item, Item::ToolSegment { .. } | Item::AgentTask { .. })
            || self.says(item, "thinking").is_some()
            || self.says(item, "assistant").is_some_and(|row| !card(row))
    }

    /// `workChildClientId`.
    fn child_client_id(&self, item: &Item) -> String {
        match item {
            Item::ToolSegment { tool_calls, .. } => client_id(&self.0[tool_calls[0]]),
            Item::AgentTask { tool_call, .. } => client_id(&self.0[*tool_call]),
            Item::Message { message, .. } => client_id(&self.0[*message]),
            other => panic!("not a work child: {other:?}"),
        }
    }

    /// The run's first real activity, else its first item.
    fn anchor<'r>(&self, run: &'r [Rc<Item>]) -> &'r Item {
        let acts = |item: &&Rc<Item>| {
            self.message(item).is_none() || self.says(item, "thinking").is_some()
        };
        run.iter().find(acts).unwrap_or(&run[0])
    }

    /// `createWorkGroup` under `key`.
    fn work_group(
        &self,
        key: &str,
        run: &[Rc<Item>],
        next: Option<&Rc<Item>>,
        boundary: Option<f64>,
    ) -> (String, Option<f64>, Option<f64>) {
        let anchor = self.anchor(run);
        let anchored = self.start_ms(anchor);
        let start = boundary
            .filter(|boundary| anchored.is_none_or(|anchored| *boundary <= anchored))
            .or(anchored);
        let end = match next.and_then(|next| self.message(next)) {
            Some(row) => created_ms(row),
            None => latest(run.iter().map(|item| self.end_ms(item))),
        };
        let duration = start.zip(end).filter(|(start, end)| end >= start);
        (
            format!("{key}-{}", self.child_client_id(anchor)),
            duration.map(|(start, end)| end - start),
            start,
        )
    }
}

impl Adapter for Rows<'_> {
    type Item = Rc<Item>;

    fn is_user_boundary(&self, item: &Rc<Item>) -> bool {
        self.says(item, "user").is_some_and(|row| !steer(row))
    }

    fn is_continuation_boundary(&self, item: &Rc<Item>) -> bool {
        self.says(item, "user").is_some_and(|row| {
            field(row, "isContinuationTrigger") == Some(&Js::Bool(true))
                || text(row, "systemCardType") == Some("auto-resume")
                || is_continuation_message(
                    "user",
                    field(row, "content").unwrap_or(&Js::Null),
                    field(row, "agentMeta"),
                )
        })
    }

    fn is_answer(&self, item: &Rc<Item>) -> bool {
        self.message(item)
            .is_some_and(|row| spoken(row) && !failed(row))
    }

    fn can_prepend_to_answer(&self, item: &Rc<Item>) -> bool {
        self.message(item)
            .is_none_or(|row| text(row, "assistantPhase") != Some("commentary"))
    }

    fn is_sealed_answer(&self, item: &Rc<Item>) -> bool {
        self.message(item).is_some_and(is_completed_assistant)
    }

    fn is_compact_boundary(&self, item: &Rc<Item>) -> bool {
        self.says(item, "assistant")
            .is_some_and(|row| text(row, "systemCardType") == Some("compact"))
    }

    fn is_activity(&self, item: &Rc<Item>) -> bool {
        let failed_answer =
            |row: &Fields| field(row, "failedSamplingAttempt") == Some(&Js::Bool(true));
        !self.running_task(item)
            && !self.workflow(item)
            && (matches!(**item, Item::ToolSegment { .. } | Item::AgentTask { .. })
                || self.says(item, "thinking").is_some()
                || self.says(item, "assistant").is_some_and(failed_answer))
    }

    fn is_archivable(&self, item: &Rc<Item>) -> bool {
        !self.running_task(item)
            && !self.workflow(item)
            && !self.delivery_prose(item)
            && self.work_child(item)
    }

    fn start_timestamp(&self, item: &Rc<Item>) -> Option<f64> {
        self.start_ms(item)
    }

    fn end_timestamp(&self, item: &Rc<Item>) -> Option<f64> {
        self.end_ms(item)
    }

    fn boundary_timestamp(&self, item: Option<&Rc<Item>>) -> Option<f64> {
        created_ms(self.message(item?)?)
    }

    fn user_boundary_end(&self, item: &Rc<Item>, previous: Option<f64>) -> Option<f64> {
        self.end_ms(item).or(previous)
    }

    fn create_group(
        &self,
        run: Vec<Rc<Item>>,
        next: Option<&Rc<Item>>,
        streaming: bool,
        boundary: Option<f64>,
    ) -> Rc<Item> {
        let (key, duration_ms, started_at_ms) = self.work_group("work", &run, next, boundary);
        Rc::new(Item::WorkGroup {
            key,
            children: run,
            duration_ms,
            is_streaming: streaming,
            started_at_ms,
            deferred: None,
        })
    }

    /// `createCompletedWorkGroup`: the answers' text with each run of work between them.
    fn create_completed_group(
        &self,
        run: Vec<Rc<Item>>,
        next: Option<&Rc<Item>>,
        boundary: Option<f64>,
    ) -> Rc<Item> {
        let answers =
            |item: &Rc<Item>| self.says(item, "assistant").is_some_and(|row| !failed(row));
        if !run.iter().any(answers) {
            return self.create_group(run, next, false, boundary);
        }
        let (key, duration_ms, started_at_ms) =
            self.work_group("work-summary", &run, next, boundary);
        let mut children = Vec::new();
        let mut acts: Vec<Rc<Item>> = Vec::new();
        let mut inner = boundary;
        for item in run {
            if self.is_activity(&item) {
                acts.push(item);
                continue;
            }
            if !acts.is_empty() {
                let acts = std::mem::take(&mut acts);
                children.push(self.create_group(acts, Some(&item), false, inner));
            }
            inner = self.boundary_timestamp(Some(&item));
            children.push(item);
        }
        if !acts.is_empty() {
            children.push(self.create_group(acts, next, false, inner));
        }
        Rc::new(Item::WorkGroup {
            key,
            children,
            duration_ms,
            is_streaming: false,
            started_at_ms,
            deferred: None,
        })
    }
}

/// `groupWorkRuns` over the items `render_items::build` made from `rows`.
pub fn group(items: Vec<Item>, rows: &[Message], streaming: bool) -> Vec<Rc<Item>> {
    let items = items.into_iter().map(Rc::new).collect();
    work_runs::group(items, streaming, &Rows(rows))
}

/// `renderItemContainsClientId`.
pub fn contains_client_id(item: &Item, rows: &[Message], id: &str) -> bool {
    let is = |row: &usize| text(&rows[*row], "clientId") == Some(id);
    match item {
        Item::ForkOrigin { .. } => false,
        Item::Message { message, .. } => is(message),
        Item::ToolSegment { tool_calls, .. } => tool_calls.iter().any(is),
        Item::AgentTask { tool_call, .. } => is(tool_call),
        Item::AgentPlan {
            source_client_ids, ..
        } => source_client_ids.iter().any(|source| source == id),
        Item::WorkGroup {
            children, deferred, ..
        } => {
            let stands_for = |key: &str| {
                key.strip_prefix("work-summary-") == Some(id)
                    || key.strip_prefix("work-") == Some(id)
            };
            deferred
                .as_ref()
                .is_some_and(|d| d.key.split('|').any(stands_for))
                || children
                    .iter()
                    .any(|child| contains_client_id(child, rows, id))
        }
        Item::ToolMedia { key, .. } | Item::GeneratedFiles { key, .. } => {
            key.ends_with(&format!("-{id}"))
        }
    }
}

/// `insertForkOriginItem`: the marker before the first item the fork made.
pub fn insert_fork_origin(
    mut items: Vec<Rc<Item>>,
    rows: &[Message],
    fork: Option<&ForkOrigin>,
) -> Vec<Rc<Item>> {
    let Some(fork) = fork else {
        return items;
    };
    let forked = js_date::parse(fork.forked_session_created_at);
    let mut times = items.iter().map(|item| Rows(rows).start_ms(item));
    let mut before = false;
    let at = times.position(|ms| {
        before |= ms.is_some_and(|ms| ms < forked);
        ms.is_some_and(|ms| ms >= forked)
    });
    let Some(at) = at.filter(|_| before) else {
        return items;
    };
    let marker = Item::ForkOrigin {
        key: format!(
            "fork-origin-{}-{}",
            fork.parent_session_id, fork.forked_at_message_id
        ),
        parent_session_id: fork.parent_session_id.to_owned(),
        forked_at_message_id: fork.forked_at_message_id.to_owned(),
    };
    items.insert(at, Rc::new(marker));
    items
}

#[cfg(test)]
mod tests;
