//! `MessageStream.tsx`'s `buildRenderItems`: a task's rows as the items the stream draws.

use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

use indexmap::{IndexMap, IndexSet};

use crate::{
    chat_message::merge::{self, Fields, Message},
    generated_files::File,
    history_view::Summary,
    js_date,
    message_rows::{
        is_agent_plan_tool_name, is_agent_task_tool_name, is_orca_communication_tool,
        is_subagent_parent_tool_use_id,
    },
    plan_todos::{Insertion, Todo},
    rich_answer::{
        self,
        js::{self, Js, truthy},
    },
    theme_import::color::js_trim,
    tool_media::{self, Media},
    work_runs::HISTORY_GAP_SPLIT_MS,
};

mod plans;
mod turn_end;

/// `RenderItem`; a row is its index in the rows given.
#[derive(Debug, PartialEq)]
pub enum Item {
    Message {
        key: String,
        message: usize,
    },
    AgentPlan {
        key: String,
        todos: Vec<Todo>,
        source_client_ids: Vec<String>,
        created_at: Option<String>,
    },
    ToolSegment {
        key: String,
        tool_calls: Vec<usize>,
        result_map: IndexMap<String, String>,
        result_ts_map: IndexMap<String, f64>,
        settled_ids: IndexSet<String>,
    },
    AgentTask {
        key: String,
        tool_call: usize,
        persisted_status: Option<String>,
        result: Option<String>,
        result_ts_ms: Option<f64>,
    },
    ToolMedia {
        key: String,
        items: Vec<Media>,
    },
    GeneratedFiles {
        key: String,
        files: Vec<File>,
        turn_start_ms: Option<f64>,
        turn_end_ms: Option<f64>,
        turn_sealed: bool,
    },
    WorkGroup {
        key: String,
        children: Vec<Rc<Item>>,
        duration_ms: Option<f64>,
        is_streaming: bool,
        started_at_ms: Option<f64>,
        deferred: Option<Deferred>,
    },
    #[cfg_attr(not(test), expect(dead_code, reason = "9.12 places the fork marker"))]
    ForkOrigin {
        key: String,
        parent_session_id: String,
        forked_at_message_id: String,
    },
}

impl Item {
    pub fn key(&self) -> &str {
        match self {
            Item::Message { key, .. }
            | Item::AgentPlan { key, .. }
            | Item::ToolSegment { key, .. }
            | Item::AgentTask { key, .. }
            | Item::ToolMedia { key, .. }
            | Item::GeneratedFiles { key, .. }
            | Item::WorkGroup { key, .. }
            | Item::ForkOrigin { key, .. } => key,
        }
    }
}

/// `DeferredHistoryWork`: the ranges a group stands for; its actions take the view.
#[derive(Debug, PartialEq)]
pub struct Deferred {
    pub summaries: Vec<Summary>,
    pub key: String,
    pub expanded: bool,
    pub revision: String,
    pub preview_complete: bool,
    pub live_preview: bool,
    pub awaiting_details: bool,
    pub loading: bool,
    pub failed: bool,
}

/// The options of `buildRenderItems` a local task's history uses.
#[derive(Clone, Copy, Default)]
pub struct Options<'a> {
    pub history_window_incomplete: bool,
    pub working_dir: &'a str,
    pub history_artifacts: &'a [Js],
}

pub(crate) fn field<'a>(row: &'a Fields, key: &str) -> Option<&'a Js> {
    merge::get(row, key)
}

/// `value[key]` of a row's object field.
pub(crate) fn prop<'a>(value: &'a Js, key: &str) -> Option<&'a Js> {
    value.obj()?.get(key)
}

pub(crate) fn text<'a>(row: &'a Fields, key: &str) -> Option<&'a str> {
    field(row, key)?.str()
}

pub(crate) fn role(row: &Fields) -> &str {
    text(row, "role").unwrap_or_default()
}

pub(crate) fn content(row: &Fields) -> &str {
    text(row, "content").unwrap_or_default()
}

/// A string field that is not empty.
fn filled<'a>(row: &'a Fields, key: &str) -> Option<&'a str> {
    text(row, key).filter(|text| !text.is_empty())
}

