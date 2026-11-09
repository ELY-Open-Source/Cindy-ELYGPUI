//! `projectHistorySourceView`: activity flags per row, then `groupWorkRuns` over them.

use std::{collections::HashSet, sync::LazyLock};

use regex::Regex;

use super::{
    Item, Row, Summary, completes_turn, content, created_ms, field, meta, rows_hash, stored, text,
};
use crate::{
    message_rows::{
        has_visible_history_result, is_agent_plan_tool_name, is_agent_task_tool_name,
        is_continuation_message, is_orca_communication_tool, is_steer_user_row, tool_use,
    },
    rich_answer::js::{Js, truthy},
    theme_import::color::js_trim,
    tool_use::{Kind, describe},
    work_runs::{self, Adapter},
};

const EDITS: [&str; 5] = ["Edit", "Write", "MultiEdit", "edit", "write"];
static SPAWN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|:|__)(?:create_worker|create_workers|send_to_worker|ghost_call)$")
        .expect("valid pattern")
});
static GHOST_CALL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|:|__)ghost_call$").expect("valid pattern"));

/// `isHistoryDetailTool`: a tool whose body folds behind a lazy detail read.
pub fn is_detail_tool(name: &str) -> bool {
    !name.is_empty()
        && !is_agent_task_tool_name(name)
        && !is_agent_plan_tool_name(name)
        && !is_orca_communication_tool(name)
        && !["AskUserQuestion", "ExitPlanMode", "Workflow"].contains(&name)
        && !EDITS.contains(&name)
        && !SPAWN.is_match(name)
}

/// `hasFileDelivery`: an MCP call that names the file it makes.
pub fn has_file_delivery(row: &Row) -> bool {
    if text(row, "role") != "tool_use" {
        return false;
    }
    let tool = parsed(row);
    matches!(
        describe(&tool.tool_name, Some(&tool.input)).kind,
        Kind::Mcp {
            created_path: Some(_),
            ..
        }
    )
}

fn parsed(row: &Row) -> crate::message_rows::ToolUse {
    tool_use(field(row, "toolUseId").and_then(Js::str), content(row))
}

fn result_id(row: &Row) -> Option<&str> {
    field(row, "toolUseId")
        .and_then(Js::str)
        .filter(|t| !t.is_empty())
}

/// `isSteerUserRow` on the row itself, which may carry `delivery` beside its meta.
fn steer(row: &Row) -> bool {
    text(row, "delivery") == "steer" || is_steer_user_row(field(row, "agentMeta"))
}

/// `isContinuationMessage` on the row itself, with its live-only flags.
fn continuation(row: &Row) -> bool {
    text(row, "role") == "user"
        && (field(row, "isContinuationTrigger") == Some(&Js::Bool(true))
            || text(row, "systemCardType") == "auto-resume"
            || is_continuation_message("user", content(row), field(row, "agentMeta")))
}

/// `rowEndMs`: a thinking block ends at its `finishedAt`.
fn row_end_ms(row: &Row) -> f64 {
    let started = created_ms(row);
    if text(row, "role") != "thinking" {
        return started;
    }
    match field(content(row), "finishedAt") {
        Some(Js::Num(n)) if n.is_finite() => *n,
        _ => started,
    }
}

fn finite(ms: f64) -> Option<f64> {
    ms.is_finite().then_some(ms)
}

mod flags;

/// `SourceItem` or `GroupItem`; a source is an index into the projected rows.
#[derive(Clone)]
enum It {
    Source {
        row: usize,
        activity: bool,
        archivable: bool,
    },
    Group(Box<Summary>, Option<Vec<Item>>),
}

struct View {
    rows: Vec<Row>,
}

impl View {
    fn row(&self, item: &It) -> Option<&Row> {
        match item {
            It::Source { row, .. } => Some(&self.rows[*row]),
            It::Group(..) => None,
        }
    }

    fn item(&self, item: It) -> Item {
        match item {
            It::Group(summary, children) => Item::Work {
                key: summary.key.clone(),
                summary: *summary,
                children,
            },
            It::Source { row, .. } => {
                let row = self.rows[row].clone();
                let key = match text(&row, "clientId") {
                    "" => text(&row, "id").to_owned(),
                    id => id.to_owned(),
                };
                Item::Messages {
                    key,
                    messages: vec![row],
                    deferred: None,
                }
            }
        }
    }

