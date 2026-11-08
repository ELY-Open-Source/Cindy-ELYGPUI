//! Cindy's `localDb`: the account-free owner's database, admitted read only as Cindy's
//! passive instances admit it, opened off the main thread.

mod admission;
mod boot;
mod cjk;
mod fatal;
mod lease;
mod open;
mod worker;

use std::path::{Path, PathBuf};

use gpui::App;

pub use fatal::screen;
pub use open::Fatal;
pub use worker::Db;

/// `LocalDbGate`'s states for this window's owner.
pub enum State {
    /// No owner this chapter can open (signed out or cloud, chapter 13).
    NoOwner,
    Opening,
    Ready {
        _db: Db,
    },
    Failed(Fatal),
}

pub struct LocalDb {
    pub state: State,
    path: Option<PathBuf>,
    details_open: bool,
}

impl gpui::Global for LocalDb {}

/// `ownerDatabasePath`: `<userData>/cindy-<ownerId>.db`.
pub fn path(user_data: &Path, owner_id: &str) -> PathBuf {
    user_data.join(format!("cindy-{owner_id}.db"))
}

/// `local-db:ensure-ready` for the owner, if any.
pub fn install(path: Option<PathBuf>, cx: &mut App) {
    let state = if path.is_some() {
        State::Opening
    } else {
        State::NoOwner
    };
    cx.set_global(LocalDb {
        state,
        path,
        details_open: false,
    });
    ensure_ready(cx);
}

/// Opens on a background thread and reports into the global; a retry runs it again.
pub fn ensure_ready(cx: &mut App) {
    let Some(path) = cx.global::<LocalDb>().path.clone() else {
        return;
    };
    cx.global_mut::<LocalDb>().state = State::Opening;
    let translator = cx.global::<crate::i18n::Translator>();
    let texts: std::collections::HashMap<&str, String> = open::TEXT_KEYS
        .iter()
        .map(|key| (*key, translator.t(key, &[])))
        .collect();
    let task = cx.background_executor().spawn(async move {
        open::open(&path, &|key| texts.get(key).cloned().expect("a listed key"))
    });
    cx.spawn(async move |cx| {
        let opened = task.await;
        cx.update(|cx| {
            cx.global_mut::<LocalDb>().state = match opened {
                Ok(db) => State::Ready { _db: db },
                Err(fatal) => State::Failed(fatal),
            };
            cx.refresh_windows();
        });
    })
    .detach();
}

#[cfg(test)]
mod tests;
