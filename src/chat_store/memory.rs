//! The store's memory: the LRU cap, the trim of left windows and the soft eviction.

use std::{rc::Rc, time::Duration};

use super::{SessionState, Store, window::Island};
use crate::{
    chat_message::merge::{Message, get},
    history_view::controller::Message as _,
    rich_answer::js::Js,
};

/// `MAX_CACHED_SESSIONS`.
const MAX_CACHED: usize = 20;
/// `TRIM_THRESHOLD` and `TRIM_TARGET`.
const TRIM_THRESHOLD: usize = 300;
const TRIM_TARGET: usize = 200;
/// `SOFT_EVICTION_CHECK_INTERVAL_MS`.
const CHECK_INTERVAL: Duration = Duration::from_secs(30);

/// `SOFT_EVICTION_DEFAULT_BUDGET`.
#[derive(Clone, Copy)]
pub struct Budget {
    pub messages: usize,
    pub characters: usize,
}

impl Default for Budget {
    fn default() -> Self {
        Budget {
            messages: 4000,
            characters: 32 * 1024 * 1024,
        }
    }
}

fn client(m: &Message) -> Option<&str> {
    super::window::text(m, "clientId")
}

/// `pruneIslandsForTrimmedWindow`.
fn prune(islands: &[Island], retained: &[Message]) -> Vec<Island> {
    let has = |id: &str| retained.iter().any(|m| client(m) == Some(id));
    let fallback = retained.first().and_then(client);
    islands
        .iter()
        .filter(|island| has(&island.newest_client_id))
        .map(|island| Island {
            oldest_client_id: match fallback {
                Some(oldest) if !has(&island.oldest_client_id) => oldest.into(),
                _ => island.oldest_client_id.clone(),
            },
            newest_client_id: island.newest_client_id.clone(),
        })
        .collect()
}

/// `retainedWindowKeepsGapCursor`: the cursor names a kept row past the first.
fn keeps_gap_cursor(retained: &[Message], cursor: Option<&str>) -> bool {
    let Some(cursor) = cursor.filter(|c| !c.is_empty()) else {
        return false;
    };
    let named =
        |m: &Message| client(m) == Some(cursor) || super::window::text(m, "id") == Some(cursor);
    retained.iter().position(named).is_some_and(|i| i > 0)
}

impl Store {
    /// `getOrCreateState`'s bookkeeping: a new slice may push the oldest idle one out.
    pub(super) fn admit(&self, id: &str) {
        let created = {
            let mut inner = self.0.borrow_mut();
            let created = !inner.sessions.contains_key(id);
            if created {
                inner
                    .sessions
                    .insert(id.to_owned(), SessionState::default());
            }
            // `_touchSession`.
            inner.access.retain(|a| a != id);
            inner.access.push(id.to_owned());
            created
        };
        if created {
            self.evict_lru();
        }
    }

    /// `_evictLruIfNeeded`; the runtime's busy guard comes with 10.0.
    fn evict_lru(&self) {
        loop {
            let candidate = {
                let inner = self.0.borrow();
                if inner.sessions.len() <= MAX_CACHED {
                    return;
                }
                let idle = |id: &&String| {
                    inner.sessions.contains_key(*id) && !inner.active.contains_key(*id)
                };
                inner.access.iter().find(idle).cloned()
            };
            let Some(id) = candidate else { return };
            log::info!(target: "chatStore", "evicted {id} past the cache of {MAX_CACHED}");
            self.purge(&id);
        }
    }

    /// `releaseRemoteHistoryView`: the view is detached before it goes inactive.
    pub(super) fn release_view(&self, id: &str) {
        let view = {
            let mut inner = self.0.borrow_mut();
            let view = inner.views.get_mut(id).and_then(|e| e.view.take());
            if view.is_some() {
                inner.views.remove(id);
            }
            view
        };
        if let Some(view) = view {
            view.set_active(false);
        }
    }

    /// `purgeSession`, for the parts the store holds so far.
    pub fn purge(&self, id: &str) {
        self.release_view(id);
        self.invalidate_window(id);
        let mut inner = self.0.borrow_mut();
        inner.sessions.shift_remove(id);
        inner.in_flight.remove(id);
        inner.fetch_tokens.remove(id);
        inner.last_viewed.remove(id);
        inner.access.retain(|a| a != id);
    }