    fn rows<'a>(&'a self, run: &'a [It]) -> impl Iterator<Item = &'a Row> + Clone + 'a {
        run.iter().filter_map(|i| self.row(i))
    }

    fn activity(item: &It) -> bool {
        matches!(item, It::Source { activity: true, .. })
    }

    /// `group`: the summary of one contiguous run.
    fn summary(&self, run: &[It], active: bool, boundary: Option<f64>) -> Summary {
        let rows: Vec<&Row> = self.rows(run).collect();
        let (first, last) = (rows[0], rows[rows.len() - 1]);
        let anchor = run
            .iter()
            .find(|i| {
                Self::activity(i)
                    && self
                        .row(i)
                        .is_some_and(|r| text(r, "role") != "tool_result")
            })
            .and_then(|i| self.row(i))
            .unwrap_or(first);
        let anchor_id = match text(anchor, "clientId") {
            "" => text(anchor, "id"),
            id => id,
        };
        let started = boundary.unwrap_or_else(|| created_ms(first));
        // `Math.max` turns NaN on any NaN.
        let ended = rows.iter().fold(0.0, |end: f64, r| {
            let ms = row_end_ms(r);
            if end.is_nan() || ms.is_nan() {
                f64::NAN
            } else {
                end.max(ms)
            }
        });
        let has_artifacts = rows.iter().any(|r| {
            field(r, "historyArtifacts")
                .and_then(Js::arr)
                .is_some_and(|a| !a.is_empty())
        });
        let artifacts = has_artifacts.then(|| {
            rows.iter()
                .flat_map(|r| {
                    field(r, "historyArtifacts")
                        .and_then(Js::arr)
                        .unwrap_or_default()
                        .to_vec()
                })
                .collect()
        });
        Summary {
            key: format!("work-{anchor_id}"),
            anchor_client_id: anchor_id.to_owned(),
            parent_tool_use_id: None,
            model: None,
            first_message_id: text(first, "id").to_owned(),
            last_message_id: text(last, "id").to_owned(),
            stored: stored(rows.iter().map(|r| text(r, "id"))),
            started_at_ms: finite(started).unwrap_or(0.0),
            ended_at_ms: finite(ended).unwrap_or(0.0),
            artifacts,
            message_count: rows.len(),
            tool_count: rows
                .iter()
                .filter(|r| text(r, "role") == "tool_use")
                .count(),
            is_streaming: active,
            revision: format!(
                "{}:{}:{}",
                text(last, "id"),
                rows.len(),
                rows_hash(rows.iter().copied())
            ),
            preview: None,
        }
    }

    /// `nest`: an outer summary keeps no artifacts; its leaves hold them.
    fn nest(mut summary: Summary, children: Vec<Item>) -> It {
        summary.artifacts = None;
        It::Group(Box::new(summary), Some(children))
    }

    fn timestamp(&self, item: &It) -> Option<f64> {
        match item {
            It::Group(summary, _) => finite(summary.started_at_ms),
            It::Source { row, .. } => finite(created_ms(&self.rows[*row])),
        }
    }

    /// `activityGroup`: unpaired results split the run; an active run carries its tail.
    fn activity_group(&self, run: Vec<It>, active: bool, boundary: Option<f64>) -> It {
        let mut summary = self.summary(&run, active, boundary);
        let calls: HashSet<Option<String>> = self
            .rows(&run)
            .filter(|r| text(r, "role") == "tool_use")
            .map(|r| parsed(r).tool_use_id)
            .collect();
        let unpaired = |item: &It| {
            self.row(item).is_some_and(|r| {
                let id = field(r, "toolUseId").and_then(Js::str).map(str::to_owned);
                text(r, "role") == "tool_result" && !calls.contains(&id)
            })
        };
        if run.iter().any(unpaired) {
            let mut children = Vec::new();
            let mut segment = Vec::new();
            for item in run {
                if unpaired(&item) {
                    if !segment.is_empty() {
                        let group =
                            self.activity_group(std::mem::take(&mut segment), active, boundary);
                        children.push(self.item(group));
                    }
                    children.push(self.item(item));
                } else {
                    segment.push(item);
                }
            }
            if !segment.is_empty() {
                children.push(self.item(self.activity_group(segment, active, boundary)));
            }
            return Self::nest(summary, children);
        }
        if active {
            let marks: Vec<usize> = (0..run.len())
                .filter(|&i| {
                    self.row(&run[i])
                        .is_some_and(|r| matches!(text(r, "role"), "thinking" | "tool_use"))
                })
                .collect();
            let from = marks
                .get(marks.len().saturating_sub(5))
                .copied()
                .unwrap_or(0);
            let mut preview = self.summary(&run[from..], true, boundary);
            preview.key = format!("preview-{}", summary.key);
            summary.preview = Some(Box::new(preview));
        }
        It::Group(Box::new(summary), None)
    }
}

