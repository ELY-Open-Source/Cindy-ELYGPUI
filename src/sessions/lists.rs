//! `CCAgentSidebarUpper.tsx`'s two lists from one bucket: the Pinned section and the main list.

use std::collections::HashMap;

use super::{
    group::{Entry, Order, Project, Session, entries, for_main_list, group},
    paths::{compare, grouping, session_key},
    settings::Settings,
};

const PROJECT_ENTRY: &str = "project:";

#[derive(Clone, Debug, PartialEq)]
pub enum Pinned {
    Session(Session),
    /// The project with its tasks not pinned on their own.
    Project(Project),
}

pub struct Lists {
    pub pinned: Vec<Pinned>,
    /// `buildSessionSourceLabelMap` for the pinned tasks.
    pub labels: HashMap<String, String>,
    pub main: Vec<Entry>,
}

/// `sidebarPinnedEntryComparisonKey`.
fn entry_identity(id: &str, platform: &str) -> String {
    match id.strip_prefix(PROJECT_ENTRY) {
        Some(key) => {
            compare(key, platform).map_or(id.to_owned(), |k| format!("{PROJECT_ENTRY}{k}"))
        }
        None => id.to_owned(),
    }
}

fn key_of(s: &Session) -> Option<String> {
    session_key(
        s.project_dir.as_deref().or(s.working_dir.as_deref()),
        s.remote_host.as_deref(),
    )
}

fn in_set(set: &[String], key: &str, platform: &str) -> bool {
    compare(key, platform).is_some_and(|k| set.contains(&k))
}

/// `buildSessionSourceLabelMap`: Chat, the project's name, else the folder's last segment.
fn labels(pinned: &[Session], known: &[Project], dialogues: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for s in pinned {
        if s.dialogue {
            if !dialogues.is_empty() {
                out.insert(s.id.clone(), dialogues.to_owned());
            }
            continue;
        }
        let named = key_of(s).and_then(|key| known.iter().find(|p| p.key == key));
        if let Some(project) = named {
            out.insert(s.id.clone(), project.name.clone());
            continue;
        }
        let Some(dir) = grouping(s.project_dir.as_deref().or(s.working_dir.as_deref())) else {
            continue;
        };
        let base = dir.rsplit(['/', '\\']).next().unwrap_or_default();
        if !base.is_empty() {
            out.insert(s.id.clone(), base.to_owned());
        }
    }
    out
}

/// Pins never follow the filter; hidden projects' tasks show as Chat tasks.
pub fn build(
    bucket: &[Session],
    recent: &[String],
    aliases: &HashMap<String, String>,
    settings: &Settings,
    platform: &str,
    order: &Order,
    dialogues_label: &str,
) -> Lists {
    let hidden: Vec<String> = settings
        .hidden
        .iter()
        .filter_map(|k| compare(k, platform))
        .collect();
    let pinned_projects: Vec<String> = settings
        .pinned_order
        .iter()
        .filter_map(|id| id.strip_prefix(PROJECT_ENTRY).filter(|k| !k.is_empty()))
        .filter_map(|k| compare(k, platform))
        .collect();
    let sidebar: Vec<Session> = bucket
        .iter()
        .map(|s| {
            let hide = !s.dialogue && key_of(s).is_some_and(|k| in_set(&hidden, &k, platform));
            Session {
                dialogue: s.dialogue || hide,
                ..s.clone()
            }
        })
        .collect();
    let plain = group(&sidebar, recent, aliases, platform, false);
    let with_pins = group(&sidebar, recent, aliases, platform, true);
    let universe = group(bucket, recent, aliases, platform, true).projects;
    let known: Vec<Project> = universe
        .into_iter()
        .filter(|p| !in_set(&hidden, &p.key, platform))
        .collect();
    let unpinned = |p: Project| Project {
        sessions: p
            .sessions
            .into_iter()
            .filter(|s| s.pinned_at.is_none())
            .collect(),
        ..p
    };
    let (mut pinned_side, mut main_projects) = (Vec::new(), Vec::new());
    for project in with_pins
        .projects
        .into_iter()
        .filter(|p| !in_set(&hidden, &p.key, platform))
    {
        if in_set(&pinned_projects, &project.key, platform) {
            let project = unpinned(project);
            let sessions = for_main_list(project.sessions, order.created);
            pinned_side.push(Pinned::Project(Project {
                sessions,
                ..project
            }));
        } else {
            main_projects.push(unpinned(project));
        }
    }
    let labels = labels(&plain.pinned, &known, dialogues_label);
    let mut pinned: Vec<Pinned> = plain.pinned.into_iter().map(Pinned::Session).collect();
    pinned.extend(pinned_side);
    if !settings.pinned_order.is_empty() {
        let mut rank: HashMap<String, usize> = HashMap::new();
        for (i, id) in settings.pinned_order.iter().enumerate() {
            rank.entry(entry_identity(id, platform)).or_insert(i);
        }
        let id = |p: &Pinned| match p {
            Pinned::Session(s) => s.id.clone(),
            Pinned::Project(p) => format!("{PROJECT_ENTRY}{}", p.key),
        };
        pinned.sort_by_key(|p| {
            rank.get(&entry_identity(&id(p), platform))
                .copied()
                .unwrap_or(usize::MAX)
        });
    }
    let main = entries(main_projects, plain.dialogues, plain.loose, order);
    Lists {
        pinned,
        labels,
        main,
    }
}
