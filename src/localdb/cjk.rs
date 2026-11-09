//! `cjkSeg.ts` and `registerCjkSeg.ts`: Han characters as their own FTS tokens.

use std::sync::OnceLock;

use rusqlite::{Connection, functions::FunctionFlags, types::ValueRef};

const ROLES: &str = "('user', 'assistant', 'ask_user', 'plan_review')";

/// V8's `\p{Script=Han}`, `[\p{L}\p{N}]` and `\p{M}`, from `scripts/sync-golden.mjs`.
#[derive(serde::Deserialize)]
struct Classes {
    han: Vec<[u32; 2]>,
    #[serde(rename = "letterOrNumber")]
    letter_or_number: Vec<[u32; 2]>,
    mark: Vec<[u32; 2]>,
}

fn classes() -> &'static Classes {
    static CLASSES: OnceLock<Classes> = OnceLock::new();
    CLASSES.get_or_init(|| {
        let text = include_str!("../../resources/cjk-classes.json");
        serde_json::from_str(text).expect("the bundled character classes parse")
    })
}

fn within(ranges: &[[u32; 2]], ch: char) -> bool {
    let cp = u32::from(ch);
    let at = ranges.partition_point(|[_, end]| *end < cp);
    ranges.get(at).is_some_and(|[start, _]| *start <= cp)
}

/// V8's `[\p{L}\p{N}]`.
pub fn letter_or_number(ch: char) -> bool {
    within(&classes().letter_or_number, ch)
}

/// `cjkSeg`: a space between Han characters, and between Han and a letter or digit.
pub fn seg(text: &str) -> String {
    let c = classes();
    let mut out = String::with_capacity(text.len() * 2);
    let mut last: Option<char> = None;
    for ch in text.chars() {
        if within(&c.mark, ch) {
            out.push(ch);
            continue;
        }
        if let Some(prev) = last {
            let (han_now, han_prev) = (within(&c.han, ch), within(&c.han, prev));
            if (han_now && han_prev)
                || (han_now && within(&c.letter_or_number, prev))
                || (han_prev && within(&c.letter_or_number, ch))
            {
                out.push(' ');
            }
        }
        out.push(ch);
        last = Some(ch);
    }
    out
}

/// `String(value)` for what better-sqlite3 hands the function.
fn text_of(value: ValueRef) -> Option<String> {
    Some(match value {
        ValueRef::Null => return None,
        ValueRef::Integer(i) => i.to_string(),
        ValueRef::Real(f) => crate::cindy_json::js_number(f),
        ValueRef::Text(t) | ValueRef::Blob(t) => String::from_utf8_lossy(t).into_owned(),
    })
}

fn table_exists(db: &Connection, name: &str) -> rusqlite::Result<bool> {
    let sql = "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?";
    db.query_row(sql, [name], |_| Ok(()))
        .map(|()| true)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(false),
            other => Err(other),
        })
}

fn fts_tables(db: &Connection) -> rusqlite::Result<bool> {
    Ok(table_exists(db, "messages")?
        && table_exists(db, "messages_fts")?
        && table_exists(db, "messages_fts_rows")?)
}

/// `installCjkSegTempTriggers`: connection-only triggers that index segmented text.
fn install_triggers(db: &Connection) -> rusqlite::Result<()> {
    if !fts_tables(db)? {
        return Ok(());
    }
    db.execute_batch(&format!(
        "DROP TRIGGER IF EXISTS temp.messages_fts_insert_cjk;
         DROP TRIGGER IF EXISTS temp.messages_fts_update_cjk;
         CREATE TEMP TRIGGER messages_fts_insert_cjk
         AFTER INSERT ON messages
         WHEN new.rewind_at IS NULL AND new.role IN {ROLES}
         BEGIN
           INSERT OR IGNORE INTO messages_fts_rows(message_id) VALUES (new.id);
           INSERT OR REPLACE INTO messages_fts(rowid, message_id, session_id, role, content)
             SELECT fts_rowid, new.id, new.session_id, new.role, cjk_seg(new.content)
             FROM messages_fts_rows
             WHERE message_id = new.id;
         END;
         CREATE TEMP TRIGGER messages_fts_update_cjk
         AFTER UPDATE OF id, session_id, role, content, rewind_at ON messages
         WHEN old.id IS NOT new.id
           OR old.session_id IS NOT new.session_id
           OR old.role IS NOT new.role
           OR old.content IS NOT new.content
           OR old.rewind_at IS NOT new.rewind_at
         BEGIN
           DELETE FROM messages_fts
             WHERE rowid = (
               SELECT fts_rowid FROM messages_fts_rows WHERE message_id = old.id
             );
           UPDATE messages_fts_rows
             SET message_id = new.id
             WHERE message_id = old.id AND old.id IS NOT new.id;
           INSERT OR IGNORE INTO messages_fts_rows(message_id)
             SELECT new.id
             WHERE new.rewind_at IS NULL AND new.role IN {ROLES};
           INSERT OR REPLACE INTO messages_fts(rowid, message_id, session_id, role, content)
             SELECT fts_rowid, new.id, new.session_id, new.role, cjk_seg(new.content)
             FROM messages_fts_rows
             WHERE message_id = new.id
               AND new.rewind_at IS NULL
               AND new.role IN {ROLES};
         END;"
    ))
}

fn triggers_installed(db: &Connection) -> rusqlite::Result<bool> {
    let sql = "SELECT count(*) FROM temp.sqlite_master WHERE type='trigger' \
               AND name IN ('messages_fts_insert_cjk', 'messages_fts_update_cjk')";
    Ok(db.query_row(sql, [], |row| row.get::<_, i64>(0))? == 2)
}

/// `registerCjkSeg`: the function, then its TEMP triggers.
pub fn register(db: &Connection) -> rusqlite::Result<()> {
    let flags = FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC;
    db.create_scalar_function("cjk_seg", 1, flags, |ctx| {
        Ok(text_of(ctx.get_raw(0)).map(|t| seg(&t)))
    })?;
    install_triggers(db)
}

/// `ensureCjkFtsTempTriggersInstalled`: a `temp_store` pragma drops TEMP triggers; reinstall
/// once and refuse to run with an unindexed `messages` table.
pub fn ensure_triggers(db: &Connection) -> anyhow::Result<()> {
    if !fts_tables(db)? || triggers_installed(db)? {
        return Ok(());
    }
    install_triggers(db)?;
    anyhow::ensure!(
        triggers_installed(db)?,
        "cjk_seg temp triggers failed to install after retry; refusing to start with a silently-unindexed messages table (#3841)"
    );
    Ok(())
}

#[cfg(test)]
mod tests;
