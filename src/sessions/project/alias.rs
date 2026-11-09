//! `handleProjectAliasChange`: the list first, then `upsertProjectAlias` on the worker.

use anyhow::{Result, bail};
use gpui::{App, Window};
use rusqlite::{Connection, params};

use super::super::{
    Sessions,
    paths::{PLATFORM, compare, comparison_key, normalize_key},
};
use crate::{
    i18n::Translator,
    localdb::{LocalDb, State},
    theme_import::color::js_trim,
    toast,
};

/// `MAX_ALIAS_LENGTH`, in UTF-16 units.
const MAX_ALIAS: usize = 80;

/// `requireProjectKey` and `sanitizeAlias`: the stored key, and none for a blank alias.
fn checked(key: &str, alias: &str) -> Result<(String, Option<String>)> {
    let Some(key) = normalize_key(key).filter(|k| {
        ["local:", "remote:", "device:"]
            .iter()
            .any(|p| k.starts_with(p))
    }) else {
        bail!("invalid projectKey");
    };
    let alias = js_trim(alias);
    if alias.encode_utf16().count() > MAX_ALIAS {
        bail!("alias must be at most {MAX_ALIAS} characters");
    }
    Ok((key, (!alias.is_empty()).then(|| alias.to_owned())))
}

/// `foldCase`: a Windows drive or UNC folder matches its rows without case.
fn folds(key: &str, platform: &str) -> bool {
    let dir = key.strip_prefix("local:").unwrap_or("");
    let b = dir.as_bytes();
    let drive = b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'/';
    platform == "win32" && (drive || dir.starts_with("//"))
}

/// `upsertProjectAlias`: every row of the project's identity goes, then the alias, if any.
pub fn upsert(db: &Connection, key: &str, alias: &str, platform: &str, now: i64) -> Result<()> {
    let (key, alias) = checked(key, alias)?;
    let fold = folds(&key, platform);
    let identity = if fold {
        comparison_key(&key, platform)
    } else {
        key.clone()
    };
    let tx = db.unchecked_transaction()?;
    let rows: Vec<String> = tx
        .prepare("SELECT project_key FROM project_aliases")?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    for row in rows {
        let row_identity = if fold {
            row.to_lowercase()
        } else {
            row.clone()
        };
        if row_identity == identity {
            tx.execute("DELETE FROM project_aliases WHERE project_key = ?1", [&row])?;
        }
    }
    if let Some(alias) = &alias {
        tx.execute(
            "INSERT INTO project_aliases (project_key, alias, updated_at) VALUES (?1, ?2, ?3)",
            params![key, alias, now],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// The list's rows with the alias laid on, newest first, as `updateAlias` sets them.
fn laid(rows: &[(String, String)], key: &str, alias: &str) -> Vec<(String, String)> {
    let identity = compare(key, PLATFORM);
    let mut next: Vec<(String, String)> = rows
        .iter()
        .filter(|(k, _)| compare(k, PLATFORM) != identity)
        .cloned()
        .collect();
    let alias = js_trim(alias);
    if !alias.is_empty() {
        next.insert(0, (key.to_owned(), alias.to_owned()));
    }
    next
}

fn rows(cx: &mut App) -> Option<&mut Vec<(String, String)>> {
    Some(&mut cx.global_mut::<Sessions>().loaded.as_mut()?.aliases)
}

/// The rename shows at once; a failed write puts the old names back and toasts.
pub fn rename(key: &str, alias: &str, window: &Window, cx: &mut App) {
    let Some(rows) = rows(cx) else {
        return log::warn!(target: "sessions", "rename project {key}: no list loaded");
    };
    let previous = std::mem::take(rows);
    *rows = laid(&previous, key, alias);
    cx.refresh_windows();
    log::info!(target: "sessions", "project {key} renamed");
    let State::Ready(db) = &cx.global::<LocalDb>().state else {
        restore(previous, "the database is not open", cx);
        return toast_failed(window, cx);
    };
    let handle = window.window_handle();
    let (key, alias) = (key.to_owned(), alias.to_owned());
    let now = jiff::Timestamp::now().as_millisecond();
    let task = db.call(move |conn| upsert(conn, &key, &alias, PLATFORM, now));
    cx.spawn(async move |cx| {
        let error = match task.await {
            Ok(Ok(())) => return,
            Ok(Err(error)) => format!("{error:#}"),
            Err(_) => "the database closed during the write".to_owned(),
        };
        cx.update(|cx| {
            restore(previous, &error, cx);
            let shown = handle.update(cx, |_, window, cx| toast_failed(window, cx));
            if shown.is_err() {
                log::warn!(target: "sessions", "renameFailed: its window closed before the toast");
            }
        });
    })
    .detach();
}

fn restore(previous: Vec<(String, String)>, error: &str, cx: &mut App) {
    log::error!(target: "sessions", "[project alias rename] {error}");
    if let Some(rows) = rows(cx) {
        *rows = previous;
    }
    cx.refresh_windows();
}

fn toast_failed(window: &Window, cx: &mut App) {
    let message = cx
        .global::<Translator>()
        .t("ccAgent.sidebar.projectAlias.renameFailed", &[]);
    toast::error(window, message, toast::Options::default(), cx);
}

#[cfg(test)]
mod tests;
