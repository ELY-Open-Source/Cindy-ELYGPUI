//! `messages.ts`'s history reads: `readMessagesList` and the reader's dependencies.

use std::collections::HashSet;

use rusqlite::{Connection, OptionalExtension, types::Value};

use super::{LIVE, Row, outline};
use crate::{
    js_date,
    localdb::{camel, search::mcp_app::TrustLabels},
    rich_answer::js::{self, Js, Obj},
};

/// `DEFAULT_LIMIT` and `MAX_LIMIT`.
const DEFAULT_LIMIT: usize = 50;
const MAX_LIMIT: usize = 100;
/// `readMessagesList`'s batch in outline form.
const OUTLINE_LIMIT: usize = 1000;
/// `getMessageSelectFields` with the rowid.
const FIELDS: &str = "id, client_id, session_id, role, content, tool_use_id, agent_meta, agent_kind, created_at, rowid";

/// A `throwIpcError`, or the database failing under it.
#[derive(Debug)]
pub enum Error {
    Ipc(&'static str, &'static str),
    Db(rusqlite::Error),
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        Error::Db(error)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Ipc(code, message) => write!(f, "[{code}] {message}"),
            Error::Db(error) => write!(f, "{error}"),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// The list options the reader passes.
#[derive(Default)]
pub struct Opts {
    pub limit: Option<f64>,
    pub before: Option<String>,
    pub before_ts: Option<f64>,
    pub after: Option<String>,
}

/// `clampLimit`.
fn clamp_limit(raw: Option<f64>) -> usize {
    match raw {
        Some(n) if n.is_finite() && n > 0.0 => (n.floor() as usize).clamp(1, MAX_LIMIT),
        _ => DEFAULT_LIMIT,
    }
}

/// How a read takes `content`: whole, outlined, or outlined with its stamp.
#[derive(Clone, Copy, PartialEq)]
enum Form {
    Full,
    Outline,
    Stamped,
}

fn select(form: Form) -> String {
    let outlined = || FIELDS.replacen("content,", &format!("{} AS content,", outline::CONTENT), 1);
    match form {
        Form::Full => FIELDS.to_owned(),
        Form::Outline => outlined(),
        Form::Stamped => format!("{}, {} AS stamp", outlined(), outline::STAMP),
    }
}

fn form(outline: bool, stamped: bool) -> Form {
    match (outline, stamped) {
        (false, _) => Form::Full,
        (true, false) => Form::Outline,
        (true, true) => Form::Stamped,
    }
}

/// `messageToCamelWithRowid`, with the outline's content stamp after it.
fn camel(row: &rusqlite::Row, form: Form, labels: &TrustLabels) -> rusqlite::Result<Row> {
    let text = |i: usize| row.get::<_, String>(i);
    let (role, client_id): (String, String) = (text(3)?, text(1)?);
    let agent_meta: Option<String> = row.get(6)?;
    let created: i64 = row.get(8)?;
    let mut out = Obj::new();
    out.insert("id".into(), Js::Str(text(0)?));
    out.insert("clientId".into(), Js::Str(client_id.clone()));
    out.insert("sessionId".into(), Js::Str(text(2)?));
    let shape = (role.as_str(), client_id.as_str(), agent_meta.as_deref());
    let content = camel::content(text(4)?, shape, labels);
    out.insert("role".into(), Js::Str(role));
    out.insert("content".into(), content);
    let optional = |v: Option<String>| v.map_or(Js::Null, Js::Str);
    out.insert("toolUseId".into(), optional(row.get(5)?));
    let meta = agent_meta
        .and_then(|m| js::parse(&m).ok())
        .unwrap_or(Js::Null);
    out.insert("agentMeta".into(), meta);
    out.insert("agentKind".into(), optional(row.get(7)?));
    let iso = js_date::iso(created).ok_or_else(|| {
        let error = format!("Invalid time value {created}").into();
        rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Integer, error)
    })?;
    out.insert("createdAt".into(), Js::Str(iso));
    out.insert("rowid".into(), Js::Num(row.get::<_, i64>(9)? as f64));
    if form == Form::Stamped {
        out.insert("historyContentStamp".into(), optional(row.get(10)?));
    }
    Ok(Js::Obj(out))
}

