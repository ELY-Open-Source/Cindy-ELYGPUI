//! `makerChatStore.ts`'s history slice for local tasks: the view, its mirror and the visit counts.
#![cfg_attr(
    not(test),
    expect(dead_code, reason = "the chat route reads the slice in 9.3")
)]

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use gpui::App;

use crate::{
    chat_message::merge::{Message, Messages},
    history_view::controller::View,
};

mod initial;
mod mirror;
mod older;
mod reconcile;
mod visit;

/// The slice of `SessionChatState` the history owns.
#[derive(Clone)]
pub struct SessionState {
    pub messages: Messages,
    pub history_loaded: bool,
    pub has_more_messages: bool,
    pub is_loading_more: bool,
    pub oldest_message_id: Option<String>,
}

impl Default for SessionState {
    /// `createInitialState`.
    fn default() -> Self {
        SessionState {
            messages: Rc::new(Vec::new()),
            history_loaded: false,
            has_more_messages: true,
            is_loading_more: false,
            oldest_message_id: None,
        }
    }
}

/// A `remoteHistoryViews` entry, less the intent queue of 20.0.
struct Entry {
    view: Option<View<Message>>,
    mirror_paused: bool,
}

#[derive(Default)]
struct Inner {
    sessions: HashMap<String, SessionState>,
    views: HashMap<String, Entry>,
    /// `_activeViewSessions`: how many views show each task.
    active: HashMap<String, usize>,
    epochs: HashMap<String, u64>,
    fetch_tokens: HashMap<String, u64>,
    next_token: u64,
    in_flight: HashSet<String>,
    force: HashMap<String, Rc<RefCell<reconcile::Flight>>>,
}

/// The store; clones share it, as the module's maps are shared.
#[derive(Clone, Default)]
pub struct Store(Rc<RefCell<Inner>>);

impl gpui::Global for Store {}

pub fn install(cx: &mut App) {
    cx.set_global(Store::default());
}

impl Store {
    pub fn global(cx: &App) -> Store {
        cx.global::<Store>().clone()
    }

    /// The task's slice, if the store holds one.
    pub fn state(&self, id: &str) -> Option<SessionState> {
        self.0.borrow().sessions.get(id).cloned()
    }

    fn has(&self, id: &str) -> bool {
        self.0.borrow().sessions.contains_key(id)
    }

    /// `getOrCreateState`.
    fn get_or_create(&self, id: &str) -> SessionState {
        let mut inner = self.0.borrow_mut();
        inner.sessions.entry(id.to_owned()).or_default().clone()
    }

    /// `setState`: `update` returns `None` to keep the state as it is.
    fn set(&self, id: &str, update: impl FnOnce(&SessionState) -> Option<SessionState>) {
        let mut inner = self.0.borrow_mut();
        let state = inner.sessions.entry(id.to_owned()).or_default();
        if let Some(next) = update(state) {
            *state = next;
        }
    }

    fn epoch(&self, id: &str) -> u64 {
        self.0.borrow().epochs.get(id).copied().unwrap_or(0)
    }

    /// `getRemoteHistoryView`: a local entry is current while it holds its view.
    fn view(&self, id: &str) -> Option<View<Message>> {
        self.0.borrow().views.get(id)?.view.clone()
    }

    /// Whether `view` is still the task's view.
    fn is_view(&self, id: &str, view: &View<Message>) -> bool {
        self.view(id).is_some_and(|v| v.same(view))
    }

    /// A database change stands for Cindy's pushes.
    pub fn invalidate_all(&self) {
        let views: Vec<_> = self
            .0
            .borrow()
            .views
            .values()
            .filter_map(|e| e.view.clone())
            .collect();
        for view in views {
            view.invalidate();
        }
    }
}

#[cfg(test)]
mod tests;
