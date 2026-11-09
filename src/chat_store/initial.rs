//! `ensureInitialMessages` through the view, with its fetch guards.

use gpui::App;

use super::{SessionState, Store};

/// One initial fetch's identity: `historyFetchToken` and the epoch it started in.
#[derive(Clone, Copy)]
struct Fetch {
    token: u64,
    epoch: u64,
}

impl Store {
    /// `isCurrentHistoryFetch`; a local task's origin never changes.
    fn is_current_fetch(&self, id: &str, fetch: Fetch) -> bool {
        let inner = self.0.borrow();
        inner.sessions.contains_key(id)
            && inner.fetch_tokens.get(id) == Some(&fetch.token)
            && inner.epochs.get(id).copied().unwrap_or(0) == fetch.epoch
    }

    /// `releaseHistoryFetchIfCurrent`.
    fn release_fetch(&self, id: &str, token: u64) {
        let mut inner = self.0.borrow_mut();
        if inner.fetch_tokens.get(id) == Some(&token) {
            inner.in_flight.remove(id);
        }
    }

    /// `retryInvalidatedInitialHistoryFetchIfNeeded`.
    fn retry_invalidated(&self, id: &str, fetch: Fetch, cx: &mut App) {
        let owns = self.0.borrow().fetch_tokens.get(id) == Some(&fetch.token);
        if owns {
            self.set(id, |s| {
                s.is_loading_more.then(|| SessionState {
                    is_loading_more: false,
                    ..s.clone()
                })
            });
        }
        let epoch_changed = self.epoch(id) != fetch.epoch;
        self.release_fetch(id, fetch.token);
        if !owns || !epoch_changed || !self.has(id) {
            return;
        }
        let loaded = self.state(id).is_some_and(|s| s.history_loaded);
        if loaded || self.0.borrow().in_flight.contains(id) {
            return;
        }
        log::info!(target: "chatStore", "initial history of {id} retried after its window changed");
        self.ensure_initial_messages(id, cx);
    }

    /// `ensureInitialMessages`: the first page through the view; the mirror fills `messages`.
    pub fn ensure_initial_messages(&self, id: &str, cx: &mut App) {
        let state = self.get_or_create(id);
        if state.history_loaded || self.0.borrow().in_flight.contains(id) {
            return;
        }
        let fetch = {
            let mut inner = self.0.borrow_mut();
            inner.next_token += 1;
            let token = inner.next_token;
            inner.fetch_tokens.insert(id.to_owned(), token);
            inner.in_flight.insert(id.to_owned());
            Fetch {
                token,
                epoch: inner.epochs.get(id).copied().unwrap_or(0),
            }
        };
        log::debug!(target: "chatStore", "initial history of {id} started");
        let view = self.create_view(id, cx);
        let run = view.refresh(false, false);
        let (store, id) = (self.clone(), id.to_owned());
        cx.spawn(async move |cx| {
            run.await;
            cx.update(|cx| store.settle_initial(&id, &view, fetch, cx));
        })
        .detach();
    }

    /// The view branch after its read, and the shared failure branch.
    fn settle_initial(
        &self,
        id: &str,
        view: &crate::history_view::controller::View<crate::chat_message::merge::Message>,
        fetch: Fetch,
        cx: &mut App,
    ) {
        if !self.is_current_fetch(id, fetch) || !self.is_view(id, view) {
            return self.retry_invalidated(id, fetch, cx);
        }
        let snapshot = view.snapshot();
        if snapshot.error.is_none() && snapshot.ready {
            log::debug!(target: "chatStore", "initial history of {id} ready");
            return self.release_fetch(id, fetch.token);
        }
        // A local view has no missing channel, so its raw fallback never runs; this is a failure.
        let error = snapshot.error.as_deref().unwrap_or("undefined");
        log::warn!(target: "chatStore", "initial history of {id} failed: {error}");
        if !self.is_current_fetch(id, fetch) {
            return self.retry_invalidated(id, fetch, cx);
        }
        {
            let mut inner = self.0.borrow_mut();
            inner.in_flight.remove(id);
            inner.fetch_tokens.remove(id);
        }
        self.set(id, |s| {
            Some(SessionState {
                history_loaded: false,
                is_loading_more: false,
                ..s.clone()
            })
        });
    }
}
