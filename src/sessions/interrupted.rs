//! `listInterruptedResumeCandidateRows`: tasks an earlier process left mid-turn.

use std::{path::Path, time::Duration};

use gpui::App;
use rusqlite::{Connection, OptionalExtension as _};

use super::{Sessions, query::TASK_SOURCES};
use crate::{
    localdb::{LocalDb, State},
    rich_answer::js::{self, Js},
};

/// `MAX_LOAD_ATTEMPTS`.
const ATTEMPTS: u64 = 5;

/// `InterruptedTurnToolUse`: the raw name and input of the turn's last tool call.
#[derive(Debug, PartialEq)]
pub struct ToolUse {
    pub name: String,
    pub input: Option<Js>,
}

/// A row of `listInterruptedResumeCandidateRows`.
#[derive(Debug, PartialEq)]
pub struct Pending {
    pub session_id: String,
    /// The turn's last stored message, else its start.
    pub interrupted_at: i64,
    pub tool: Option<ToolUse>,
}

/// What `interrupted-resume-candidates` gives the dialog.
#[derive(Debug, PartialEq)]
pub struct Candidate {
    pub session_id: String,
    pub interrupted_at: i64,
    /// The last tool call's island line, else its name.
    pub activity: Option<String>,
}

impl Candidate {
    fn new(pending: Pending, t: &crate::i18n::Translator) -> Self {
        let activity = pending
            .tool
            .map(|tool| crate::tool_row::activity(t, &tool.name, tool.input.as_ref()));
        Self {
            session_id: pending.session_id,
            interrupted_at: pending.interrupted_at,
            activity,
        }
    }
}

/// The window's load, keyed by owner, as the dialog's `_load` singleton is.
#[derive(Default)]
pub struct Interrupted {
    pub owner: Option<std::path::PathBuf>,
    pub candidates: Option<Vec<Candidate>>,
}

impl gpui::Global for Interrupted {}

/// `parseToolUse`: a JSON object with a nonempty string `toolName`.
fn tool_use(raw: Option<&str>) -> Option<ToolUse> {
    let parsed = js::parse(raw?).ok()?;
    let fields = parsed.obj()?;
    let name = match fields.get("toolName") {
        Some(Js::Str(name)) if !name.is_empty() => name.clone(),
        _ => return None,
    };
    Some(ToolUse {
        name,
        input: fields.get("input").cloned(),
    })
}

/// `botSessionInputBlockReason` passes and `isShadowedCompanionSession` does not.
fn can_resume(bot: &str, role: &str, status: &str, shadowed: &[String]) -> bool {
    let shadow = matches!(role, "canonical" | "routine") && shadowed.iter().any(|id| id == bot);
    role != "history" && status == "active" && !shadow
}

