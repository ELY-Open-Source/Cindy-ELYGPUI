//! Cindy's `sessionsStore.ts` for this window's bucket: the task list, read from the local
//! database and read again whenever that database changes on disk.

mod fold;
mod group;
mod lists;
mod main_list;
mod paths;
mod pinned;
mod query;
mod row;
mod settings;
mod sweep;

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use gpui::App;

use crate::localdb::{Db, LocalDb, State};
use group::Session;
use query::Status;

/// The sidebar filter's status preference.
const STATUS_KEY: &str = "cc-agent.sidebar.filter.status";

/// What one read brings: the bucket's tasks and what grouping them needs.
pub struct Loaded {
    sessions: Vec<Session>,
    recent: Vec<String>,
    aliases: Vec<(String, String)>,
    settings: settings::Settings,
}

pub struct Sessions {
    loaded: Option<Loaded>,
    /// Folded project groups; 13.0 keeps them in owner-scoped storage.
    collapsed: HashSet<String>,
    folds: HashMap<String, crate::motion::Tween>,
    show_all: HashSet<String>,
    show_all_entries: bool,
    loading: bool,
    dirty: bool,
    /// The database handle the sweep has run for; a new open sweeps again.
    swept_for: Option<Db>,
    user_data: PathBuf,
    /// `owners/<key>`, where `sidebar-settings.json` lives.
    owner_dir: Option<PathBuf>,
    _watcher: Option<notify::RecommendedWatcher>,
}

impl gpui::Global for Sessions {}

/// Reads the list whenever the database opens, and after each change Cindy writes.
pub fn install(user_data: PathBuf, owner_dir: Option<PathBuf>, cx: &mut App) {
    cx.set_global(Sessions {
        loaded: None,
        collapsed: HashSet::new(),
        folds: HashMap::new(),
        show_all: HashSet::new(),
        show_all_entries: false,
        loading: false,
        dirty: false,
        swept_for: None,
        user_data,
        owner_dir,
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

/// Cindy's push events reach Cindy's own windows only; here the database, its WAL and the
/// sidebar settings changing on disk stand for them.
fn watch(cx: &mut App) {
    use notify::{RecursiveMode, Watcher as _};
    let Some(path) = cx.global::<LocalDb>().path().map(PathBuf::from) else {
        return;
    };
    let (changed, mut changes) = futures::channel::mpsc::unbounded();
    let owner_dir = cx.global::<Sessions>().owner_dir.clone();
    let mut names = vec![
        path.clone(),
        PathBuf::from(format!("{}-wal", path.display())),
    ];
    names.extend(owner_dir.iter().map(|dir| dir.join(settings::FILE)));
    let watcher =
        notify::recommended_watcher(move |event: notify::Result<notify::Event>| match event {
            Ok(event)
                if (event.kind.is_modify() || event.kind.is_create())
                    && event.paths.iter().any(|p| names.contains(p)) =>
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
        if let Some(dir) = &owner_dir
            && let Err(error) = w.watch(dir, RecursiveMode::NonRecursive)
        {
            log::warn!(target: "sessions", "sidebar settings unwatched in {}: {error}", dir.display());
        }
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
    let owner_dir = store.owner_dir.clone();
    let read = db.call(move |conn| {
        if sweep {
            let now = jiff::Timestamp::now().as_millisecond();
            sweep::sweep(conn, &user_data, now);
        }
        Ok::<_, rusqlite::Error>(Loaded {
            sessions: query::list(conn, status)?,
            recent: query::recent(conn)?,
            aliases: query::aliases(conn)?,
            settings: owner_dir
                .map(|dir| settings::read(&dir, paths::PLATFORM))
                .unwrap_or_default(),
        })
    });
    cx.spawn(async move |cx| {
        let rows = read.await;
        cx.update(|cx| {
            let store = cx.global_mut::<Sessions>();
            store.loading = false;
            match rows {
                Ok(Ok(loaded)) => {
                    log::info!(target: "sessions", "{} {status:?} tasks, {} recent projects, {} aliases, {} pins ordered, {} hidden projects",
                        loaded.sessions.len(), loaded.recent.len(), loaded.aliases.len(),
                        loaded.settings.pinned_order.len(), loaded.settings.hidden.len());
                    store.loaded = Some(loaded);
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

pub use main_list::list;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod view_tests;
