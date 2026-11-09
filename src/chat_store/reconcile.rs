//! `reconcileRemoteMessages`' history-view branch, as a forced resume read runs it.

use std::{cell::RefCell, collections::HashMap, collections::HashSet, rc::Rc};

use futures::{
    FutureExt,
    future::{LocalBoxFuture, Shared, join_all},
};
use gpui::ForegroundExecutor;

use super::{SessionState, Store, mirror::page_rows};
use crate::{
    chat_message::merge::{self, Key, Message, Messages, Options, RowsOrder},
    history_view::controller::{View, summaries},
    rich_answer::js::{self, Js},
};

pub type Run = Shared<LocalBoxFuture<'static, Result<bool, String>>>;

/// The options its callers pass: `force` and `repair` always hold.
#[derive(Clone, Default)]
pub struct Opts {
    /// The caller just reactivated the view, whose read this one joins.
    pub activation_read: bool,
    pub preserve: HashSet<String>,
}

/// `HistoryViewForceFlight`.
pub struct Flight {
    run: Option<Run>,
    pub(super) rerun: bool,
    rows_at_start: HashMap<Key, Message>,
    latest: Opts,
}

/// What `runHistoryView` takes before its first await.
struct Start {
    at_start: HashMap<Key, Message>,
    epoch: u64,
    read: crate::history_view::controller::Run,
}

fn text<'a>(row: &'a Message, key: &str) -> Option<&'a str> {
    merge::get(row, key).and_then(Js::str)
}

fn rows_by_client(messages: &Messages) -> HashMap<Key, Message> {
    messages
        .iter()
        .map(|r| (merge::client_id(r), r.clone()))
        .collect()
}

fn same(a: Option<&Message>, b: Option<&Message>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        _ => false,
    }
}

impl Store {
    /// `reconcileRemoteMessages` with `force` and `repair`: one forced read per task at a time.
    pub(super) fn reconcile(&self, id: &str, opts: Opts, fg: &ForegroundExecutor) -> Run {
        let Some(view) = self.view(id) else {
            // A local task has no raw remote reconcile.
            return async { Ok(false) }.boxed_local().shared();
        };
        if let Some(flight) = self.0.borrow().force.get(id).cloned() {
            let mut flight = flight.borrow_mut();
            flight.rerun = true;
            flight.latest = opts;
            return flight.run.clone().expect("a started flight");
        }
        let messages = self.get_or_create(id).messages;
        let flight = Rc::new(RefCell::new(Flight {
            run: None,
            rerun: false,
            rows_at_start: rows_by_client(&messages),
            latest: opts.clone(),
        }));
        self.0
            .borrow_mut()
            .force
            .insert(id.to_owned(), flight.clone());
        // `runHistoryView` runs to its first await inside this call.
        let start = Start {
            at_start: rows_by_client(&messages),
            epoch: self.epoch(id),
            read: view.refresh(false, !opts.activation_read),
        };
        let (store, task, fg2) = (self.clone(), id.to_owned(), fg.clone());
        let held = flight.clone();
        let run = async move {
            let result = store.flight(&task, &view, opts, start, &held, &fg2).await;
            let mut inner = store.0.borrow_mut();
            if inner.force.get(&task).is_some_and(|f| Rc::ptr_eq(f, &held)) {
                inner.force.remove(&task);
            }
            result
        }
        .boxed_local()
        .shared();
        flight.borrow_mut().run = Some(run.clone());
        fg.spawn(run.clone()).detach();
        run
    }

    async fn flight(
        &self,
        id: &str,
        view: &View<Message>,
        opts: Opts,
        start: Start,
        flight: &Rc<RefCell<Flight>>,
        fg: &ForegroundExecutor,
    ) -> Result<bool, String> {
        let mut applied = match self.run_view(id, view, &opts, start, flight).await {
            Ok(applied) => applied,
            Err(error) if !flight.borrow().rerun || !self.has(id) => return Err(error),
            Err(_) => false,
        };
        if flight.borrow().rerun && self.has(id) {
            self.0.borrow_mut().force.remove(id);
            let mut latest = flight.borrow().latest.clone();
            let messages = self.get_or_create(id).messages;
            {
                let held = flight.borrow();
                for row in messages.iter() {
                    let key = merge::client_id(row);
                    if held
                        .rows_at_start
                        .get(&key)
                        .is_some_and(|r| !Rc::ptr_eq(r, row))
                        && let Some(client) = text(row, "clientId")
                    {
                        latest.preserve.insert(client.to_owned());
                    }
                }
            }
            log::debug!(target: "chatStore", "forced history read of {id} runs again");
            applied = self.reconcile(id, latest, fg).await?;
        }
        Ok(applied)
    }