/// `listInterruptedPendingRows` with companion tasks that can resume, each turn's last traces.
pub fn candidates(
    db: &Connection,
    boot_at: i64,
    shadowed: &[String],
) -> rusqlite::Result<Vec<Pending>> {
    let sql = format!(
        "SELECT sessions.id, sessions.source, sessions.active_turn_started_at,
                bot_session_links.bot_id, bot_session_links.role, bot_profiles.status
         FROM sessions
         LEFT JOIN bot_session_links ON bot_session_links.session_id = sessions.id
         LEFT JOIN bot_profiles ON bot_profiles.id = bot_session_links.bot_id
         WHERE sessions.status = 'active'
           AND (sessions.source IN ({TASK_SOURCES}) OR (sessions.source = 'bot'
             AND bot_session_links.role IN ('canonical', 'delegation', 'routine', 'channel')))
           AND sessions.hidden_at IS NULL
           AND sessions.active_turn_started_at > COALESCE(sessions.last_turn_ended_at, 0)
           AND sessions.active_turn_started_at > COALESCE(sessions.cleared_at, 0)
           AND sessions.active_turn_started_at < ?"
    );
    let mut statement = db.prepare(&sql)?;
    type Row = (
        String,
        String,
        i64,
        Option<String>,
        Option<String>,
        Option<String>,
    );
    let rows: Vec<Row> = statement
        .query_map([boot_at], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let mut last = db.prepare(
        "SELECT created_at FROM messages
         WHERE session_id = ?1 AND rewind_at IS NULL AND created_at >= ?2
         ORDER BY created_at DESC, rowid DESC LIMIT 1",
    )?;
    let mut tool = db.prepare(
        "SELECT content FROM messages
         WHERE session_id = ?1 AND rewind_at IS NULL AND created_at >= ?2 AND role = 'tool_use'
         ORDER BY created_at DESC, rowid DESC LIMIT 1",
    )?;
    let mut out = Vec::new();
    for (id, source, started, bot, role, status) in rows {
        // A companion task with its full link must pass the send path's guards.
        if let (true, Some(bot), Some(role), Some(status)) = (source == "bot", &bot, &role, &status)
            && !can_resume(bot, role, status, shadowed)
        {
            log::info!(target: "interrupted-tasks-resume", "{id}: companion task cannot resume");
            continue;
        }
        let at: Option<i64> = last.query_row((&id, started), |r| r.get(0)).optional()?;
        let content: Option<String> = tool.query_row((&id, started), |r| r.get(0)).optional()?;
        out.push(Pending {
            interrupted_at: at.unwrap_or(started),
            tool: tool_use(content.as_deref()),
            session_id: id,
        });
    }
    Ok(out)
}

/// `companionShadowStore().read()`: the owner's shadowed companions; a bad file reads as none.
pub fn shadowed(owner_dir: &Path) -> Vec<String> {
    let path = owner_dir.join("companion-cloud").join("shadowed.json");
    let text = match crate::atomic_file::read(&path) {
        Ok(text) => text.unwrap_or_else(|| "[]".to_owned()),
        Err(error) => {
            log::warn!(target: "interrupted-tasks-resume", "shadowed companions unread: {error}");
            return Vec::new();
        }
    };
    match js::parse(&text) {
        Ok(Js::Arr(ref items)) => items
            .iter()
            .filter_map(|v| v.str().map(str::to_owned))
            .collect(),
        Ok(_) => Vec::new(),
        Err(_) => {
            log::warn!(target: "interrupted-tasks-resume", "shadowed companions: not JSON");
            Vec::new()
        }
    }
}

/// Starts the owner's load once the database is ready: five tries, `2000 * attempt` ms apart.
pub fn load(cx: &mut App) {
    let State::Ready(db) = &cx.global::<LocalDb>().state else {
        return;
    };
    let db = db.clone();
    let owner = cx.global::<Sessions>().owner_dir.clone();
    let store = cx.default_global::<Interrupted>();
    let Some(dir) = owner.clone().filter(|_| store.owner != owner) else {
        return;
    };
    (store.owner, store.candidates) = (owner, None);
    let boot_at = crate::cross_process_lock::started_ms() as i64;
    cx.spawn(async move |cx| {
        let mut found = Vec::new();
        for attempt in 1..=ATTEMPTS {
            let dir = dir.clone();
            let read = db.call(move |conn| candidates(conn, boot_at, &shadowed(&dir)));
            match read.await {
                Ok(Ok(rows)) => {
                    found = rows;
                    break;
                }
                Ok(Err(error)) if attempt < ATTEMPTS => {
                    log::warn!(target: "interrupted-tasks-resume", "load attempt {attempt}: {error}");
                }
                Ok(Err(error)) => {
                    log::warn!(target: "interrupted-tasks-resume", "interrupted tasks load gave up: {error}");
                    break;
                }
                Err(_) => {
                    log::error!(target: "interrupted-tasks-resume", "the database thread dropped the load");
                    break;
                }
            }
            let wait = Duration::from_millis(2000 * attempt);
            cx.background_executor().timer(wait).await;
        }
        log::info!(target: "interrupted-tasks-resume", "found {} interrupted task(s)", found.len());
        cx.update(|cx| {
            let t = cx.global::<crate::i18n::Translator>();
            let found = found.into_iter().map(|p| Candidate::new(p, t)).collect();
            cx.default_global::<Interrupted>().candidates = Some(found);
        });
    })
    .detach();
}

#[cfg(test)]
mod tests;
