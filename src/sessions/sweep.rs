//! `sweepLeftoverProvisionalSessions`: unsent tasks a past run left behind, removed once.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension as _};
use serde::{Deserialize, Serialize};

const FILE: &str = "provisional-sessions.json";
/// `FOREIGN_ENTRY_TTL_MS`: an entry for another owner's database is kept a week.
const FOREIGN_TTL_MS: i64 = 7 * 24 * 60 * 60_000;

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub(super) struct Entry {
    id: String,
    #[serde(rename = "createdAt")]
    created_at: f64,
    #[serde(rename = "managedDir", skip_serializing_if = "Option::is_none")]
    managed_dir: Option<String>,
}

/// `loadLeftovers`: entries with a string id and a numeric `createdAt`.
fn load(path: &Path) -> Vec<Entry> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => {
            log::warn!(target: "provisional-sessions", "provisional session file unreadable: {error}");
            return Vec::new();
        }
    };
    let parsed = match crate::cindy_json::roundtrip(&text) {
        Ok(parsed) => parsed,
        Err(error) => {
            log::warn!(target: "provisional-sessions", "provisional session file unreadable: {error}");
            return Vec::new();
        }
    };
    let entries = parsed
        .get("entries")
        .and_then(|e| e.as_array())
        .cloned()
        .unwrap_or_default();
    entries
        .iter()
        .filter_map(|entry| {
            let id = entry.get("id")?.as_str().filter(|id| !id.is_empty())?;
            Some(Entry {
                id: id.to_owned(),
                created_at: entry.get("createdAt")?.as_f64()?,
                managed_dir: entry
                    .get("managedDir")
                    .and_then(|d| d.as_str())
                    .map(str::to_owned),
            })
        })
        .collect()
}

/// `persist`: `{ version: 1, entries }` through a temp file, or no file at all.
fn persist(path: &Path, entries: &[Entry]) {
    let written = if entries.is_empty() {
        std::fs::remove_file(path).or_else(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Ok(())
            } else {
                Err(e)
            }
        })
    } else {
        let temp = PathBuf::from(format!("{}.{}.tmp", path.display(), std::process::id()));
        let body = serde_json::json!({ "version": 1, "entries": entries }).to_string();
        std::fs::write(&temp, body).and_then(|()| std::fs::rename(&temp, path))
    };
    if let Err(error) = written {
        log::warn!(target: "provisional-sessions", "provisional session file write failed: {error}");
    }
}

/// `deleteUnsentSessionRow`: only a never-sent task with no messages.
fn delete_unsent(db: &Connection, id: &str) -> rusqlite::Result<bool> {
    let sent: Option<Option<i64>> = db
        .query_row(
            "SELECT user_send_at FROM sessions WHERE id = ? LIMIT 1",
            [id],
            |r| r.get(0),
        )
        .optional()?;
    let Some(None) = sent else { return Ok(false) };
    let messages: i64 = db.query_row(
        "SELECT count(*) FROM messages WHERE session_id = ?",
        [id],
        |r| r.get(0),
    )?;
    if messages > 0 {
        return Ok(false);
    }
    db.execute(
        "DELETE FROM sessions WHERE id = ? AND user_send_at IS NULL",
        [id],
    )?;
    Ok(true)
}

/// `removeEmptyDialogueDir`: only the folder named for the task, only when empty.
fn remove_empty_dir(id: &str, managed: Option<&str>) {
    let Some(dir) = managed.map(Path::new) else {
        return;
    };
    if dir.file_name().and_then(|n| n.to_str()) == Some(id) {
        let _ = std::fs::remove_dir(dir);
    }
}

/// `sweepOwner`, for the database this connection serves. Returns the removed ids.
pub fn sweep(db: &Connection, user_data: &Path, now: i64) -> Vec<String> {
    let path = user_data.join(FILE);
    let pending = load(&path);
    if pending.is_empty() {
        return Vec::new();
    }
    let mut keep = Vec::new();
    let mut removed = Vec::new();
    for entry in &pending {
        let found = db
            .query_row(
                "SELECT id FROM sessions WHERE id = ? LIMIT 1",
                [&entry.id],
                |_| Ok(()),
            )
            .optional();
        let result = found.and_then(|row| match row {
            None => {
                if (now as f64) - entry.created_at < FOREIGN_TTL_MS as f64 {
                    keep.push(entry.clone());
                }
                Ok(None)
            }
            Some(()) => delete_unsent(db, &entry.id).map(Some),
        });
        match result {
            Ok(None) => {}
            Ok(Some(deleted)) => {
                if deleted {
                    removed.push(entry.id.clone());
                    remove_empty_dir(&entry.id, entry.managed_dir.as_deref());
                }
                log::info!(target: "provisional-sessions", "provisional session leftover swept sessionId {} deleted {deleted}", entry.id);
            }
            Err(error) => {
                keep.push(entry.clone());
                log::warn!(target: "provisional-sessions", "provisional session leftover sweep failed sessionId {}: {error}", entry.id);
            }
        }
    }
    let parked: Vec<Entry> = load(&path)
        .into_iter()
        .filter(|e| !pending.contains(e))
        .collect();
    keep.extend(parked);
    persist(&path, &keep);
    removed
}