    /// `_trimMessagesIfNeeded`; warm reading windows need 9.14's scroll memory, so none is kept.
    pub(super) fn trim_messages(&self) {
        let ids: Vec<String> = self.0.borrow().sessions.keys().cloned().collect();
        for id in ids {
            self.trim_session(&id);
        }
    }

    /// `_trimSessionMessages`: the newest 200 rows of a left window over 300.
    fn trim_session(&self, id: &str) {
        let Some(state) = self.state(id) else { return };
        if state.messages.len() <= TRIM_THRESHOLD || self.0.borrow().active.contains_key(id) {
            return;
        }
        *self.0.borrow_mut().epochs.entry(id.to_owned()).or_default() += 1;
        let retained = state.messages[state.messages.len() - TRIM_TARGET..].to_vec();
        let keep = !state.history_window_islands.is_empty()
            && keeps_gap_cursor(&retained, state.oldest_message_id.as_deref());
        log::info!(target: "chatStore", "trimmed {id} from {} rows", state.messages.len());
        self.set(id, |s| {
            Some(SessionState {
                history_window_islands: Rc::new(prune(&s.history_window_islands, &retained)),
                messages: Rc::new(retained),
                has_more_messages: true,
                oldest_message_id: keep.then(|| s.oldest_message_id.clone()).flatten(),
                is_loading_more: false,
                ..s.clone()
            })
        });
    }

    /// `_isSoftEvictionCandidate`, less the runtime's busy and queue checks of 10.0.
    fn evictable(&self, id: &str, state: &SessionState) -> bool {
        let inner = self.0.borrow();
        inner.last_viewed.contains_key(id)
            && !inner.active.contains_key(id)
            && !state.messages.is_empty()
            && !state
                .messages
                .iter()
                .any(|m| get(m, "isPendingPersist") == Some(&Js::Bool(true)))
    }

    /// `_softEvictIdleSessions`: the least recently left windows go until the cache fits.
    pub(super) fn soft_evict(&self) {
        let now = self.0.borrow().background.now();
        let budget = self.0.borrow().budget;
        let sessions: Vec<(String, SessionState)> = self
            .0
            .borrow()
            .sessions
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let (mut messages, mut characters) = (0, 0);
        let mut candidates = Vec::new();
        for (id, state) in sessions {
            let size: usize = state.messages.iter().map(|m| m.characters()).sum();
            messages += state.messages.len();
            characters += size;
            if !self.evictable(&id, &state) {
                continue;
            }
            let left = self.0.borrow().last_viewed.get(&id).copied().unwrap_or(now);
            candidates.push((id, left, state.messages.len(), size));
        }
        let within = |m: usize, c: usize| m <= budget.messages && c <= budget.characters;
        if within(messages, characters) {
            return;
        }
        candidates.sort_by_key(|c| c.1);
        for (id, _, count, size) in candidates {
            if within(messages, characters) {
                break;
            }
            log::info!(target: "chatStore", "soft evicted {id}: {count} rows, {size} characters");
            self.soft_evict_session(&id);
            messages -= count;
            characters -= size;
        }
    }

    /// `_softEvictSession`: the window and its view go; a return reads it again.
    fn soft_evict_session(&self, id: &str) {
        self.release_view(id);
        self.invalidate_window(id);
        self.set(id, |_| {
            Some(SessionState {
                messages: Rc::new(Vec::new()),
                history_loaded: false,
                oldest_message_id: None,
                has_more_messages: true,
                is_loading_more: false,
                history_window_islands: Rc::new(Vec::new()),
            })
        });
    }

    /// `_ensureSoftEvictionTimer`: one 30s check for the life of the store.
    pub(super) fn ensure_timer(&self) {
        let (foreground, background) = {
            let inner = self.0.borrow();
            if inner.timer.is_some() {
                return;
            }
            (inner.foreground.clone(), inner.background.clone())
        };
        let weak = Rc::downgrade(&self.0);
        let task = foreground.spawn(async move {
            loop {
                background.timer(CHECK_INTERVAL).await;
                let Some(inner) = weak.upgrade() else { return };
                Store(inner).soft_evict();
            }
        });
        self.0.borrow_mut().timer = Some(task);
    }

    /// Records a leave time, as `_lastViewedAt.set` does.
    pub(super) fn note_left(&self, id: &str) {
        let now = self.0.borrow().background.now();
        self.0.borrow_mut().last_viewed.insert(id.to_owned(), now);
    }
}
