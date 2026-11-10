//! `CCAgentSidebarUpper.tsx`'s two lists from one bucket: the Pinned section and the main list.

use std::collections::HashMap;

use crate::localdb::search::merge::COLLATOR;

use super::{
    group::{Entry, Order, Project, Recent, Session, entries, for_main_list, group},
    paths::{compare, grouping, session_key},
    settings::Settings,
};

pub(super) const PROJECT_ENTRY: &str = "project:";

#[derive(Clone, Debug, PartialEq)]
pub enum Pinned {
    Session(Box<Session>),
    /// The project with its tasks not pinned on their own.
    Project(Project),
}

impl Pinned {
    /// The entry's `pinnedOrder` id: the task's, or `project:` and the project key.
    pub fn id(&self) -> String {
        match self {
            Pinned::Session(s) => s.id.clone(),
            Pinned::Project(p) => format!("{PROJECT_ENTRY}{}", p.key),
        }
    }
}

pub struct Lists {
    pub pinned: Vec<Pinned>,
    /// `buildSessionSourceLabelMap` for the pinned tasks.
    pub labels: HashMap<String, String>,
    pub main: Vec<Entry>,
    /// The dialogues the main list draws, grouped or loose, for the rail's panel.
    pub dialogues: Vec<Session>,
    /// The rail's Projects panel: its drafts, then every project, pinned ones too.
    pub unclassified: Vec<Session>,
    pub rail_projects: Vec<Project>,
    /// `dialogueSourceLabelMap` for the main list's loose tasks.
    pub main_labels: HashMap<String, String>,
    /// `dialogueActionSessions`' ids: every dialogue, pinned ones too, for the group's menu.
    pub dialogue_actions: Vec<String>,
    /// `groupsWithPinnedProjects`' keys: the projects a project link can focus.
    pub focusable: Vec<String>,
}

/// `sidebarPinnedEntryComparisonKey`.
pub(super) fn entry_identity(id: &str, platform: &str) -> String {
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
pub(super) fn labels(
    pinned: &[Session],
    known: &[Project],
    dialogues: &str,
) -> HashMap<String, String> {
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
    recent: &[Recent],
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
    // Pins and labels see the whole bucket; the main list sees the last-activity window.
    let plain = group(&sidebar, recent, aliases, platform, false);
    let catalogue = group(&sidebar, recent, aliases, platform, true);
    let dialogue_actions = catalogue.dialogues.iter().map(|s| s.id.clone()).collect();
    let all_projects = catalogue.projects;
    let recent_main: &[Recent] = if order.cutoff.is_some() { &[] } else { recent };
    let windowed: Vec<Session> = sidebar
        .iter()
        .filter(|s| order.cutoff.is_none_or(|cutoff| s.activity_at >= cutoff))
        .cloned()
        .collect();
    let main = group(&windowed, recent_main, aliases, platform, false);
    let main_with_pins = group(&windowed, recent_main, aliases, platform, true).projects;
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
    let shown = |p: &Project| !in_set(&hidden, &p.key, platform);
    let pinned_side: Vec<Pinned> = all_projects
        .into_iter()
        .filter(|p| shown(p) && in_set(&pinned_projects, &p.key, platform))
        .map(|p| {
            let p = unpinned(p);
            let sessions = for_main_list(p.sessions, order.created);
            Pinned::Project(Project { sessions, ..p })
        })
        .collect();
    let matches = |s: &Session| order.vendor.is_none_or(|v| s.agent_kind == v);
    let narrowed = |p: Project| {
        let sessions: Vec<Session> = p.sessions.iter().filter(|s| matches(s)).cloned().collect();
        // `persistentProjectMatchesVendor`: no known kind matches any harness.
        let kept = match (&p.persistent, order.vendor) {
            _ if !sessions.is_empty() => true,
            (None, _) => false,
            (Some(_), None) => true,
            (Some(kinds), Some(v)) => kinds.is_empty() || kinds.iter().any(|k| k == v),
        };
        kept.then(|| unpinned(Project { sessions, ..p }))
    };
    let visible: Vec<Project> = main_with_pins.into_iter().filter(shown).collect();
    let focusable = visible.iter().map(|p| p.key.clone()).collect();
    let rail_projects = for_sidebar(
        visible.iter().cloned().filter_map(narrowed).collect(),
        order,
    );
    let main_projects: Vec<Project> = visible
        .into_iter()
        .filter(|p| !in_set(&pinned_projects, &p.key, platform))
        .filter_map(narrowed)
        .collect();
    let pin_labels = labels(&plain.pinned, &known, dialogues_label);
    let mut pinned: Vec<Pinned> = plain
        .pinned
        .into_iter()
        .map(|s| Pinned::Session(Box::new(s)))
        .collect();
    pinned.extend(pinned_side);
    if !settings.pinned_order.is_empty() {
        let mut rank: HashMap<String, usize> = HashMap::new();
        for (i, id) in settings.pinned_order.iter().enumerate() {
            rank.entry(entry_identity(id, platform)).or_insert(i);
        }
        pinned.sort_by_key(|p| {
            rank.get(&entry_identity(&p.id(), platform))
                .copied()
                .unwrap_or(usize::MAX)
        });
    }
    let pick = |list: Vec<Session>| list.into_iter().filter(|s| matches(s)).collect::<Vec<_>>();
    let dialogues = pick(main.dialogues);
    // Flat mode labels project tasks too; drafts never.
    let labelled: Vec<Session> = if order.flat {
        let projects = main_projects
            .iter()
            .flat_map(|p| p.sessions.iter().cloned());
        projects.chain(dialogues.iter().cloned()).collect()
    } else {
        dialogues.clone()
    };
    let main_labels = labels(&labelled, &known, dialogues_label);
    let loose = pick(main.loose);
    let unclassified = sessions_for_sidebar(loose.clone(), order.created);
    let main = entries(main_projects, dialogues.clone(), loose, order);
    Lists {
        pinned,
        labels: pin_labels,
        main,
        dialogues,
        unclassified,
        rail_projects,
        main_labels,
        dialogue_actions,
        focusable,
    }
}

/// `sortSessionsForSidebar`; `priority` waits for the agent runtime (10.0).
fn sessions_for_sidebar(mut sessions: Vec<Session>, created: bool) -> Vec<Session> {
    if created {
        sessions.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| COLLATOR.compare(&a.id, &b.id))
        });
    }
    sessions
}

/// `sortProjectsForSidebar`; the manual order comes with 13.0, as the main list's does.
fn for_sidebar(projects: Vec<Project>, order: &Order) -> Vec<Project> {
    let mut projects: Vec<Project> = projects
        .into_iter()
        .map(|p| Project {
            sessions: sessions_for_sidebar(p.sessions, order.created),
            ..p
        })
        .collect();
    if !order.custom && order.created {
        let newest = |p: &Project| {
            p.sessions
                .iter()
                .map(|s| s.created_at)
                .max()
                .unwrap_or(0)
                .max(0)
        };
        let first = |p: &Project| p.sessions.first().map_or(String::new(), |s| s.id.clone());
        projects.sort_by(|a, b| {
            newest(b)
                .cmp(&newest(a))
                .then_with(|| COLLATOR.compare(&first(a), &first(b)))
        });
    }
    projects
}