impl Adapter for View {
    type Item = It;

    fn is_user_boundary(&self, item: &It) -> bool {
        self.row(item)
            .is_some_and(|r| text(r, "role") == "user" && !steer(r))
    }

    fn is_continuation_boundary(&self, item: &It) -> bool {
        self.row(item).is_some_and(continuation)
    }

    fn is_answer(&self, item: &It) -> bool {
        self.row(item).is_some_and(|r| {
            text(r, "role") == "assistant"
                && content(r).str().is_some_and(|s| !js_trim(s).is_empty())
        })
    }

    fn can_prepend_to_answer(&self, item: &It) -> bool {
        self.row(item).is_none_or(|r| {
            meta(r).and_then(|m| m.get("assistantPhase")) != Some(&Js::Str("commentary".into()))
        })
    }

    fn is_sealed_answer(&self, item: &It) -> bool {
        self.row(item).is_some_and(completes_turn)
    }

    fn is_compact_boundary(&self, item: &It) -> bool {
        self.row(item).is_some_and(|r| text(r, "role") == "system")
    }

    fn is_activity(&self, item: &It) -> bool {
        Self::activity(item)
    }

    fn is_archivable(&self, item: &It) -> bool {
        matches!(
            item,
            It::Source {
                archivable: true,
                ..
            }
        )
    }

    fn start_timestamp(&self, item: &It) -> Option<f64> {
        self.timestamp(item)
    }

    fn end_timestamp(&self, item: &It) -> Option<f64> {
        match item {
            It::Group(summary, _) => finite(summary.ended_at_ms),
            It::Source { row, .. } => finite(row_end_ms(&self.rows[*row])),
        }
    }

    fn boundary_timestamp(&self, item: Option<&It>) -> Option<f64> {
        item.and_then(|i| self.timestamp(i))
    }

    fn user_boundary_end(&self, item: &It, previous: Option<f64>) -> Option<f64> {
        self.timestamp(item).or(previous)
    }

    fn create_group(
        &self,
        run: Vec<It>,
        _: Option<&It>,
        streaming: bool,
        boundary: Option<f64>,
    ) -> It {
        self.activity_group(run, streaming, boundary)
    }

    fn create_completed_group(&self, run: Vec<It>, _: Option<&It>, boundary: Option<f64>) -> It {
        if run.iter().all(Self::activity) {
            return self.activity_group(run, false, boundary);
        }
        let mut summary = self.summary(&run, false, boundary);
        summary.key = format!("work-summary-{}", summary.anchor_client_id);
        let mut children = Vec::new();
        let mut activities = Vec::new();
        let mut previous = boundary;
        for item in run {
            if Self::activity(&item) {
                activities.push(item);
                continue;
            }
            if !activities.is_empty() {
                let group = self.activity_group(std::mem::take(&mut activities), false, previous);
                children.push(self.item(group));
            }
            previous = self.timestamp(&item);
            children.push(self.item(item));
        }
        if !activities.is_empty() {
            children.push(self.item(self.activity_group(activities, false, previous)));
        }
        Self::nest(summary, children)
    }

    fn fold_answer_tail(&self) -> bool {
        false
    }
}

/// `projectHistorySourceView`.
pub fn project(rows: Vec<Row>, streaming: bool, preserve_mixed: bool) -> Vec<Item> {
    let items = flags::flags(&rows, preserve_mixed);
    let view = View { rows };
    work_runs::group(items, streaming, &view)
        .into_iter()
        .map(|i| view.item(i))
        .collect()
}
