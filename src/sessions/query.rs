//! `local-db:sessions:list`'s rows: `selectSessionListRows` under `ordinaryTaskListFilter`.

use rusqlite::Connection;

use super::group::Session;

/// `DESKTOP_TASK_LIST_SESSION_SOURCES`: the visible sources less `review` and `side-chat`;
/// `CINDY_MAKE_ENABLED` is off at the reference commit.
const TASK_SOURCES: &str = "'desktop', 'feishu', 'slack', 'telegram', 'x', 'discord', 'wechat', \
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

/// Newest `updated_at` first, ties by `id`, as the list's CTE picks them.
pub fn list(db: &Connection, status: Status) -> rusqlite::Result<Vec<Session>> {
    let filter = ORDINARY.replace("TASK_SOURCES", TASK_SOURCES);
    let sql = format!(
        "SELECT sessions.id, sessions.title, sessions.status, sessions.source, sessions.workspace_kind,
                sessions.working_dir, sessions.project_dir, sessions.remote_host_id, sessions.user_send_at,
                sessions.pinned_at, {MESSAGES}, sessions.created_at,
                coalesce(sessions.last_activity_at, sessions.user_send_at, sessions.updated_at)
         FROM sessions WHERE {filter} AND {}
         ORDER BY sessions.updated_at DESC, sessions.id DESC LIMIT ?",
        status.sql()
    );
    let mut statement = db.prepare(&sql)?;
    let rows = statement.query_map([LIMIT], |row| {
        Ok(Session {
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
        })
    })?;
    rows.collect()
}

/// `listRecentWorkdirs`' paths, last used first.
pub fn recent(db: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut statement =
        db.prepare("SELECT path FROM recent_workdirs ORDER BY last_used_at DESC")?;
    let rows = statement.query_map([], |row| row.get(0))?;
    rows.collect()
}

/// `listProjectAliases`: key and alias, newest first.
pub fn aliases(db: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    let mut statement =
        db.prepare("SELECT project_key, alias FROM project_aliases ORDER BY updated_at DESC")?;
    let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    rows.collect()
}
