//! `projectGrouping.ts`'s `groupSessions` and `mainListModel.ts`'s `buildMainListEntries`,
//! project mode: projects, the dialogue group and loose tasks in one ranking.

use std::collections::HashMap;

use super::paths::{comparison_key, grouping, identity_key, normalize_key};

/// What grouping reads of a task.
#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub status: String,
    pub source: String,
    pub dialogue: bool,
    pub working_dir: Option<String>,
    pub project_dir: Option<String>,
    pub remote_host: Option<String>,
    pub user_send_at: Option<i64>,
    pub pinned_at: Option<i64>,
    pub messages: i64,
    pub created_at: i64,
    /// `getSessionListActivityAt`, in ms.
    pub activity_at: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Project {
    pub key: String,
    pub name: String,
    pub sessions: Vec<Session>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    Project(Project),
    Dialogues(Vec<Session>),
    Loose(Session),
}

/// `FilterSortBy` and `FilterProjectOrder`, plus the dialogue switch.
pub struct Order<'a> {
    pub created: bool,
    pub custom: bool,
    pub manual: &'a [String],
    pub group_dialogue: bool,
}

fn status_rank(s: &Session) -> u8 {
    match s.status.as_str() {
        "active" => 0,
        "archived" => 1,
        _ => 2,
    }
}

/// `compareSessionsByStatusThenSortTimeDesc`.
fn by_status_then_time(list: &mut [Session]) {
    list.sort_by(|a, b| {
        status_rank(a)
            .cmp(&status_rank(b))
            .then(b.activity_at.cmp(&a.activity_at))
    });
}

/// `sortSessionsForMainList`: created (then id) or activity, ties in list order.
pub fn for_main_list(mut list: Vec<Session>, created: bool) -> Vec<Session> {
    if created {
        list.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| a.id.cmp(&b.id))
        });
    } else {
        list.sort_by_key(|s| std::cmp::Reverse(s.activity_at));
    }
    list
}

fn split(dir: &str) -> Vec<&str> {
    dir.split('/').filter(|s| !s.is_empty()).collect()
}

fn tail(parts: &[&str], n: usize) -> String {
    parts[parts.len().saturating_sub(n)..].join("/")
}

/// Display names: an alias, the oldest project's bare basename, or enough parent segments.
fn names(
    dirs: &[(String, String, i64)],
    aliases: &HashMap<String, String>,
    platform: &str,
) -> Vec<String> {
    let mut by_base: Vec<(String, Vec<usize>)> = Vec::new();
    for (i, (_, dir, _)) in dirs.iter().enumerate() {
        let base = split(dir).last().map_or(dir.clone(), |b| (*b).to_owned());
        match by_base.iter_mut().find(|(b, _)| *b == base) {
            Some((_, members)) => members.push(i),
            None => by_base.push((base, vec![i])),
        }
    }
    let mut min_seg = vec![1usize; dirs.len()];
    let mut winner = vec![false; dirs.len()];
    for (_, members) in &by_base {
        if members.len() == 1 {
            continue;
        }
        let mut ranked = members.clone();
        ranked.sort_by(|&a, &b| {
            dirs[a]
                .2
                .cmp(&dirs[b].2)
                .then_with(|| dirs[a].0.cmp(&dirs[b].0))
        });
        winner[ranked[0]] = true;
        for &later in &ranked[1..] {
            min_seg[later] = 2;
        }
    }
    let max_segments = dirs
        .iter()
        .map(|(_, d, _)| split(d).len())
        .max()
        .unwrap_or(1)
        .max(1);
    let mut tails: HashMap<(usize, String), i64> = HashMap::new();
    let mut same: HashMap<&str, i64> = HashMap::new();
    for (_, dir, _) in dirs {
        *same.entry(dir).or_default() += 1;
        let parts = split(dir);
        for n in 1..=max_segments {
            *tails.entry((n, tail(&parts, n))).or_default() += 1;
        }
    }
    dirs.iter()
        .enumerate()
        .map(|(i, (key, dir, _))| {
            if let Some(alias) = aliases.get(&comparison_key(key, platform)) {
                return alias.clone();
            }
            let parts = split(dir);
            let own = parts.last().map_or(dir.clone(), |b| (*b).to_owned());
            if winner[i] {
                return own;
            }
            let top = max_segments.max(min_seg[i]);
            for n in min_seg[i]..=top {
                let mine = tail(&parts, n);
                if tails.get(&(n, mine.clone())).copied().unwrap_or(0) - same[dir.as_str()] <= 0 {
                    return if mine.is_empty() { own } else { mine };
                }
            }
            let fallback = tail(&parts, top);
            if fallback.is_empty() { own } else { fallback }
        })
        .collect()
}

/// `normalizeProjectAliases`: newest first, one per comparison key.
pub fn aliases(rows: &[(String, String)], platform: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for (key, alias) in rows {
        let (Some(key), alias) = (normalize_key(key), alias.trim()) else {
            continue;
        };
        if !alias.is_empty() {
            out.entry(comparison_key(&key, platform))
                .or_insert_with(|| alias.to_owned());
        }
    }
    out
}

