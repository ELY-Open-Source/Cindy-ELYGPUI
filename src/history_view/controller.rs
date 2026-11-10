//! `historyViewController.ts`: one task's reading state over its history pages.

use std::{cell::RefCell, rc::Rc};

use futures::future::{LocalBoxFuture, Shared};
use gpui::{BackgroundExecutor, ForegroundExecutor, Task};
use indexmap::{IndexMap, IndexSet};

use super::{
    Item, Summary,
    reader::{DetailPage, Page, Ref},
};

mod continuity;
mod details;
mod locate;
mod page;

/// The rows a view holds: Cindy's `HistoryMessageSource`.
pub trait Message: Clone + PartialEq + 'static {
    fn id(&self) -> &str;
    fn client_id(&self) -> &str;
    fn created_at(&self) -> &str;
    /// `detailCharacters`: the UTF-16 length of every string inside.
    fn characters(&self) -> usize;
}

/// `HistoryViewTransport`, less the interest intent of 20.0.
pub trait Transport<T> {
    fn page(&self, before: Option<String>) -> LocalBoxFuture<'static, Result<Page<T>, String>>;
    fn details(
        &self,
        r: Ref,
        after: Option<String>,
    ) -> LocalBoxFuture<'static, Result<DetailPage<T>, String>>;
}

/// `HistoryWorkDetailState`.
#[derive(Clone, Debug, PartialEq)]
pub struct Detail<T> {
    pub messages: Vec<T>,
    pub revision: String,
    pub last_message_id: String,
    pub loading: bool,
    pub complete: bool,
    pub error: Option<String>,
}

/// `HistoryViewSnapshot`.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot<T> {
    pub items: Vec<Item<T>>,
    pub details: IndexMap<String, Detail<T>>,
    pub expanded: IndexSet<String>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
    pub loading: bool,
    pub ready: bool,
    pub error: Option<String>,
    pub stale: bool,
}

impl<T> Default for Snapshot<T> {
    fn default() -> Self {
        Snapshot {
            items: Vec::new(),
            details: IndexMap::new(),
            expanded: IndexSet::new(),
            next_cursor: None,
            has_more: false,
            loading: false,
            ready: false,
            error: None,
            stale: false,
        }
    }
}

/// `isHistoryViewUnavailable`.
pub fn unavailable(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    [
        "channel_not_allowed",
        "unsupported_capability",
        "not registered",
        "no handler",
    ]
    .iter()
    .any(|m| error.contains(m))
}

pub type Run = Shared<LocalBoxFuture<'static, ()>>;

/// A reply resumes its reader a turn later, as an awaited settled promise does.
pub async fn settled<F: std::future::Future>(reply: F) -> F::Output {
    let value = reply.await;
    let mut yielded = false;
    std::future::poll_fn(|cx| {
        if std::mem::replace(&mut yielded, true) {
            return std::task::Poll::Ready(());
        }
        cx.waker().wake_by_ref();
        std::task::Poll::Pending
    })
    .await;
    value
}

struct Inner<T> {
    state: Snapshot<T>,
    listeners: Vec<(usize, Rc<dyn Fn()>)>,
    next_listener: usize,
    generation: u64,
    /// Each range's read in flight, with the token that tells reruns apart.
    detail_runs: IndexMap<String, (u64, Run)>,
    next_run: u64,
    expanded_holds: IndexMap<String, usize>,
    active: bool,
    page_run: Option<Run>,
    page_older: bool,
    page_generation: u64,
    latest_read_started_ms: f64,
    latest_read_ok_started_ms: f64,
    refresh_timer: Option<Task<()>>,
}

impl<T> Inner<T> {
    /// The page read still in flight; a settled one counts as none, as Cindy's `finally` clears it.
    fn running_page(&self) -> Option<Run> {
        self.page_run.clone().filter(|r| r.peek().is_none())
    }
}

/// `HistoryViewController`; clones share one view.
pub struct View<T> {
    inner: Rc<RefCell<Inner<T>>>,
    transport: Rc<dyn Transport<T>>,
    foreground: ForegroundExecutor,
    background: BackgroundExecutor,
}

