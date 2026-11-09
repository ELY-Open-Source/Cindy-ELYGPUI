//! `createRemoteHistoryView` for a local task: the view, its borrowed detail rows and the mirror.

use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    rc::{Rc, Weak},
};

use futures::{FutureExt, future::LocalBoxFuture};
use gpui::App;

use super::{Entry, SessionState, Store};
use crate::{
    chat_message::{
        merge::{self, Fields, Key, Message, Options, RowsOrder, handoff::confirm_remote_users},
        transport::Local,
    },
    history_view::{
        Item,
        controller::{self, Transport, View, leaves},
        reader::{DetailPage, Page, Ref},
    },
    rich_answer::js::Js,
};

/// `borrowedDetailRows`: the rows `details` returned, held as a `WeakSet` holds them.
#[derive(Clone, Default)]
struct Borrowed(Rc<RefCell<Vec<Weak<Fields>>>>);

impl Borrowed {
    fn has(&self, row: &Message) -> bool {
        let ptr = Rc::as_ptr(row);
        self.0
            .borrow()
            .iter()
            .any(|w| w.as_ptr() == ptr && w.strong_count() > 0)
    }
}

/// The view's transport: `details` lends its rows to the store.
struct Lending {
    local: Local,
    borrowed: Borrowed,
}

impl Transport<Message> for Lending {
    fn page(
        &self,
        before: Option<String>,
    ) -> LocalBoxFuture<'static, Result<Page<Message>, String>> {
        self.local.page(before)
    }

    fn details(
        &self,
        r: Ref,
        after: Option<String>,
    ) -> LocalBoxFuture<'static, Result<DetailPage<Message>, String>> {
        let (read, borrowed) = (self.local.details(r, after), self.borrowed.clone());
        async move {
            let page = read.await?;
            let mut rows = borrowed.0.borrow_mut();
            rows.retain(|w| w.strong_count() > 0);
            rows.extend(page.messages.iter().map(Rc::downgrade));
            Ok(page)
        }
        .boxed_local()
    }
}

fn text<'a>(row: &'a Message, key: &str) -> Option<&'a str> {
    merge::get(row, key).and_then(Js::str)
}

fn truthy(row: &Message, key: &str) -> bool {
    crate::rich_answer::js::truthy(merge::get(row, key))
}

/// `isRetirableHistoryTool`: only read and search bodies can be read back.
fn retirable_tool(name: &str) -> bool {
    ["Read", "read", "Grep", "grep", "Glob", "glob"].contains(&name)
        && crate::history_view::source::is_detail_tool(name)
}

/// The page's leaf rows.
pub(super) fn page_rows(items: &[Item<Message>]) -> Vec<Message> {
    let leaves = leaves(items).into_iter();
    leaves
        .flat_map(|item| match item {
            Item::Messages { messages, .. } => messages.clone(),
            Item::Work { .. } => Vec::new(),
        })
        .collect()
}

/// The snapshot's rows: page leaves, then every detail range.
fn available(snapshot: &controller::Snapshot<Message>) -> Vec<Message> {
    let mut rows = page_rows(&snapshot.items);
    for detail in snapshot.details.values() {
        rows.extend(detail.messages.iter().cloned());
    }
    rows
}

fn client_ids<'a>(rows: impl IntoIterator<Item = &'a Message>) -> HashSet<String> {
    rows.into_iter()
        .filter_map(|r| text(r, "clientId").map(str::to_owned))
        .collect()
}

/// `new Set(rows.map(row => row.clientId))`, under SameValueZero.
fn client_keys(rows: &[Message]) -> HashSet<Key> {
    rows.iter().map(|r| merge::client_id(r)).collect()
}