/// The row's `clientId` as a template string writes it.
pub(crate) fn client_id(row: &Fields) -> String {
    js::string(field(row, "clientId"))
}

/// `Date.parse(value ?? '')`, when it is a time.
pub(crate) fn time(value: Option<&Js>) -> Option<f64> {
    let ms = js_date::parse(value.and_then(Js::str).unwrap_or_default());
    ms.is_finite().then_some(ms)
}

pub(crate) fn created_ms(row: &Fields) -> Option<f64> {
    time(field(row, "createdAt"))
}

/// `isSubagentInternalMessage`.
fn is_subagent_internal(row: &Fields) -> bool {
    filled(row, "parentToolUseId").is_some_and(is_subagent_parent_tool_use_id)
}

/// `isRenderTurnBoundary`.
fn is_turn_boundary(row: &Fields) -> bool {
    match role(row) {
        "user" => {
            text(row, "delivery") != Some("steer") && !truthy(field(row, "isSyntheticTrigger"))
        }
        "assistant" => matches!(
            text(row, "systemCardType"),
            Some("cindy-make" | "cindy-make-doctor" | "review")
        ),
        _ => false,
    }
}

/// `isCompletedAssistantMessage`.
pub fn is_completed_assistant(row: &Fields) -> bool {
    let positive = |value: Option<&Js>| matches!(value, Some(Js::Num(n)) if *n > 0.0);
    let money = field(row, "turnMoney").and_then(|money| prop(money, "amount"));
    !truthy(field(row, "failedSamplingAttempt"))
        && (field(row, "turnCompleted") == Some(&Js::Bool(true))
            || positive(money)
            || positive(field(row, "turnCostUsd"))
            || field(row, "turnUsageDetails").is_some())
}

/// `isEmptyOrcaCommunicationResult`.
fn is_empty_orca_result(content: &str) -> bool {
    let trimmed = js_trim(content);
    if trimmed.is_empty() {
        return true;
    }
    let parsed = js::parse(trimmed);
    let Ok(Js::Obj(record)) = &parsed else {
        return false;
    };
    let said = ["message", "result", "text", "content", "error", "detail"]
        .iter()
        .any(|key| js::non_empty(record.get(*key)).is_some());
    !said && record.get("ok") == Some(&Js::Bool(true))
}

/// `shouldHideToolResult`.
fn hides_result(tool_name: &str, content: &str) -> bool {
    is_orca_communication_tool(tool_name) && is_empty_orca_result(content)
}

/// A tool segment still taking calls.
#[derive(Default)]
struct Segment {
    calls: Vec<usize>,
    results: IndexMap<String, String>,
    result_ts: IndexMap<String, f64>,
    settled: IndexSet<String>,
    end_ms: Option<f64>,
    media: Vec<Media>,
}

impl Segment {
    /// `notePendingSegmentEnd`.
    fn note_end(&mut self, ms: Option<f64>) {
        if let Some(ms) = ms {
            self.end_ms = Some(self.end_ms.map_or(ms, |end| end.max(ms)));
        }
    }
}

struct Build<'a> {
    /// The rows past the first filter, each with its index in the rows given.
    all: Vec<(usize, &'a Message)>,
    /// The rows a reader sees, as indices into `all`.
    visible: Vec<usize>,
    opts: Options<'a>,
    inline_media: HashMap<usize, Vec<String>>,
    results: HashMap<&'a str, &'a str>,
    result_ts: HashMap<&'a str, f64>,
    segment: Segment,
    turn_start: usize,
    items: Vec<Item>,
}