impl<T> Clone for View<T> {
    fn clone(&self) -> Self {
        View {
            inner: self.inner.clone(),
            transport: self.transport.clone(),
            foreground: self.foreground.clone(),
            background: self.background.clone(),
        }
    }
}

pub struct WeakView<T> {
    inner: std::rc::Weak<RefCell<Inner<T>>>,
    transport: Rc<dyn Transport<T>>,
    foreground: ForegroundExecutor,
    background: BackgroundExecutor,
}

impl<T> WeakView<T> {
    pub fn upgrade(&self) -> Option<View<T>> {
        Some(View {
            inner: self.inner.upgrade()?,
            transport: self.transport.clone(),
            foreground: self.foreground.clone(),
            background: self.background.clone(),
        })
    }
}

/// `historyWorkSummaries` over borrowed items.
pub fn summaries<T>(items: &[Item<T>]) -> Vec<&Summary> {
    let mut out = Vec::new();
    for item in items {
        match item {
            Item::Work {
                children: Some(children),
                ..
            } => out.extend(summaries(children)),
            Item::Work { summary, .. } => {
                out.push(summary);
                out.extend(summary.preview.as_deref());
            }
            Item::Messages { deferred, .. } => out.extend(deferred.as_ref()),
        }
    }
    out
}

/// `historyViewLeaves`.
pub fn leaves<T>(items: &[Item<T>]) -> Vec<&Item<T>> {
    items
        .iter()
        .flat_map(|item| match item {
            Item::Work {
                children: Some(children),
                ..
            } => leaves(children),
            _ => vec![item],
        })
        .collect()
}

/// A patch to the snapshot; `None` keeps the field.
struct Patch<T> {
    items: Option<Vec<Item<T>>>,
    details: Option<IndexMap<String, Detail<T>>>,
    expanded: Option<IndexSet<String>>,
    next_cursor: Option<Option<String>>,
    has_more: Option<bool>,
    loading: Option<bool>,
    ready: Option<bool>,
    error: Option<Option<String>>,
    stale: Option<bool>,
}

impl<T> Default for Patch<T> {
    fn default() -> Self {
        Patch {
            items: None,
            details: None,
            expanded: None,
            next_cursor: None,
            has_more: None,
            loading: None,
            ready: None,
            error: None,
            stale: None,
        }
    }
}

impl<T: Message> View<T> {
    pub fn new(
        transport: Rc<dyn Transport<T>>,
        foreground: ForegroundExecutor,
        background: BackgroundExecutor,
    ) -> Self {
        let inner = Inner {
            state: Snapshot::default(),
            listeners: Vec::new(),
            next_listener: 0,
            generation: 0,
            detail_runs: IndexMap::new(),
            next_run: 0,
            expanded_holds: IndexMap::new(),
            active: true,
            page_run: None,
            page_older: false,
            page_generation: 0,
            latest_read_started_ms: 0.0,
            latest_read_ok_started_ms: 0.0,
            refresh_timer: None,
        };
        View {
            inner: Rc::new(RefCell::new(inner)),
            transport,
            foreground,
            background,
        }
    }

    pub fn is_active(&self) -> bool {
        self.inner.borrow().active
    }