/// `mirrorSnapshot`'s state update.
fn mirrored(
    state: &SessionState,
    snapshot: &controller::Snapshot<Message>,
    borrowed: &Borrowed,
    loading_older: bool,
) -> Option<SessionState> {
    let available = available(snapshot);
    let add_only = Options {
        add_only: true,
        ..Options::default()
    };
    let merged = merge::merge(
        &available,
        &state.messages,
        &add_only,
        RowsOrder::OldestFirst,
    );
    let users = client_ids(available.iter().filter(|r| text(r, "role") == Some("user")));
    let mut messages = confirm_remote_users(&merged, &users);
    if !snapshot.stale && snapshot.error.is_none() {
        // Durable tool pairs retire with 10.0's echoes; borrowed detail bodies retire here.
        let ids = client_keys(&available);
        let reclaimable: HashSet<&str> = messages
            .iter()
            .filter(|r| text(r, "role") == Some("tool_use"))
            .filter(|r| retirable_tool(text(r, "toolName").unwrap_or("")))
            .filter_map(|r| text(r, "toolUseId").filter(|id| !id.is_empty()))
            .collect();
        let retired = |row: &Message| {
            let role = text(row, "role");
            let tool = text(row, "toolUseId").filter(|id| !id.is_empty());
            borrowed.has(row)
                && !ids.contains(&merge::client_id(row))
                && (role == Some("thinking")
                    || (matches!(role, Some("tool_use" | "tool_result"))
                        && tool.is_some_and(|id| reclaimable.contains(id))))
                && !truthy(row, "isStreaming")
                && !truthy(row, "isPendingPersist")
        };
        if messages.iter().any(retired) {
            let kept = messages.iter().filter(|r| !retired(r)).cloned().collect();
            messages = Rc::new(kept);
        }
    }
    let unchanged = state.history_loaded
        && Rc::ptr_eq(&state.messages, &messages)
        && state.has_more_messages == snapshot.has_more
        && state.is_loading_more == loading_older
        && state.oldest_message_id == snapshot.next_cursor;
    (!unchanged).then(|| SessionState {
        history_loaded: true,
        messages,
        has_more_messages: snapshot.has_more,
        is_loading_more: loading_older,
        oldest_message_id: snapshot.next_cursor.clone(),
    })
}

impl Store {
    /// `createRemoteHistoryView`: the task's view, made once.
    pub(super) fn create_view(&self, id: &str, cx: &mut App) -> View<Message> {
        if let Some(view) = self.view(id) {
            return view;
        }
        log::info!(target: "historyView", "view created for {id}");
        let borrowed = Borrowed::default();
        let transport = Lending {
            local: Local::new(id.to_owned(), cx.to_async()),
            borrowed: borrowed.clone(),
        };
        let view = View::new(
            Rc::new(transport),
            cx.foreground_executor().clone(),
            cx.background_executor().clone(),
        );
        {
            let mut inner = self.0.borrow_mut();
            let entry = inner.views.entry(id.to_owned()).or_insert(Entry {
                view: None,
                mirror_paused: false,
            });
            entry.view = Some(view.clone());
        }
        let scheduled = Rc::new(Cell::new(false));
        let (store, task, weak) = (self.clone(), id.to_owned(), view.downgrade());
        let foreground = cx.foreground_executor().clone();
        drop(view.subscribe(move || {
            let Some(view) = weak.upgrade() else { return };
            if !store.is_view(&task, &view) || !store.has(&task) {
                return;
            }
            let snapshot = view.snapshot();
            if snapshot.ready {
                let loading = view.is_loading_older();
                store.set(&task, |s| {
                    (s.is_loading_more != loading).then(|| SessionState {
                        is_loading_more: loading,
                        ..s.clone()
                    })
                });
            }
            if scheduled.replace(true) {
                return;
            }
            let epoch = store.epoch(&task);
            let (store, task, scheduled, borrowed) = (
                store.clone(),
                task.clone(),
                scheduled.clone(),
                borrowed.clone(),
            );
            // `queueMicrotask(mirrorSnapshot)`.
            foreground
                .spawn(async move {
                    scheduled.set(false);
                    store.mirror(&task, &view, epoch, &borrowed);
                })
                .detach();
        }));
        view
    }

    /// `mirrorSnapshot`.
    fn mirror(&self, id: &str, view: &View<Message>, epoch: u64, borrowed: &Borrowed) {
        let paused = self
            .0
            .borrow()
            .views
            .get(id)
            .is_none_or(|e| e.mirror_paused);
        if !self.is_view(id, view) || !self.has(id) || paused || self.epoch(id) != epoch {
            return;
        }
        let snapshot = view.snapshot();
        if !snapshot.ready {
            return;
        }
        let loading = view.is_loading_older();
        self.set(id, |s| mirrored(s, &snapshot, borrowed, loading));
    }
}
