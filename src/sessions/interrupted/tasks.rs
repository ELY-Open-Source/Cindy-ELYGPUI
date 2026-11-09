//! `loadInterruptedTasks`: the candidates with their rows and companion names, newest first.

use std::collections::HashMap;

use rusqlite::Connection;

use super::{Pending, candidates};
use crate::{
    i18n::Translator,
    sessions::{group::Session, query},
};

/// `InterruptedTask`.
#[derive(Debug, PartialEq)]
pub struct Task {
    pub session: Session,
    pub interrupted_at: i64,
    /// The last tool call's island line, else its name.
    pub activity: Option<String>,
    /// The companion's name, shown in place of a companion task's source.
    pub companion: Option<String>,
}

/// What the database worker reads in one go.
pub struct Read {
    pending: Vec<Pending>,
    sessions: Vec<Session>,
    names: HashMap<String, String>,
}

/// The candidates, `getMany`'s rows for them and, with a companion task, `bots.list`'s names.
pub fn read(db: &Connection, boot_at: i64, shadowed: &[String]) -> rusqlite::Result<Read> {
    let pending = candidates(db, boot_at, shadowed)?;
    let ids: Vec<String> = pending.iter().map(|p| p.session_id.clone()).collect();
    let sessions = query::by_ids(db, &ids)?;
    let names = if sessions.iter().any(|s| s.source == "bot") {
        companion_names(db).unwrap_or_else(|error| {
            log::warn!(target: "interrupted-tasks-resume", "companion names unread: {error}");
            HashMap::new()
        })
    } else {
        HashMap::new()
    };
    Ok(Read {
        pending,
        sessions,
        names,
    })
}

/// `loadBotNamesBySessionId`: each linked task's companion, profiles newest first, the last wins.
fn companion_names(db: &Connection) -> rusqlite::Result<HashMap<String, String>> {
    let mut statement = db.prepare(
        "SELECT bot_session_links.session_id, bot_profiles.display_name
         FROM bot_profiles
         JOIN bot_session_links ON bot_session_links.bot_id = bot_profiles.id
         JOIN sessions ON sessions.id = bot_session_links.session_id
         ORDER BY bot_profiles.updated_at DESC, bot_session_links.created_at DESC",
    )?;
    let rows = statement.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    rows.collect()
}

/// The rows that still exist, newest interruption first.
pub fn assemble(read: Read, t: &Translator) -> Vec<Task> {
    let Read {
        pending,
        sessions,
        names,
    } = read;
    let mut tasks: Vec<Task> = pending
        .into_iter()
        .filter_map(|p| {
            let session = sessions.iter().find(|s| s.id == p.session_id)?.clone();
            let activity = p
                .tool
                .map(|tool| crate::tool_row::activity(t, &tool.name, tool.input.as_ref()));
            Some(Task {
                companion: names.get(&session.id).cloned(),
                session,
                interrupted_at: p.interrupted_at,
                activity,
            })
        })
        .collect();
    tasks.sort_by_key(|t| std::cmp::Reverse(t.interrupted_at));
    tasks
}
