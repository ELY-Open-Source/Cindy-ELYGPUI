//! `requestConversationSearch` and the lock `useConversationSearch` takes from it.

use gpui::{App, Window};

use super::{Search, Which, field, of, of_mut, reset, run, scroll};
use crate::sessions::{
    Sessions,
    paths::{PLATFORM, compare},
};

/// The scope a project's or the dialogues' menu asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum Lock {
    Project {
        key: String,
        name: String,
        ids: Vec<String>,
    },
    /// `lockedSessionGroup`: its ids sorted, as an explicit snapshot.
    Group { name: String, ids: Vec<String> },
}

/// `requestConversationSearch`: the inline search takes it, and the rail's while it shows.
pub fn request(lock: Lock, window: &mut Window, cx: &mut App) {
    log::info!(target: "search", "search requested in {}", match &lock {
        Lock::Project { key, .. } => key.as_str(),
        Lock::Group { .. } => "a task group",
    });
    let rail = cx.global::<Search>().rail_shown;
    take(Which::Inline, lock.clone(), cx);
    scroll::freeze(cx);
    let input = field(Which::Inline, window, cx).read(cx).focus().clone();
    window.defer(cx, move |window, cx| window.focus(&input, cx));
    if rail {
        take(Which::Rail, lock, cx);
        crate::sessions::rail::search::open(window, cx);
    }
    cx.refresh_windows();
}

/// The lock effect: the lock, its project as the selection, and an empty query.
fn take(w: Which, lock: Lock, cx: &mut App) {
    let lock = match lock {
        Lock::Project { key, name, ids } => Lock::Project {
            key,
            name,
            ids: unique(ids),
        },
        Lock::Group { name, ids } => {
            let mut ids = unique(ids);
            ids.sort();
            Lock::Group { name, ids }
        }
    };
    let o = &mut of_mut(w, cx).options;
    o.projects = match &lock {
        Lock::Project { key, .. } => Some(vec![key.clone()]),
        Lock::Group { .. } => None,
    };
    o.lock = Some(lock);
    reset(w, cx);
    released(w, cx);
}

/// `[...new Set(ids)]`: first spellings, in order.
fn unique(ids: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(ids.len());
    for id in ids {
        if !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

/// `clearLock`: back to the global search.
pub fn clear(w: Which, cx: &mut App) {
    let o = &mut of_mut(w, cx).options;
    if o.lock.take().is_none() {
        return;
    }
    o.projects = None;
    log::info!(target: "search", "{w:?} lock cleared");
    run::sync(w, cx);
    cx.refresh_windows();
}

/// The provider's release: a locked project hidden or gone from the known ones lets go.
pub fn released(w: Which, cx: &mut App) -> bool {
    let Some(Lock::Project { key, .. }) = &of(w, cx).options.lock else {
        return false;
    };
    let Some(loaded) = cx.global::<Sessions>().loaded.as_ref() else {
        return false;
    };
    let target = compare(key, PLATFORM);
    let same = |k: &str| target.is_some() && compare(k, PLATFORM) == target;
    let hidden = loaded.settings.hidden.iter().any(|k| same(k));
    // An empty catalogue keeps the lock: the menu's ids still apply.
    let gone = !loaded.known.is_empty() && !loaded.known.iter().any(|p| same(&p.key));
    if !(hidden || gone) {
        return false;
    }
    log::info!(target: "search", "{w:?} lock on {key} released: {}", if hidden { "hidden" } else { "not shown" });
    reset(w, cx);
    clear(w, cx);
    true
}

/// The lock's label: the group's name, else the known project's, else the name it was asked with.
pub fn label(lock: &Lock, exact: bool, cx: &App) -> String {
    match lock {
        Lock::Group { name, .. } => name.clone(),
        Lock::Project { key, name, .. } => {
            let target = compare(key, PLATFORM);
            super::projects::known_of(cx)
                .iter()
                .find(|p| match exact {
                    true => &p.key == key,
                    false => target.is_some() && compare(&p.key, PLATFORM) == target,
                })
                .map_or_else(|| name.clone(), |p| p.name.clone())
        }
    }
}

#[cfg(test)]
mod tests;