    /// `runHistoryView`.
    async fn run_view(
        &self,
        id: &str,
        view: &View<Message>,
        opts: &Opts,
        start: Start,
        flight: &Rc<RefCell<Flight>>,
    ) -> Result<bool, String> {
        let Start {
            at_start,
            epoch,
            read,
        } = start;
        // Pending interactions are read with 10.0.
        read.await;
        if !self.is_view(id, view) || !view.is_active() {
            return Ok(false);
        }
        let live: Vec<&Message> = at_start
            .values()
            .filter(|r| js::truthy(merge::get(r, "isStreaming")))
            .collect();
        let snapshot = view.snapshot();
        let all = summaries(&snapshot.items);
        let recovery: HashSet<&str> = all
            .iter()
            .filter(|s| {
                live.iter().any(|row| {
                    if s.parent_tool_use_id.is_some()
                        && !js::truthy(merge::get(row, "parentToolUseId"))
                    {
                        return false;
                    }
                    let at = crate::js_date::parse(text(row, "createdAt").unwrap_or(""));
                    text(row, "clientId") == Some(s.anchor_client_id.as_str())
                        || (at >= s.started_at_ms && at <= s.ended_at_ms)
                })
            })
            .map(|s| s.key.as_str())
            .collect();
        let reads = all
            .iter()
            .filter(|s| recovery.contains(s.key.as_str()) || snapshot.expanded.contains(&s.key))
            .map(|s| view.load_details((*s).clone(), recovery.contains(s.key.as_str())));
        join_all(reads.collect::<Vec<_>>()).await;
        if !self.is_view(id, view) || !view.is_active() || self.epoch(id) != epoch {
            return Ok(false);
        }
        let details = view.snapshot();
        let incomplete = summaries(&details.items).iter().any(|s| {
            if !recovery.contains(s.key.as_str()) && !details.expanded.contains(&s.key) {
                return false;
            }
            details
                .details
                .get(&s.key)
                .is_none_or(|d| !d.complete || d.error.is_some() || d.revision != s.revision)
        });
        if incomplete {
            log::warn!(target: "chatStore", "forced history read of {id} met incomplete details; the raw reconcile comes with 9.2f");
            return Ok(false);
        }
        if !self.is_view(id, view) || !view.is_active() {
            return Ok(false);
        }
        let snapshot = view.snapshot();
        if let Some(error) = snapshot.error.clone() {
            return Err(error);
        }
        if self.epoch(id) != epoch {
            return Ok(false);
        }
        if snapshot.ready {
            self.hydrate(id, &snapshot, opts, &at_start, flight);
        }
        Ok(snapshot.ready)
    }

    /// The forced read hydrates rows the store has not changed since it began.
    fn hydrate(
        &self,
        id: &str,
        snapshot: &crate::history_view::controller::Snapshot<Message>,
        opts: &Opts,
        at_start: &HashMap<Key, Message>,
        flight: &Rc<RefCell<Flight>>,
    ) {
        let mut rows: Vec<Message> = summaries(&snapshot.items)
            .iter()
            .flat_map(|s| match snapshot.details.get(&s.key) {
                Some(d) if d.complete && d.error.is_none() && d.revision == s.revision => {
                    d.messages.clone()
                }
                _ => Vec::new(),
            })
            .collect();
        rows.extend(page_rows(&snapshot.items));
        rows.retain(|r| {
            !text(r, "id").unwrap_or("").starts_with("history-live:")
                && !text(r, "clientId").is_some_and(|c| opts.preserve.contains(c))
        });
        self.set(id, |state| {
            let untouched = state
                .messages
                .iter()
                .filter(|r| same(at_start.get(&merge::client_id(r)), Some(r)))
                .filter_map(|r| text(r, "clientId").map(str::to_owned))
                .collect();
            let options = Options {
                add_only: true,
                add_only_except: Some(untouched),
                ..Options::default()
            };
            let messages = merge::merge(&rows, &state.messages, &options, RowsOrder::OldestFirst);
            // `noteHydration`: advance the flight's baseline only for its own writes.
            let previous = rows_by_client(&state.messages);
            let mut flight = flight.borrow_mut();
            for row in messages.iter() {
                let key = merge::client_id(row);
                if same(flight.rows_at_start.get(&key), previous.get(&key)) {
                    flight.rows_at_start.insert(key, row.clone());
                }
            }
            (!Rc::ptr_eq(&messages, &state.messages)).then(|| SessionState {
                messages,
                ..state.clone()
            })
        });
    }
}
