//! The options' Projects filter and its selection.

use gpui::App;

use super::{
    Options, Which,
    lock::{self, Lock},
    of, of_mut, run,
};
use crate::sessions::{
    Sessions,
    group::{self, Project, Recent, Session},
    paths::{PLATFORM, compare},
};

/// `visibleSidebarProjects` over every status's projects, Orca workers left out.
pub fn known(
    all: &[Session],
    recent: &[Recent],
    aliases: &[(String, String)],
    hidden: &[String],
) -> Vec<Project> {
    let hidden: Vec<String> = hidden.iter().filter_map(|k| compare(k, PLATFORM)).collect();
    let tasks: Vec<Session> = all.iter().filter(|s| !s.orca_worker).cloned().collect();
    let aliases = group::aliases(aliases, PLATFORM);
    group::group(&tasks, recent, &aliases, PLATFORM, false)
        .projects
        .into_iter()
        .filter(|p| compare(&p.key, PLATFORM).is_none_or(|k| !hidden.contains(&k)))
        .collect()
}

/// The known projects of the last read; none before it.
pub(super) fn known_of(cx: &App) -> &[Project] {
    cx.global::<Sessions>()
        .loaded
        .as_ref()
        .map_or(&[], |l| &l.known)
}

/// `nextProjectSelection`; none is `all`.
pub(super) fn next(prev: Option<&[String]>, key: &str) -> Option<Vec<String>> {
    let Some(prev) = prev else {
        return Some(vec![key.to_owned()]);
    };
    let Some(target) = compare(key, PLATFORM) else {
        return Some(prev.to_vec());
    };
    let same = |k: &String| compare(k, PLATFORM).as_ref() == Some(&target);
    if !prev.iter().any(same) {
        return Some(prev.iter().cloned().chain([key.to_owned()]).collect());
    }
    let rest: Vec<String> = prev.iter().filter(|k| !same(k)).cloned().collect();
    (!rest.is_empty()).then_some(rest)
}

/// Whether `key` is in the selection, by comparison key.
pub(super) fn selected(selection: Option<&[String]>, key: &str) -> bool {
    let target = compare(key, PLATFORM);
    selection
        .is_some_and(|keys| target.is_some() && keys.iter().any(|k| compare(k, PLATFORM) == target))
}

/// `reconcileProjectSelectionWithVisibleProjects`: each selected key as a known project's own key.
fn reconcile(selection: &[String], known: &[Project]) -> Vec<String> {
    let mut next: Vec<String> = Vec::new();
    for key in selection {
        let target = compare(key, PLATFORM);
        let current = known
            .iter()
            .rev()
            .find(|p| target.is_some() && compare(&p.key, PLATFORM) == target);
        if let Some(p) = current
            && !next.contains(&p.key)
        {
            next.push(p.key.clone());
        }
    }
    next
}

/// `selectedProjectSessionIds`: the group's ids, else the selected projects' (the lock's own as a fallback).
pub(super) fn scope(o: &Options, cx: &App) -> Option<Vec<String>> {
    if let Some(Lock::Group { ids, .. }) = &o.lock {
        return Some(ids.clone());
    }
    let selection = o.projects.as_deref()?;
    let mut ids: Vec<String> = known_of(cx)
        .iter()
        .filter(|p| selected(Some(selection), &p.key))
        .flat_map(|p| p.sessions.iter().map(|s| s.id.clone()))
        .collect();
    let mut chosen: Vec<String> = selection
        .iter()
        .filter_map(|k| compare(k, PLATFORM))
        .collect();
    chosen.sort();
    chosen.dedup();
    if let Some(Lock::Project {
        key, ids: locked, ..
    }) = &o.lock
        && ids.is_empty()
        && chosen.len() == 1
        && selected(Some(selection), key)
    {
        ids = locked.clone();
    }
    ids.sort();
    ids.dedup();
    Some(ids)
}

/// A read landed: a lock may let go, a selection keeps known projects; each search follows its scope.
pub fn loaded(cx: &mut App) {
    for w in [Which::Inline, Which::Rail] {
        if of(w, cx).options.lock.is_some() {
            lock::released(w, cx);
        } else if let Some(selection) = of(w, cx).options.projects.clone() {
            let next = reconcile(&selection, known_of(cx));
            if next != selection {
                log::info!(target: "search", "{w:?} project filter {} -> {} projects", selection.len(), next.len());
                of_mut(w, cx).options.projects = (!next.is_empty()).then_some(next);
            }
        }
        run::sync(w, cx);
    }
}

#[cfg(test)]
mod tests;
