//! `makerChatStore.ts`'s history slice for local tasks: the view, its mirror and the visit counts.
#![cfg_attr(
    not(test),
    expect(dead_code, reason = "the chat route reads the slice in 9.3")
)]

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
    time::Instant,
};

use gpui::{App, BackgroundExecutor, ForegroundExecutor, Task};
use indexmap::IndexMap;

use crate::{
    chat_message::merge::{Message, Messages},
    history_view::controller::View,
};

mod initial;
mod memory;
mod mirror;
mod older;
mod raw;
mod reconcile;
mod visit;
mod window;

/// The slice of `SessionChatState` the history owns.
#[derive(Clone)]
pub struct SessionState {
    pub messages: Messages,
    pub history_loaded: bool,
    pub has_more_messages: bool,
    pub is_loading_more: bool,
    pub oldest_message_id: Option<String>,
    pub history_window_islands: Rc<Vec<window::Island>>,
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
            history_window_islands: Rc::new(Vec::new()),
        }
    }
}

/// A `remoteHistoryViews` entry, less the intent queue of 20.0.
struct Entry {
    view: Option<View<Message>>,
    mirror_paused: bool,
}

struct Inner {
    /// `sessions`, in the order a `Map` keeps.
    sessions: IndexMap<String, SessionState>,
    /// `_accessOrder`: least recently used first.
    access: Vec<String>,
    /// `_lastViewedAt`.
    last_viewed: HashMap<String, Instant>,
    budget: memory::Budget,
    timer: Option<Task<()>>,
    foreground: ForegroundExecutor,
    background: BackgroundExecutor,
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
#[derive(Clone)]
pub struct Store(Rc<RefCell<Inner>>);

impl gpui::Global for Store {}

pub fn install(cx: &mut App) {
    let inner = Inner {
        sessions: IndexMap::new(),
        access: Vec::new(),
        last_viewed: HashMap::new(),
        budget: memory::Budget::default(),
        timer: None,
        foreground: cx.foreground_executor().clone(),
        background: cx.background_executor().clone(),
        views: HashMap::new(),
        active: HashMap::new(),
        epochs: HashMap::new(),
        fetch_tokens: HashMap::new(),
        next_token: 0,
        in_flight: HashSet::new(),
        force: HashMap::new(),
    };
    cx.set_global(Store(Rc::new(RefCell::new(inner))));
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
        self.admit(id);
        // A slice the cap pushed out at once still answers, as Cindy's local does.
        self.0
            .borrow()
            .sessions
            .get(id)
            .cloned()
            .unwrap_or_default()
    }

    /// `setState`: `update` returns `None` to keep the state as it is.
    fn set(&self, id: &str, update: impl FnOnce(&SessionState) -> Option<SessionState>) {
        let prev = self.get_or_create(id);
        if let Some(next) = update(&prev) {
            self.0.borrow_mut().sessions.insert(id.to_owned(), next);
        }
    }

    /// The views open on `id`, for tests outside the store.
    #[cfg(test)]
    pub fn visits(&self, id: &str) -> usize {
        self.0.borrow().active.get(id).copied().unwrap_or(0)
    }

    pub(crate) fn epoch(&self, id: &str) -> u64 {
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
