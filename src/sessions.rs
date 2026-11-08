//! Cindy's `sessionsStore.ts` for this window's bucket: the task list, read from the local
//! database and read again whenever that database changes on disk.

mod filter;
mod fold;
mod group;
mod header;
mod list_row;
mod lists;
mod main_list;
mod meta;
mod paths;
mod pin;
mod pinned;
mod preview;
mod projection;
mod query;
mod rename;
mod row;
mod settings;
mod sweep;
mod task_menu;
mod title;
mod view_mode;
mod write;

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
    recent: Vec<group::Recent>,
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
    /// `useTaskInfoFields`' memory copy.
    task_info: Vec<&'static str>,
    /// The row whose title is being edited.
    editing: Option<rename::Editing>,
    /// Writes started, and each task's latest mute write, as `latestWriteSeq` keeps them.
    writes: u64,
    latest_mute: HashMap<String, u64>,
    loading: bool,
    dirty: bool,
    /// The database handle the sweep has run for; a new open sweeps again.
    swept_for: Option<Db>,
    user_data: PathBuf,
    /// `owners/<key>`, where `sidebar-settings.json` lives.
    owner_dir: Option<PathBuf>,
    /// Pinned-order saves in flight, one at a time.
    pins: pin::Writes,
    watcher: Option<notify::RecommendedWatcher>,
    /// Whether the watcher sees `owner_dir`, which the first pin may create.
    owner_watched: bool,
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
        task_info: meta::fields(cx.global::<crate::preferences::Preferences>()),
        editing: None,
        writes: 0,
        latest_mute: HashMap::new(),
        loading: false,
        dirty: false,
        swept_for: None,
        user_data,
        owner_dir,
        pins: pin::Writes::default(),
        watcher: None,
        owner_watched: false,
    });
    view_mode::install(cx);
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
        Ok(w)
    });
    match watched {
        Ok(watcher) => cx.global_mut::<Sessions>().watcher = Some(watcher),
        Err(error) => {
            return log::error!(target: "sessions", "database watcher not started: {error}");
        }
    }
    watch_owner(cx);
    cx.spawn(async move |cx| {
        while futures::StreamExt::next(&mut changes).await.is_some() {
            cx.update(reload);
        }
    })
    .detach();
}

/// Adds `owner_dir` to the watcher once it exists.
fn watch_owner(cx: &mut App) {
    use notify::{RecursiveMode, Watcher as _};
    let store = cx.global_mut::<Sessions>();
    let (Some(watcher), Some(dir), false) =
        (&mut store.watcher, &store.owner_dir, store.owner_watched)
    else {
        return;
    };
    match watcher.watch(dir, RecursiveMode::NonRecursive) {
        Ok(()) => {
            log::info!(target: "sessions", "sidebar settings watched in {}", dir.display());
            store.owner_watched = true;
        }
        Err(error) => {
            log::warn!(target: "sessions", "sidebar settings unwatched in {}: {error}", dir.display());
        }
    }
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
        let (sessions, backfill) = query::list(conn, status)?;
        let loaded = Loaded {
            sessions,
            recent: query::recent(conn, paths::PLATFORM)?,
            aliases: query::aliases(conn)?,
            settings: owner_dir
                .map(|dir| settings::read(&dir, paths::PLATFORM))
                .unwrap_or_default(),
        };
        Ok::<_, rusqlite::Error>((loaded, backfill))
    });
    cx.spawn(async move |cx| {
        let rows = read.await;
        cx.update(|cx| {
            let store = cx.global_mut::<Sessions>();
            store.loading = false;
            match rows {
                Ok(Ok((loaded, backfill))) => {
                    log::info!(target: "sessions", "{} {status:?} tasks, {} recent projects, {} aliases, {} pins ordered, {} hidden projects",
                        loaded.sessions.len(), loaded.recent.len(), loaded.aliases.len(),
                        loaded.settings.pinned_order.len(), loaded.settings.hidden.len());
                    store.show(loaded);
                    if !backfill.is_empty() {
                        // The worker runs it; nothing waits for the reply.
                        drop(db.call(move |conn| projection::backfill(conn, &backfill)));
                    }
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

impl Sessions {
    /// A read's rows replace the list; an edit whose task left it ends, as its row unmounts.
    fn show(&mut self, loaded: Loaded) {
        if let Some(editing) = &self.editing
            && !loaded.sessions.iter().any(|s| s.id == editing.id)
        {
            log::info!(target: "sessions", "rename {} dropped: the task left the list", editing.id);
            self.editing = None;
        }
        let mut loaded = loaded;
        if let Some(current) = &self.loaded {
            self.pins.read(
                &mut loaded.settings.pinned_order,
                &current.settings.pinned_order,
            );
        }
        self.loaded = Some(loaded);
    }
}

pub use main_list::list;

#[cfg(test)]
mod menu_tests;
#[cfg(test)]
mod pin_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod view_tests;
