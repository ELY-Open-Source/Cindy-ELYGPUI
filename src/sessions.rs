//! Cindy's `sessionsStore.ts` for this window's bucket: the task list, read from the local
//! database and read again whenever that database changes on disk.

mod flat;
mod query;
mod sweep;

use std::path::PathBuf;

use gpui::App;

use crate::localdb::{Db, LocalDb, State};
pub use flat::list;
use query::{Row, Status};

/// The sidebar filter's status preference.
const STATUS_KEY: &str = "cc-agent.sidebar.filter.status";

pub struct Sessions {
    rows: Option<Vec<Row>>,
    loading: bool,
    dirty: bool,
    /// The database handle the sweep has run for; a new open sweeps again.
    swept_for: Option<Db>,
    user_data: PathBuf,
    _watcher: Option<notify::RecommendedWatcher>,
}

impl gpui::Global for Sessions {}

/// Reads the list whenever the database opens, and after each change Cindy writes.
pub fn install(user_data: PathBuf, cx: &mut App) {
    cx.set_global(Sessions {
        rows: None,
        loading: false,
        dirty: false,
        swept_for: None,
        user_data,
        _watcher: None,
    });
    cx.observe_global::<LocalDb>(|cx| {
        if matches!(cx.global::<LocalDb>().state, State::Ready(_)) {
            watch(cx);
            reload(cx);
        }
    })
    .detach();
}

/// Cindy's push events reach Cindy's own windows only; here the database file and its WAL
/// changing on disk stand for them.
fn watch(cx: &mut App) {
    use notify::{RecursiveMode, Watcher as _};
    let Some(path) = cx.global::<LocalDb>().path().map(PathBuf::from) else {
        return;
    };
    let (changed, mut changes) = futures::channel::mpsc::unbounded();
    let names = [
        path.clone(),
        PathBuf::from(format!("{}-wal", path.display())),
    ];
    let watcher =
        notify::recommended_watcher(move |event: notify::Result<notify::Event>| match event {
            Ok(event)
                if event.kind.is_modify() && event.paths.iter().any(|p| names.contains(p)) =>
            {
                let _ = changed.unbounded_send(());
            }
            Ok(_) => {}
            Err(error) => log::error!(target: "sessions", "database watcher failed: {error}"),
        });
    let watched = watcher.and_then(|mut w| {
        w.watch(
            path.parent().expect("a database has a folder"),
            RecursiveMode::NonRecursive,
        )?;
        Ok(w)
    });
    match watched {
        Ok(watcher) => cx.global_mut::<Sessions>()._watcher = Some(watcher),
        Err(error) => {
            return log::error!(target: "sessions", "database watcher not started: {error}");
        }
    }
    cx.spawn(async move |cx| {
        while futures::StreamExt::next(&mut changes).await.is_some() {
            cx.update(reload);
        }
    })
    .detach();
}

/// `forceRefresh` with Cindy's single flight: a change during a read reads once more after.
pub fn reload(cx: &mut App) {
    let State::Ready(db) = &cx.global::<LocalDb>().state else {
        return;
    };
    let db = db.clone();
    let status = Status::parse(
        cx.global::<crate::preferences::Preferences>()
            .text(STATUS_KEY)
            .ok()
            .flatten(),
    );
    let store = cx.global_mut::<Sessions>();
    if store.loading {
        store.dirty = true;
        return;
    }
    store.loading = true;
    let sweep = store.swept_for.as_ref() != Some(&db);
    store.swept_for = Some(db.clone());
    let user_data = store.user_data.clone();
    let read = db.call(move |conn| {
        if sweep {
            let now = jiff::Timestamp::now().as_millisecond();
            sweep::sweep(conn, &user_data, now);
        }
        query::list(conn, status)
    });
    cx.spawn(async move |cx| {
        let rows = read.await;
        cx.update(|cx| {
            let store = cx.global_mut::<Sessions>();
            store.loading = false;
            match rows {
                Ok(Ok(rows)) => {
                    log::info!(target: "sessions", "{} {status:?} tasks", rows.len());
                    store.rows = Some(rows);
                }
                Ok(Err(error)) => log::error!(target: "sessions", "task list read failed: {error}"),
                Err(_) => log::error!(target: "sessions", "the database closed during the read"),
            }
            if std::mem::take(&mut store.dirty) {
                reload(cx);
            }
            cx.refresh_windows();
        });
    })
    .detach();
}

#[cfg(test)]
mod tests;