impl<'a> Build<'a> {
    fn row(&self, at: usize) -> &'a Fields {
        self.all[self.visible[at]].1
    }

    /// The visible row's index in the rows given.
    fn source(&self, at: usize) -> usize {
        self.all[self.visible[at]].0
    }

    fn is_result(&self, at: usize) -> bool {
        at < self.visible.len() && role(self.row(at)) == "tool_result"
    }

    /// `recordTurnInlineMedia`: what a turn's finished answers embed themselves.
    fn record_inline_media(&mut self, lo: usize, hi: usize) {
        let mut urls = Vec::new();
        for row in (lo..hi).map(|at| self.row(at)) {
            let answer = role(row) == "assistant"
                && !truthy(field(row, "systemCardType"))
                && !truthy(field(row, "isStreaming"));
            if answer {
                urls.extend(rich_answer::media_sources(content(row)));
            }
        }
        self.inline_media.insert(lo, urls);
    }

    fn turn_shows(&self, turn: usize, url: &str) -> bool {
        let shown = self.inline_media.get(&turn);
        shown.is_some_and(|urls| urls.iter().any(|u| u == url))
    }

    /// `flushSegment`.
    fn flush_segment(&mut self) {
        let segment = std::mem::take(&mut self.segment);
        let Some(first) = segment.calls.first() else {
            return;
        };
        let first = client_id(self.all[self.visible[*first]].1);
        let calls = segment.calls.iter().map(|at| self.source(*at)).collect();
        self.items.push(Item::ToolSegment {
            key: format!("seg-{first}"),
            tool_calls: calls,
            result_map: segment.results,
            result_ts_map: segment.result_ts,
            settled_ids: segment.settled,
        });
        if !segment.media.is_empty() {
            let mut seen = HashSet::new();
            let mut items = segment.media;
            items.retain(|item| seen.insert(item.url.clone()));
            let key = format!("media-{first}");
            self.items.push(Item::ToolMedia { key, items });
        }
    }

    /// `collectResultMedia`: a result's media the turn does not show inline.
    fn collect_media(&mut self, result: &str) {
        let media = tool_media::extract(result);
        let turn = self.turn_start;
        let fresh = media.into_iter().filter(|m| !self.turn_shows(turn, &m.url));
        let fresh: Vec<Media> = fresh.collect();
        self.segment.media.extend(fresh);
    }

    /// The index past the `tool_result` rows that follow `at`.
    fn past_results(&self, at: usize) -> usize {
        (at + 1..=self.visible.len())
            .find(|j| !self.is_result(*j))
            .unwrap_or(self.visible.len())
    }

    /// An agent task's card with the result that answers it.
    fn agent_task(&mut self, at: usize) -> usize {
        self.flush_segment();
        let row = self.row(at);
        let id = filled(row, "toolUseId");
        let mut result = id.and_then(|id| self.results.get(id).copied());
        let mut result_ts = id.and_then(|id| self.result_ts.get(id).copied());
        let end = self.past_results(at);
        for adjacent in (at + 1..end).map(|j| self.row(j)) {
            result = result.or(Some(content(adjacent)));
            let ts = created_ms(adjacent).filter(|ts| result_ts.is_none_or(|known| *ts > known));
            result_ts = ts.or(result_ts);
        }
        self.items.push(Item::AgentTask {
            key: format!("task-{}", client_id(row)),
            tool_call: self.source(at),
            persisted_status: filled(row, "agentTaskStatus").map(str::to_owned),
            result: result.map(str::to_owned),
            result_ts_ms: result_ts,
        });
        end
    }

    /// A tool call joins the segment, with its result by id or by adjacency.
    fn tool_call(&mut self, at: usize, tool_name: &str) -> usize {
        let row = self.row(at);
        let client = client_id(row);
        let called = created_ms(row);
        let gap = self.segment.end_ms.zip(called);
        if !self.segment.calls.is_empty()
            && gap.is_some_and(|(end, now)| now - end > HISTORY_GAP_SPLIT_MS)
        {
            self.flush_segment();
        }
        self.segment.calls.push(at);
        self.segment.note_end(called);
        let id = text(row, "toolUseId");
        if let Some(main) = id.and_then(|id| self.results.get(id).copied()) {
            self.segment.settled.insert(client.clone());
            let ts = id.and_then(|id| self.result_ts.get(id).copied());
            if let Some(ts) = ts {
                self.segment.result_ts.insert(client.clone(), ts);
                self.segment.note_end(Some(ts));
            }
            if !hides_result(tool_name, main) {
                self.segment.results.insert(client.clone(), main.to_owned());
                self.collect_media(main);
            }
        }
        let end = self.past_results(at);
        for j in at + 1..end {
            let adjacent = self.row(j);
            self.segment.settled.insert(client.clone());
            if let Some(ts) = created_ms(adjacent) {
                self.segment.note_end(Some(ts));
                let known = self.segment.result_ts.get(&client);
                if known.is_none_or(|known| ts > *known) {
                    self.segment.result_ts.insert(client.clone(), ts);
                }
            }
            let result = content(adjacent);
            if !self.segment.results.contains_key(&client) && !hides_result(tool_name, result) {
                self.segment
                    .results
                    .insert(client.clone(), result.to_owned());
                self.collect_media(result);
            }
        }
        end
    }

    /// One row of the linear build; the index of the next row to read.
    fn step(&mut self, at: usize, plans: &IndexMap<usize, Insertion>) -> usize {
        let row = self.row(at);
        if is_turn_boundary(row) {
            self.flush_segment();
            self.flush_turn_end(self.turn_start, at);
            self.turn_start = at;
        }
        let tool_name = text(row, "toolName").unwrap_or_default();
        match role(row) {
            "ask_user" if text(row, "askUserStatus") != Some("answered") => at + 1,
            "tool_use" if matches!(tool_name, "AskUserQuestion" | "ExitPlanMode") => at + 1,
            "tool_use" if is_agent_plan_tool_name(tool_name) => {
                if let Some(plan) = plans.get(&at).filter(|plan| plan.todos.len() >= 2) {
                    self.flush_segment();
                    self.items.push(Item::AgentPlan {
                        key: plan.key.clone(),
                        todos: plan.todos.clone(),
                        source_client_ids: plan.source_client_ids.clone(),
                        created_at: text(row, "createdAt").map(str::to_owned),
                    });
                }
                at + 1
            }
            "tool_use" if is_agent_task_tool_name(tool_name) || tool_name == "Workflow" => {
                self.agent_task(at)
            }
            "tool_use" => self.tool_call(at, tool_name),
            "tool_result" => at + 1,
            "assistant"
                if !truthy(field(row, "systemCardType")) && js_trim(content(row)).is_empty() =>
            {
                at + 1
            }
            _ => {
                self.flush_segment();
                let key = format!("msg-{}", client_id(row));
                let message = self.source(at);
                self.items.push(Item::Message { key, message });
                at + 1
            }
        }
    }
}

