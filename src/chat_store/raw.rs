//! The raw reads beside the view: `listMessagesFor` and `runRemoteReconcile`.

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use gpui::AsyncApp;

use super::{
    SessionState, Store,
    reconcile::{Flight, note_hydration},
    window::{self, Stamp, text},
};
use crate::{
    chat_message::{
        map_server_messages,
        merge::{self, Fields, Key, Message, Options, RowsOrder},
    },
    history_view::rows,
    i18n::Translator,
    localdb::{LocalDb, State, search::mcp_app::TrustLabels},
    rich_answer::js::Js,
};

/// `messageService.list`: one `local-db:messages:list` page on the database worker.
pub async fn list(cx: &AsyncApp, id: &str, opts: rows::Opts) -> Result<Vec<Js>, String> {
    let (db, labels) = cx.update(|cx| {
        let State::Ready(db) = &cx.global::<LocalDb>().state else {
            return Err("DbClient not ready".to_owned());
        };
        Ok((db.clone(), TrustLabels::new(cx.global::<Translator>())))
    })?;
    let session = id.to_owned();
    let answer = db.call(move |conn| {
        rows::list(conn, &session, &opts, false, &labels).map_err(|e| e.to_string())
    });
    let rows = answer
        .await
        .map_err(|_| "the database closed during the read".to_owned())??;
    log::debug!(target: "chatStore", "messages list of {id}: {} rows", rows.len());
    Ok(rows)
}

/// `mapServerMessages` as store rows.
pub fn mapped(rows: &[Js]) -> Result<Vec<Message>, String> {
    let out = map_server_messages(rows)?;
    let fields = out.into_iter().map(|row| {
        let fields: Fields = row
            .into_fields()
            .into_iter()
            .map(|(k, v)| (k, v.map(Rc::new)))
            .collect();
        Rc::new(fields)
    });
    Ok(fields.collect())
}

fn raw_key(row: &Js) -> Key {
    merge::key(row.obj().and_then(|r| r.get("clientId")))
}

impl Store {
    /// `invalidateMessageHistoryWindow`; the viewport's load budget resets with 9.14.
    pub(super) fn invalidate_window(&self, id: &str) {
        if let Some(view) = self.view(id) {
            view.reset();
        }
        *self.0.borrow_mut().epochs.entry(id.to_owned()).or_default() += 1;
    }