/// `createdAt` and rowid of a cursor row.
fn cursor(db: &Connection, sql: &str, params: &[&str]) -> rusqlite::Result<Option<(i64, i64)>> {
    db.query_row(sql, rusqlite::params_from_iter(params), |r| {
        Ok((r.get(0)?, r.get(1)?))
    })
    .optional()
}

/// The session's `clearedAt`, None when the task is gone.
fn cleared(db: &Connection, session: &str) -> rusqlite::Result<Option<Option<i64>>> {
    db.query_row(
        "SELECT cleared_at FROM sessions WHERE id = ? LIMIT 1",
        [session],
        |r| r.get(0),
    )
    .optional()
}

/// `readMessagesList` past its import side effects: newest first, or oldest first after `after`.
pub fn list(
    db: &Connection,
    session: &str,
    opts: &Opts,
    outline: bool,
    labels: &TrustLabels,
) -> Result<Vec<Row>> {
    let limit = if outline {
        OUTLINE_LIMIT
    } else {
        clamp_limit(opts.limit)
    };
    let mut before = None;
    let mut before_ms = None;
    if let Some(id) = opts.before.as_deref().filter(|b| !b.is_empty()) {
        before = cursor(
            db,
            "SELECT created_at, rowid FROM messages WHERE id = ? LIMIT 1",
            &[id],
        )?;
    } else if let Some(ms) = opts.before_ts.filter(|t| t.is_finite()) {
        before_ms = Some(ms);
    }
    let mut after = None;
    if before.is_none()
        && before_ms.is_none()
        && let Some(id) = opts.after.as_deref().filter(|a| !a.is_empty())
    {
        let sql = "SELECT created_at, rowid FROM messages WHERE id = ? AND session_id = ? LIMIT 1";
        after = cursor(db, sql, &[id, session])?;
    }
    let mut conds = vec!["session_id = ?".to_owned(), "rewind_at IS NULL".to_owned()];
    let mut params = vec![Value::Text(session.to_owned())];
    let mut stable = |op: &str, (created, rowid): (i64, i64)| {
        conds.push(format!(
            "(created_at {op} ? OR (created_at = ? AND rowid {op} ?))"
        ));
        params.extend([
            Value::Integer(created),
            Value::Integer(created),
            Value::Integer(rowid),
        ]);
    };
    if let Some(at) = before {
        stable("<", at);
    } else if let Some(ms) = before_ms {
        conds.push("created_at < ?".into());
        params.push(Value::Real(ms));
    } else if let Some(at) = after {
        stable(">", at);
    }
    if let Some(at) = cleared(db, session)?.flatten() {
        conds.push("created_at > ?".into());
        params.push(Value::Integer(at));
    }
    let order = if after.is_some() { "ASC" } else { "DESC" };
    let sql = format!(
        "SELECT {} FROM messages WHERE {} ORDER BY created_at {order}, rowid {order} LIMIT {limit}",
        select(form(outline, true)),
        conds.join(" AND ")
    );
    let mut statement = db.prepare(&sql)?;
    let rows = statement.query_map(rusqlite::params_from_iter(params), |r| {
        camel(r, form(outline, true), labels)
    })?;
    let mut rows = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    if after.is_some() {
        rows.reverse();
    }
    log::debug!(target: "localDb", "messages list {} rows, outline {outline}", rows.len());
    Ok(rows)
}

/// The `clearedAt` cut and the task's presence, as each reader dependency checks them.
fn visible(db: &Connection, session: &str) -> Result<Option<i64>> {
    cleared(db, session)?.ok_or(Error::Ipc("NOT_FOUND", "History is unavailable"))
}

fn after_clear(cut: Option<i64>, params: &mut Vec<Value>) -> &'static str {
    match cut {
        Some(at) => {
            params.push(Value::Integer(at));
            " AND created_at > ?"
        }
        None => "",
    }
}

