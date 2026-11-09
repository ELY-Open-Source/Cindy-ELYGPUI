//! `local-db:sessions:list`'s rows: `selectSessionListRows` under `ordinaryTaskListFilter`.

use rusqlite::Connection;

use super::{
    group::{Recent, Session},
    paths::{comparison_key, grouping},
    preview, projection,
};

/// `DESKTOP_TASK_LIST_SESSION_SOURCES`: the visible sources less `review` and `side-chat`;
/// `CINDY_MAKE_ENABLED` is off at the reference commit.
pub(super) const TASK_SOURCES: &str = "'desktop', 'feishu', 'slack', 'telegram', 'x', 'discord', 'wechat', \
                            'dingtalk', 'wecom', 'scheduler', 'learn', 'shared', 'plugin'";

/// `ordinaryTaskListFilter`: task sources, a companion's routine or channel task, never hidden.
const ORDINARY: &str = "(sessions.source IN (TASK_SOURCES) OR (sessions.source = 'bot' AND EXISTS (
        SELECT 1 FROM bot_session_links task_link
        JOIN bot_profiles task_bot ON task_bot.id = task_link.bot_id
        WHERE task_link.session_id = sessions.id AND task_link.role IN ('routine', 'channel')
          AND task_bot.status <> 'deleting' AND (task_bot.hidden_at IS NULL OR task_bot.hidden_at = 0)
      ))) AND sessions.hidden_at IS NULL";

/// The sidebar's `FilterStatus`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Active,
    Archived,
    All,
}

impl Status {
    /// `loadStatus`: anything else is `active`.
    pub fn parse(raw: Option<&str>) -> Self {
        match raw {
            Some("archived") => Self::Archived,
            Some("all") => Self::All,
            _ => Self::Active,
        }
    }

    fn sql(self) -> &'static str {
        match self {
            Self::Active => "sessions.status = 'active'",
            Self::Archived => "sessions.status = 'archived'",
            Self::All => "sessions.status != 'deleted'",
        }
    }
}

/// `DEFAULT_LIMIT`, the store's fetch size, within `MAX_LIMIT`.
pub const LIMIT: i64 = 1000;

/// `SESSION_MESSAGE_COUNT_SQL`: the cached count, else a count of the messages.
const MESSAGES: &str =
    "CASE WHEN sessions.list_message_count IS NOT NULL THEN sessions.list_message_count
    ELSE (SELECT count(*) FROM messages m WHERE m.session_id = sessions.id) END";

/// Newest `updated_at` first, ties by `id`, as the list's CTE picks them; with the rows whose
/// cached preview or count is still empty.
pub fn list(
    db: &Connection,
    status: Status,
) -> rusqlite::Result<(Vec<Session>, Vec<projection::Item>)> {
    let filter = ORDINARY.replace("TASK_SOURCES", TASK_SOURCES);
    let tail = format!(
        "{filter} AND {} ORDER BY sessions.updated_at DESC, sessions.id DESC LIMIT ?",
        status.sql()
    );
    read(db, &tail, rusqlite::params_from_iter([LIMIT]))
}

/// `selectSessionsByIds`' batch of ids per query.
const ID_BATCH: usize = 256;

/// `selectSessionsByIds`: the rows of `ids` in their order, missing ones left out.
pub fn by_ids(db: &Connection, ids: &[String]) -> rusqlite::Result<Vec<Session>> {
    let mut seen = std::collections::HashSet::new();
    let unique: Vec<&String> = ids.iter().filter(|id| seen.insert(*id)).collect();
    let mut found = std::collections::HashMap::new();
    for batch in unique.chunks(ID_BATCH) {
        let marks = vec!["?"; batch.len()].join(", ");
        let tail = format!("sessions.id IN ({marks})");
        let (rows, _) = read(db, &tail, rusqlite::params_from_iter(batch))?;
        found.extend(rows.into_iter().map(|s| (s.id.clone(), s)));
    }
    Ok(unique.iter().filter_map(|id| found.remove(*id)).collect())
}