    /// `runRemoteReconcile` under a forced repair: newest pages back to the known window.
    pub(super) async fn raw_reconcile(
        &self,
        id: &str,
        preserve: &HashSet<String>,
        flight: &Rc<RefCell<Flight>>,
        cx: &AsyncApp,
    ) -> Result<bool, String> {
        // Pending interactions are read with 10.0.
        let Some(state) = self.state(id).filter(|s| s.history_loaded) else {
            return Ok(false);
        };
        let known: HashSet<Key> = state.messages.iter().map(|m| merge::client_id(m)).collect();
        let at_start: HashMap<Key, Message> = state
            .messages
            .iter()
            .map(|m| (merge::client_id(m), m.clone()))
            .collect();
        let epoch = self.epoch(id);
        log::info!(target: "chatStore", "raw reconcile of {id} started");
        let (mut collected, mut before) = (Vec::new(), None);
        let (mut contiguous, mut at_history_start) = (false, false);
        for _ in 0..10 {
            let opts = rows::Opts {
                limit: Some(50.0),
                before: before.take(),
                ..rows::Opts::default()
            };
            let rows = list(cx, id, opts).await?;
            if rows.is_empty() {
                at_history_start = true;
                break;
            }
            collected.extend(rows.iter().cloned());
            let overlap = rows.iter().any(|r| known.contains(&raw_key(r)));
            contiguous |= overlap;
            if overlap && !rows.iter().any(window::trimmed) {
                break;
            }
            if !window::page_has_more(&rows) {
                at_history_start = true;
                break;
            }
            let Some(oldest) = window::oldest_row(&rows, RowsOrder::NewestFirst) else {
                at_history_start = true;
                break;
            };
            before = oldest.get("id").and_then(Js::str).map(str::to_owned);
        }
        if self.epoch(id) != epoch {
            log::info!(target: "chatStore", "raw reconcile of {id} dropped: its window changed");
            return Ok(false);
        }
        if collected.is_empty() {
            return Ok(true);
        }
        let rows = mapped(&collected)?;
        let rows: Vec<Message> = rows
            .into_iter()
            .filter(|m| !text(m, "clientId").is_some_and(|c| preserve.contains(c)))
            .collect();
        let s = self.get_or_create(id);
        // A repair keeps late rows and live rows; the store's stream flag comes with 10.0.
        let late: Vec<Message> = match contiguous {
            true => Vec::new(),
            false => s
                .messages
                .iter()
                .filter(|m| {
                    !known.contains(&merge::client_id(m))
                        || merge::get(m, "isStreaming") == Some(&Js::Bool(true))
                })
                .cloned()
                .collect(),
        };
        let untouched = s
            .messages
            .iter()
            .filter(|m| {
                at_start
                    .get(&merge::client_id(m))
                    .is_some_and(|r| Rc::ptr_eq(r, m))
            })
            .filter_map(|m| text(m, "clientId").map(str::to_owned))
            .collect();
        let messages = match contiguous {
            true => {
                let options = Options {
                    add_only: true,
                    add_only_except: Some(untouched),
                    ..Options::default()
                };
                merge::merge(&rows, &s.messages, &options, RowsOrder::NewestFirst)
            }
            false if rows.is_empty() => Rc::new(late.clone()),
            false => {
                let add_only = Options {
                    add_only: true,
                    ..Options::default()
                };
                merge::merge(
                    &rows,
                    &Rc::new(late.clone()),
                    &add_only,
                    RowsOrder::NewestFirst,
                )
            }
        };
        note_hydration(flight, &s.messages, &messages);
        if Rc::ptr_eq(&messages, &s.messages) {
            return Ok(true);
        }
        if contiguous {
            self.set(id, |s| {
                Some(SessionState {
                    messages,
                    ..s.clone()
                })
            });
            return Ok(true);
        }
        self.invalidate_window(id);
        let oldest = window::oldest_row(&collected, RowsOrder::NewestFirst);
        let newest = window::newest_row(&collected);
        let authoritative: HashSet<Key> = collected.iter().map(raw_key).collect();
        let detached = match (oldest, newest) {
            (Some(oldest), Some(newest)) => late.iter().any(|m| {
                if authoritative.contains(&merge::client_id(m)) {
                    return false;
                }
                // `thinkingSafeTimelineRow`: a thinking block needs its finish time.
                let finished = merge::get(m, "thinkingFinishedAtMs");
                if text(m, "role") == Some("thinking")
                    && !matches!(finished, Some(Js::Num(n)) if n.is_finite())
                {
                    return true;
                }
                let at = Stamp::message(m);
                let older = window::compare(at, Stamp::raw(oldest));
                let newer = window::compare(at, Stamp::raw(newest));
                older.is_lt() || newer.is_gt() || older.is_eq() || newer.is_eq()
            }),
            _ => false,
        };
        log::info!(target: "chatStore", "raw reconcile of {id} replaced its window, detached {detached}");
        let oldest_id = oldest
            .and_then(|o| o.get("id"))
            .and_then(Js::str)
            .map(str::to_owned);
        self.set(id, |s| {
            Some(SessionState {
                history_window_islands: Rc::new(match detached {
                    true => window::conservative(&messages),
                    false => Vec::new(),
                }),
                oldest_message_id: oldest_id.or_else(|| s.oldest_message_id.clone()),
                has_more_messages: !at_history_start,
                is_loading_more: false,
                messages,
                ..s.clone()
            })
        });
        Ok(true)
    }
}