/// `groupSessions` without pins (`recent` = `recent_workdirs` paths, seeded first), then `buildMainListEntries` in project mode.
pub fn entries(
    sessions: &[Session],
    recent: &[String],
    aliases: &HashMap<String, String>,
    platform: &str,
    order: &Order,
) -> Vec<Entry> {
    let mut representative: HashMap<String, (String, String)> = HashMap::new();
    for project in recent {
        let Some(dir) = grouping(Some(project)) else {
            continue;
        };
        let key = format!("local:{dir}");
        representative
            .entry(comparison_key(&key, platform))
            .or_insert((key, dir));
    }
    let (mut dialogues, mut loose) = (Vec::new(), Vec::new());
    let mut groups: Vec<(String, String, Vec<Session>)> = Vec::new();
    for s in sessions.iter().filter(|s| s.pinned_at.is_none()) {
        if s.dialogue {
            dialogues.push(s.clone());
            continue;
        }
        let dir = grouping(s.project_dir.as_deref().or(s.working_dir.as_deref()));
        let placed = matches!(s.source.as_str(), "scheduler" | "plugin");
        let draft = !placed && s.user_send_at.is_none() && s.messages == 0;
        let Some(dir) = dir.filter(|_| !draft) else {
            loose.push(s.clone());
            continue;
        };
        let mut key = identity_key(&dir, s.remote_host.as_deref());
        let mut shown = dir.clone();
        if s.remote_host.is_none() {
            let compare = comparison_key(&key, platform);
            match representative.get(&compare) {
                Some((k, d)) => (key, shown) = (k.clone(), d.clone()),
                None => {
                    representative.insert(compare, (key.clone(), dir));
                }
            }
        }
        match groups.iter_mut().find(|(k, ..)| *k == key) {
            Some((.., list)) => list.push(s.clone()),
            None => groups.push((key, shown, vec![s.clone()])),
        }
    }
    for project in recent {
        let Some(dir) = grouping(Some(project)) else {
            continue;
        };
        let seed = format!("local:{dir}");
        let compare = comparison_key(&seed, platform);
        let known = groups
            .iter()
            .find(|(k, ..)| comparison_key(k, platform) == compare);
        if known.is_none() {
            groups.push((seed, dir, Vec::new()));
        }
    }
    let earliest = |list: &[Session]| {
        list.iter()
            .map(|s| s.created_at)
            .filter(|t| *t > 0)
            .min()
            .unwrap_or(i64::MAX)
    };
    let dirs: Vec<(String, String, i64)> = groups
        .iter()
        .map(|(k, d, l)| (k.clone(), d.clone(), earliest(l)))
        .collect();
    let names = names(&dirs, aliases, platform);
    let mut out: Vec<Entry> = groups
        .into_iter()
        .zip(names)
        .map(|((key, _, mut list), name)| {
            by_status_then_time(&mut list);
            Entry::Project(Project {
                key,
                name,
                sessions: for_main_list(list, order.created),
            })
        })
        .collect();
    by_status_then_time(&mut dialogues);
    by_status_then_time(&mut loose);
    if order.group_dialogue {
        if !dialogues.is_empty() {
            out.push(Entry::Dialogues(for_main_list(dialogues, order.created)));
        }
    } else {
        out.extend(
            for_main_list(dialogues, order.created)
                .into_iter()
                .map(Entry::Loose),
        );
    }
    out.extend(
        for_main_list(loose, order.created)
            .into_iter()
            .map(Entry::Loose),
    );
    rank(out, order)
}

pub fn entry_sessions(entry: &Entry) -> &[Session] {
    match entry {
        Entry::Project(p) => &p.sessions,
        Entry::Dialogues(list) => list,
        Entry::Loose(s) => std::slice::from_ref(s),
    }
}

/// `compareEntriesBySortBy` without `priority`, which waits for the agent runtime (10.0).
fn compare(a: &Entry, b: &Entry, order: &Order) -> std::cmp::Ordering {
    let time = |e: &Entry| {
        let pick = |s: &Session| {
            if order.created {
                s.created_at
            } else {
                s.activity_at
            }
        };
        entry_sessions(e)
            .iter()
            .map(pick)
            .filter(|t| *t > 0)
            .max()
            .unwrap_or(0)
    };
    let by_time = time(b).cmp(&time(a));
    if by_time.is_ne() || !order.created {
        return by_time;
    }
    let first = |e: &Entry| {
        entry_sessions(e)
            .first()
            .map(|s| s.id.clone())
            .unwrap_or_default()
    };
    first(a).cmp(&first(b))
}

/// `sortMainListEntries`: custom order puts projects first in the manual order.
fn rank(mut entries: Vec<Entry>, order: &Order) -> Vec<Entry> {
    if !order.custom {
        entries.sort_by(|a, b| compare(a, b, order));
        return entries;
    }
    let keys: Vec<String> = entries
        .iter()
        .filter_map(|e| {
            if let Entry::Project(p) = e {
                Some(p.key.clone())
            } else {
                None
            }
        })
        .collect();
    let mut manual: Vec<String> = Vec::new();
    for raw in order.manual {
        let Some(key) = normalize_key(raw) else {
            continue;
        };
        if keys.contains(&key) && !manual.contains(&key) {
            manual.push(key);
        }
    }
    manual.extend(
        keys.iter()
            .filter(|k| {
                !order
                    .manual
                    .iter()
                    .any(|m| normalize_key(m).as_ref() == Some(k))
            })
            .cloned(),
    );
    let position = |e: &Entry| match e {
        Entry::Project(p) => manual.iter().position(|k| *k == p.key),
        _ => None,
    };
    entries.sort_by(|a, b| match (position(a), position(b)) {
        (Some(x), Some(y)) => x.cmp(&y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => compare(a, b, order),
    });
    entries
}

#[cfg(test)]
mod tests;