    /// `===` between controllers.
    pub fn same(&self, other: &View<T>) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }

    /// A handle for the view's own listeners, which must not keep it alive.
    pub fn downgrade(&self) -> WeakView<T> {
        WeakView {
            inner: Rc::downgrade(&self.inner),
            transport: self.transport.clone(),
            foreground: self.foreground.clone(),
            background: self.background.clone(),
        }
    }

    pub fn snapshot(&self) -> Snapshot<T> {
        self.inner.borrow().state.clone()
    }

    /// `isLoadingOlder`.
    pub fn is_loading_older(&self) -> bool {
        let inner = self.inner.borrow();
        inner.state.loading && inner.page_older
    }

    /// `latestReadStartedAt`: when the last latest-page read began, 0 before the first.
    pub fn latest_read_started_at(&self) -> f64 {
        self.inner.borrow().latest_read_started_ms
    }

    /// `latestReadOkStartedAt`: the start of the last latest-page read that was applied.
    pub fn latest_read_ok_started_at(&self) -> f64 {
        self.inner.borrow().latest_read_ok_started_ms
    }

    /// `isReadingLatest`: a latest-page read is in flight; older reads do not count.
    pub fn is_reading_latest(&self) -> bool {
        let inner = self.inner.borrow();
        inner.running_page().is_some() && !inner.page_older
    }

    /// `subscribe`; the returned closure unsubscribes.
    pub fn subscribe<F: Fn() + 'static>(&self, listener: F) -> impl FnOnce() + use<T, F> {
        let mut inner = self.inner.borrow_mut();
        let id = inner.next_listener;
        inner.next_listener += 1;
        inner.listeners.push((id, Rc::new(listener)));
        let weak = Rc::downgrade(&self.inner);
        move || {
            if let Some(inner) = weak.upgrade() {
                inner.borrow_mut().listeners.retain(|(i, _)| *i != id);
            }
        }
    }

    fn publish(&self, patch: Patch<T>, source: Option<&str>) {
        let listeners = {
            let mut inner = self.inner.borrow_mut();
            let previous = inner.state.items.len();
            let s = &mut inner.state;
            macro_rules! set {
                ($($f:ident),*) => { $(if let Some(v) = patch.$f { s.$f = v; })* };
            }
            set!(
                items,
                details,
                expanded,
                next_cursor,
                has_more,
                loading,
                ready,
                error,
                stale
            );
            if let Some(source) = source {
                let messages: usize = leaves(&s.items)
                    .iter()
                    .map(|i| match i {
                        Item::Messages { messages, .. } => messages.len(),
                        Item::Work { .. } => 0,
                    })
                    .sum();
                let detail: usize = s.details.values().map(|d| d.messages.len()).sum();
                log::debug!(target: "history", "history view changed source {source} previousItemCount {previous} itemCount {} messageCount {messages} detailMessageCount {detail} ready {} stale {}", s.items.len(), s.ready, s.stale);
            }
            inner
                .listeners
                .iter()
                .map(|(_, l)| l.clone())
                .collect::<Vec<_>>()
        };
        for listener in listeners {
            listener();
        }
    }

    fn spawn(&self, run: impl std::future::Future<Output = ()> + 'static) {
        self.foreground.spawn(run).detach();
    }

    /// `setActive`: leaving or returning invalidates every read in flight.
    pub fn set_active(&self, active: bool) {
        if active == self.is_active() {
            if active && !self.inner.borrow().state.ready {
                drop(self.refresh(false, false));
            }
            return;
        }
        let ready = {
            let mut inner = self.inner.borrow_mut();
            inner.refresh_timer = None;
            inner.active = active;
            inner.generation += 1;
            inner.detail_runs.clear();
            inner.state.ready
        };
        self.publish(
            Patch {
                loading: Some(false),
                stale: (!active).then_some(ready),
                ..Patch::default()
            },
            None,
        );
        if active {
            drop(self.refresh(false, false));
        }
    }

    /// `reset`: empty, then read again while active.
    pub fn reset(&self) {
        {
            let mut inner = self.inner.borrow_mut();
            inner.generation += 1;
            inner.detail_runs.clear();
        }
        self.publish(
            Patch {
                items: Some(Vec::new()),
                details: Some(IndexMap::new()),
                expanded: Some(IndexSet::new()),
                ready: Some(false),
                next_cursor: Some(None),
                has_more: Some(false),
                loading: Some(false),
                error: Some(None),
                stale: Some(false),
            },
            Some("reset"),
        );
        if self.is_active() {
            drop(self.refresh(false, false));
        }
    }

    /// Runs a read with no caller awaiting it, as an unawaited promise still settles.
    fn drive(&self, run: Run) {
        self.spawn(run);
    }
}

#[cfg(test)]
mod tests;