/// `buildRenderItems` for a local task's history; `Err` is a throw.
pub fn build(rows: &[Message], opts: Options) -> Result<Vec<Item>, String> {
    let modal = |row: &Fields| {
        let data = field(row, "systemCardData");
        data.and_then(|data| prop(data, "modalOnly")) == Some(&Js::Bool(true))
    };
    let all: Vec<(usize, &Message)> = rows.iter().enumerate().filter(|(_, r)| !modal(r)).collect();
    let visible: Vec<usize> = (0..all.len())
        .filter(|at| !is_subagent_internal(all[*at].1))
        .collect();
    let mut build = Build {
        all,
        visible,
        opts,
        inline_media: HashMap::new(),
        results: HashMap::new(),
        result_ts: HashMap::new(),
        segment: Segment::default(),
        turn_start: 0,
        items: Vec::new(),
    };
    let messages: Vec<&Fields> = (0..build.visible.len()).map(|at| build.row(at)).collect();
    let mut start = 0;
    for (index, row) in messages.iter().enumerate() {
        if is_turn_boundary(row) && index > start {
            build.record_inline_media(start, index);
            start = index;
        }
    }
    build.record_inline_media(start, messages.len());
    for row in messages.iter().filter(|row| role(row) == "tool_result") {
        let Some(id) = filled(row, "toolUseId") else {
            continue;
        };
        if let Some(result) = text(row, "content") {
            build.results.insert(id, result);
        }
        if let Some(ts) = created_ms(row) {
            build.result_ts.insert(id, ts);
        }
    }
    let plans = plans::cards(&messages, opts.history_window_incomplete)?;
    let mut at = 0;
    while at < messages.len() {
        at = build.step(at, &plans);
    }
    build.flush_segment();
    build.flush_turn_end(build.turn_start, messages.len());
    Ok(build.items)
}

#[cfg(test)]
mod tests;