/// The columns `sessionToCamel` reads, under `tail`'s filter and order.
fn read(
    db: &Connection,
    tail: &str,
    params: impl rusqlite::Params,
) -> rusqlite::Result<(Vec<Session>, Vec<projection::Item>)> {
    let sql = format!(
        "SELECT sessions.id, sessions.title, sessions.status, sessions.source, sessions.workspace_kind,
                sessions.working_dir, sessions.project_dir, sessions.remote_host_id, sessions.user_send_at,
                sessions.pinned_at, {MESSAGES}, sessions.created_at,
                coalesce(sessions.last_activity_at, sessions.user_send_at, sessions.updated_at),
                sessions.agent_kind, sessions.orca_role, sessions.total_token_usage,
                sessions.total_cost_usd, sessions.total_cost_amount, sessions.total_cost_currency,
                sessions.notifications_muted, {}, sessions.list_preview, sessions.list_preview_role,
                sessions.summary, sessions.updated_at
         FROM sessions WHERE {tail}",
        projection::columns(),
    );
    let mut statement = db.prepare(&sql)?;
    let rows = statement.query_map(params, |row| {
        let (extract, role): (Option<String>, Option<String>) = (row.get(20)?, row.get(21)?);
        let cached: (Option<String>, Option<String>) = (row.get(24)?, row.get(25)?);
        // The cached preview with its own role, else the latest message.
        let preview = match &cached.0 {
            Some(text) => preview::finalize(Some(text), cached.1.as_deref()),
            None => preview::finalize(extract.as_deref(), role.as_deref()),
        };
        let orca: Option<String> = row.get(14)?;
        let session = Session {
            id: row.get(0)?,
            title: row.get(1)?,
            status: row.get(2)?,
            source: row.get(3)?,
            dialogue: row.get::<_, Option<String>>(4)?.as_deref() == Some("dialogue"),
            working_dir: row.get(5)?,
            project_dir: row.get(6)?,
            remote_host: row.get(7)?,
            user_send_at: row.get(8)?,
            pinned_at: row.get(9)?,
            messages: row.get(10)?,
            created_at: row.get(11)?,
            activity_at: row.get(12)?,
            agent_kind: row.get(13)?,
            orca_lead: orca.as_deref() == Some("lead"),
            orca_worker: orca.as_deref() == Some("worker"),
            tokens: row.get(15)?,
            cost_usd: row.get(16)?,
            cost_amount: row.get(17)?,
            cost_currency: row.get(18)?,
            muted: row.get(19)?,
            preview,
            summary: row.get(26)?,
            updated_at: row.get(27)?,
        };
        let uncached = (row.get(22)?, row.get(23)?);
        let item = projection::item(
            &session.id,
            extract.as_deref(),
            role.as_deref(),
            uncached,
            session.messages,
        );
        Ok((session, item))
    })?;
    let mut sessions = Vec::new();
    let mut items = Vec::new();
    for row in rows {
        let (session, item) = row?;
        sessions.push(session);
        items.extend(item);
    }
    Ok((sessions, items))
}

/// `listRecentWorkdirs`: paths last used first, with the agent kinds of their local tasks.
pub fn recent(db: &Connection, platform: &str) -> rusqlite::Result<Vec<Recent>> {
    let mut statement =
        db.prepare("SELECT path, last_used_at FROM recent_workdirs ORDER BY last_used_at DESC")?;
    let paths: Vec<(String, i64)> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let mut statement = db.prepare(
        "SELECT DISTINCT working_dir, project_dir, agent_kind FROM sessions
         WHERE workspace_kind = 'project' AND remote_host_id IS NULL AND working_dir IS NOT NULL
           AND (orca_role IS NULL OR orca_role <> 'worker')",
    )?;
    let tasks: Vec<(Option<String>, String)> = statement
        .query_map([], |row| {
            let (working, project): (Option<String>, Option<String>) = (row.get(0)?, row.get(1)?);
            Ok((project.or(working), row.get(2)?))
        })?
        .collect::<Result<_, _>>()?;
    Ok(with_kinds(paths, &tasks, platform))
}

/// `recentWorkdirProjectIdentity` matched against each task's folder.
pub fn with_kinds(
    paths: Vec<(String, i64)>,
    tasks: &[(Option<String>, String)],
    platform: &str,
) -> Vec<Recent> {
    let identity =
        |dir: &str| grouping(Some(dir)).map(|d| comparison_key(&format!("local:{d}"), platform));
    paths
        .into_iter()
        .map(|(path, last_used)| {
            let own = identity(&path);
            let mut kinds: Vec<String> = tasks
                .iter()
                .filter(|(dir, _)| own.is_some() && dir.as_deref().and_then(identity) == own)
                .map(|(_, kind)| kind.clone())
                .collect();
            kinds.sort();
            kinds.dedup();
            Recent {
                path,
                kinds,
                last_used,
            }
        })
        .collect()
}

/// `listProjectAliases`: key and alias, newest first.
pub fn aliases(db: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    let mut statement =
        db.prepare("SELECT project_key, alias FROM project_aliases ORDER BY updated_at DESC")?;
    let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    rows.collect()
}
