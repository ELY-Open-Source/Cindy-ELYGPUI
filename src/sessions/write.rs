//! `sessionService.patchMeta` for a local task: the row patched in place, the write on the
//! database worker as `updateSessionInDb` makes it, rolled back with a toast if it fails.

use gpui::{App, ClipboardItem, Window};
use rusqlite::{Connection, params};

use super::Sessions;
use crate::{
    cindy_json::encode_uri_component,
    i18n::Translator,
    localdb::{LocalDb, State},
    toast,
};

/// The fields the row menu edits; all are `SETTINGS_ONLY_FIELDS`, so `updatedAt` stays.
#[derive(Clone, Debug, PartialEq)]
pub enum Meta {
    Title(String),
    Muted(bool),
    /// `pinnedAt` in unix ms; none unpins.
    Pinned(Option<i64>),
}

impl Meta {
    fn name(&self) -> &'static str {
        match self {
            Meta::Title(_) => "title",
            Meta::Muted(_) => "notificationsMuted",
            Meta::Pinned(_) => "pinnedAt",
        }
    }
}

/// `writeSessionPatch` without a status: one `UPDATE`, then the row must exist.
pub fn patch(db: &Connection, id: &str, meta: &Meta) -> rusqlite::Result<()> {
    let changed = match meta {
        Meta::Title(title) => db.execute(
            "UPDATE sessions SET title = ?1 WHERE id = ?2",
            params![title, id],
        )?,
        Meta::Muted(muted) => db.execute(
            "UPDATE sessions SET notifications_muted = ?1 WHERE id = ?2",
            params![muted, id],
        )?,
        // An unpinned task's summary has nowhere to show, so it goes too.
        Meta::Pinned(at) => db.execute(
            "UPDATE sessions SET pinned_at = ?1,
               summary = CASE WHEN ?1 IS NULL THEN NULL ELSE summary END WHERE id = ?2",
            params![at, id],
        )?,
    };
    if changed == 0 {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }
    Ok(())
}

/// Sets `meta` on the loaded row and returns what it replaced.
fn set_local(id: &str, meta: &Meta, cx: &mut App) -> Option<Meta> {
    let loaded = cx.global_mut::<Sessions>().loaded.as_mut()?;
    let row = loaded.sessions.iter_mut().find(|s| s.id == id)?;
    Some(match meta {
        Meta::Title(title) => Meta::Title(std::mem::replace(&mut row.title, title.clone())),
        Meta::Muted(muted) => Meta::Muted(std::mem::replace(&mut row.muted, *muted)),
        Meta::Pinned(at) => Meta::Pinned(std::mem::replace(&mut row.pinned_at, *at)),
    })
}

/// An unpin clears the summary too, as `patchLocal` does; returns the one it cleared.
fn clear_summary(id: &str, meta: &Meta, cx: &mut App) -> Option<String> {
    if *meta != Meta::Pinned(None) {
        return None;
    }
    let loaded = cx.global_mut::<Sessions>().loaded.as_mut()?;
    let row = loaded.sessions.iter_mut().find(|s| s.id == id)?;
    row.summary.take()
}

fn current(id: &str, cx: &App) -> Option<bool> {
    let loaded = cx.global::<Sessions>().loaded.as_ref()?;
    loaded.sessions.iter().find(|s| s.id == id).map(|s| s.muted)
}

fn failed(window: &Window, key: &'static str, cx: &mut App) {
    let message = cx.global::<Translator>().t(key, &[]);
    toast::error(window, message, toast::Options::default(), cx);
}

/// A failed write: the row restored unless a later one replaced it, and the toast shown.
struct Undo {
    id: String,
    meta: Meta,
    previous: Option<Meta>,
    /// The summary an unpin cleared.
    summary: Option<String>,
    seq: u64,
}

impl Undo {
    /// Restores the row and names the toast to show.
    fn run(self, error: &str, cx: &mut App) -> &'static str {
        log::error!(target: "sessions", "patch {} {} failed: {error}", self.id, self.meta.name());
        let (key, undo) = match &self.meta {
            Meta::Title(_) => ("ccAgent.sidebar.renameFailed", true),
            Meta::Pinned(_) => ("ccAgent.sidebar.pinFailed", true),
            Meta::Muted(muted) => (
                "ccAgent.sidebar.muteNotificationsFailed",
                cx.global::<Sessions>().latest_mute.get(&self.id) == Some(&self.seq)
                    && current(&self.id, cx) == Some(*muted),
            ),
        };
        if let (true, Some(previous)) = (undo, self.previous) {
            set_local(&self.id, &previous, cx);
            if let Some(summary) = self.summary {
                let loaded = cx.global_mut::<Sessions>().loaded.as_mut();
                let row = loaded.and_then(|l| l.sessions.iter_mut().find(|s| s.id == self.id));
                if let Some(row) = row {
                    row.summary = Some(summary);
                }
            }
            cx.refresh_windows();
        }
        key
    }
}

/// `handleRename`, `handleTogglePin` and `setSessionNotificationsMuted`: the row first, then
/// the write.
pub fn apply(id: &str, meta: Meta, window: &Window, cx: &mut App) {
    let previous = set_local(id, &meta, cx);
    let summary = clear_summary(id, &meta, cx);
    let store = cx.global_mut::<Sessions>();
    store.writes += 1;
    let seq = store.writes;
    if matches!(meta, Meta::Muted(_)) {
        store.latest_mute.insert(id.to_owned(), seq);
    }
    cx.refresh_windows();
    log::info!(target: "sessions", "patch {id} {}", meta.name());
    let undo = Undo {
        id: id.to_owned(),
        meta: meta.clone(),
        previous,
        summary,
        seq,
    };
    let State::Ready(db) = &cx.global::<LocalDb>().state else {
        let key = undo.run("the database is not open", cx);
        failed(window, key, cx);
        return;
    };
    let handle = window.window_handle();
    let (sid, write) = (id.to_owned(), meta);
    let task = db.call(move |conn| patch(conn, &sid, &write));
    cx.spawn(async move |cx| {
        let result = task.await;
        cx.update(|cx| {
            let key = match result {
                Ok(Ok(())) => return,
                Ok(Err(error)) => undo.run(&error.to_string(), cx),
                Err(_) => undo.run("the database closed during the write", cx),
            };
            let shown = handle.update(cx, |_, window, cx| failed(window, key, cx));
            if shown.is_err() {
                log::warn!(target: "sessions", "{key}: its window closed before the toast");
            }
        });
    })
    .detach();
}

/// `buildSessionDeepLink` for a local task.
pub fn deep_link(id: &str) -> String {
    format!("cindy://session/{}", encode_uri_component(id))
}

/// `handleCopyDeepLinkSelect`.
pub fn copy_link(id: &str, window: &Window, cx: &mut App) {
    cx.write_to_clipboard(ClipboardItem::new_string(deep_link(id)));
    log::info!(target: "sessions", "copied the link of {id}");
    let message = cx
        .global::<Translator>()
        .t("ccAgent.sidebar.deepLink.copied", &[]);
    toast::success(window, message, toast::Options::default(), cx);
}