/// The reader's `anchor`: one visible row by id, or a live id by its client id.
pub fn anchor(
    db: &Connection,
    session: &str,
    id: &str,
    outline: bool,
    labels: &TrustLabels,
) -> Result<Row> {
    let cut = visible(db, session)?;
    let (column, key) = match id.strip_prefix(LIVE) {
        Some(client) => ("client_id", client),
        None => ("id", id),
    };
    let mut params = vec![Value::Text(session.to_owned()), Value::Text(key.to_owned())];
    let clear = after_clear(cut, &mut params);
    let sql = format!(
        "SELECT {} FROM messages WHERE session_id = ? AND {column} = ? AND rewind_at IS NULL{clear} LIMIT 1",
        select(form(outline, false))
    );
    db.query_row(&sql, rusqlite::params_from_iter(params), |r| {
        camel(r, form(outline, false), labels)
    })
    .optional()?
    .ok_or(Error::Ipc("NOT_FOUND", "History range changed"))
}

/// The reader's `hydrate`: full rows by id, in batches of 100, every one still visible.
pub fn hydrate(
    db: &Connection,
    session: &str,
    ids: &[String],
    labels: &TrustLabels,
) -> Result<Vec<Row>> {
    let cut = visible(db, session)?;
    let mut out = Vec::with_capacity(ids.len());
    for batch in ids.chunks(100) {
        let mut params = vec![Value::Text(session.to_owned())];
        params.extend(batch.iter().map(|id| Value::Text(id.clone())));
        let clear = after_clear(cut, &mut params);
        let sql = format!(
            "SELECT {FIELDS} FROM messages WHERE session_id = ? AND id IN ({}) AND rewind_at IS NULL{clear}",
            vec!["?"; batch.len()].join(",")
        );
        let mut statement = db.prepare(&sql)?;
        let rows = statement.query_map(rusqlite::params_from_iter(params), |r| {
            camel(r, Form::Full, labels)
        })?;
        let rows = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        if rows.len() != batch.len() {
            return Err(Error::Ipc("NOT_FOUND", "History range changed"));
        }
        out.extend(rows);
    }
    Ok(out)
}

/// The reader's `validate`: every id, stored or live, still visible. Live rows come with 10.0.
pub fn validate(db: &Connection, session: &str, ids: &[String]) -> Result<()> {
    let cut = visible(db, session)?;
    let mut valid = HashSet::new();
    for batch in ids.chunks(100) {
        let (live, stored): (Vec<&String>, Vec<&String>) =
            batch.iter().partition(|id| id.starts_with(LIVE));
        let mut params = vec![Value::Text(session.to_owned())];
        params.extend(stored.iter().map(|id| Value::Text((*id).clone())));
        params.extend(
            live.iter()
                .map(|id| Value::Text(id[LIVE.len()..].to_owned())),
        );
        let clear = after_clear(cut, &mut params);
        let marks = |n: usize| vec!["?"; n].join(",");
        let sql = format!(
            "SELECT id, client_id FROM messages WHERE session_id = ? AND rewind_at IS NULL
             AND (id IN ({}) OR client_id IN ({})){clear}",
            marks(stored.len()),
            marks(live.len())
        );
        let mut statement = db.prepare(&sql)?;
        let rows = statement.query_map(rusqlite::params_from_iter(params), |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (id, client) = row?;
            valid.insert(id);
            valid.insert(format!("{LIVE}{client}"));
        }
    }
    match ids.iter().all(|id| valid.contains(id)) {
        true => Ok(()),
        false => Err(Error::Ipc("NOT_FOUND", "History range changed")),
    }
}

/// The reader's `revision`: this open, then the write counters of other connections and its own.
pub fn revision(db: &Connection, open: u64) -> Result<String> {
    let sql = "SELECT total_changes() AS changes, data_version AS version FROM pragma_data_version";
    let (changes, version): (i64, i64) = db.query_row(sql, [], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(format!("{open}:{version}:{changes}"))
}

#[cfg(test)]
pub(crate) mod tests;
