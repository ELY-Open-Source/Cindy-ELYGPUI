//! `local-db:sessions:list`'s rows: `selectSessionListRows` under `ordinaryTaskListFilter`.

use rusqlite::Connection;

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

/// A list row with what this chapter shows of it.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub id: String,
    pub title: String,
    pub pinned: bool,
    pub created_at: i64,
    /// `getSessionListActivityAt`: last activity, else the last send, else the update.
    pub activity_at: i64,
}

/// `DEFAULT_LIMIT`, the store's fetch size, within `MAX_LIMIT`.
pub const LIMIT: i64 = 1000;

/// Newest `updated_at` first, ties by `id`, as the list's CTE picks them.
pub fn list(db: &Connection, status: Status) -> rusqlite::Result<Vec<Row>> {
    let filter = ORDINARY.replace("TASK_SOURCES", TASK_SOURCES);
    let sql = format!(
        "SELECT sessions.id, sessions.title, sessions.pinned_at, sessions.created_at,
                coalesce(sessions.last_activity_at, sessions.user_send_at, sessions.updated_at)
         FROM sessions WHERE {filter} AND {}
         ORDER BY sessions.updated_at DESC, sessions.id DESC LIMIT ?",
        status.sql()
    );
    let mut statement = db.prepare(&sql)?;
    let rows = statement.query_map([LIMIT], |row| {
        Ok(Row {
            id: row.get(0)?,
            title: row.get(1)?,
            pinned: row.get::<_, Option<i64>>(2)?.is_some(),
            created_at: row.get(3)?,
            activity_at: row.get(4)?,
        })
    })?;
    rows.collect()
}
