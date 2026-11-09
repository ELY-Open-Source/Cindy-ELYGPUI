//! `handlePinnedReorder`: a drag's visible order merged into `pinnedOrder` and saved.

use gpui::{App, Window};

use super::{
    Sessions, group,
    lists::{PROJECT_ENTRY, entry_identity},
    paths::PLATFORM,
    pin, settings,
};

/// `normalizeManualPinnedOrder`: entries no longer active drop out, new ones go last.
fn normalize(prev: &[String], active: &[String], key: &dyn Fn(&str) -> String) -> Vec<String> {
    let active_keys: Vec<String> = active.iter().map(|e| key(e)).collect();
    let (mut seen, mut next) = (Vec::new(), Vec::new());
    for id in prev.iter().chain(active) {
        let identity = key(id);
        if !active_keys.contains(&identity) || seen.contains(&identity) {
            continue;
        }
        seen.push(identity);
        next.push(id.clone());
    }
    next
}

/// `mergeVisibleReorder`: the visible slots take the new order; hidden entries stay put.
fn merge_visible(full: &[String], visible: &[String], key: &dyn Fn(&str) -> String) -> Vec<String> {
    let mut stored: Vec<(String, &String)> = Vec::new();
    for id in full {
        let identity = key(id);
        if !stored.iter().any(|(k, _)| *k == identity) {
            stored.push((identity, id));
        }
    }
    let shown: Vec<String> = visible.iter().map(|e| key(e)).collect();
    let mut queue: std::collections::VecDeque<String> = visible
        .iter()
        .map(|id| {
            let identity = key(id);
            stored
                .iter()
                .find(|(k, _)| *k == identity)
                .map_or(id.clone(), |(_, e)| (*e).clone())
        })
        .collect();
    let mut out: Vec<String> = full
        .iter()
        .map(|id| match shown.contains(&key(id)) {
            true => queue.pop_front().unwrap_or_else(|| id.clone()),
            false => id.clone(),
        })
        .collect();
    out.extend(queue);
    out
}

/// `buildManualPinnedReorder`: the drag's base and its merged order.
pub(super) fn build(
    ranked: &[String],
    active: &[String],
    visible: &[String],
    key: &dyn Fn(&str) -> String,
) -> (Vec<String>, Vec<String>) {
    let display = normalize(ranked, active, key);
    let ranked_keys: Vec<String> = ranked.iter().map(|e| key(e)).collect();
    let base = display
        .iter()
        .filter(|e| ranked_keys.contains(&key(e)))
        .cloned()
        .collect();
    (base, merge_visible(&display, visible, key))
}

/// `activePinnedSidebarEntryIds`: pinned tasks, then the order's project pins, one per identity.
pub(super) fn active(
    manual: &[String],
    pinned: &[String],
    key: &dyn Fn(&str) -> String,
) -> Vec<String> {
    let projects = manual.iter().filter(|e| {
        e.strip_prefix(PROJECT_ENTRY)
            .is_some_and(|rest| !rest.is_empty())
    });
    let (mut seen, mut out) = (Vec::new(), Vec::new());
    for id in pinned.iter().chain(projects) {
        let identity = key(id);
        if !seen.contains(&identity) {
            seen.push(identity);
            out.push(id.clone());
        }
    }
    out
}

/// `handlePinnedReorder` with `setManualPinnedOrder`: `visible` is the shown entries' new order.
pub(super) fn reorder(visible: &[String], window: &Window, cx: &mut App) {
    let Some(loaded) = cx.global::<Sessions>().loaded.as_ref() else {
        log::warn!(target: "sessions", "pinned reorder: no list loaded");
        return;
    };
    let key = |e: &str| entry_identity(e, PLATFORM);
    let pinned: Vec<String> = group::pinned_in_display_order(&loaded.sessions)
        .into_iter()
        .map(|s| s.id)
        .collect();
    let manual = &loaded.settings.pinned_order;
    let active = active(manual, &pinned, &key);
    let (base, order) = build(manual, &active, visible, &key);
    let next = normalize(&order, &active, &key);
    log::info!(target: "sessions", "pinned reorder: {} shown, {} saved", visible.len(), next.len());
    let mutation = settings::Mutation::Reorder {
        base,
        order: next.clone(),
    };
    pin::persist(next, mutation, window, cx);
}

#[cfg(test)]
mod tests;
