//! `loadOlderMessages`: through the view once it is ready, else raw pages of 50.

use std::{collections::HashSet, rc::Rc};

use futures::{FutureExt, future::LocalBoxFuture};
use gpui::{App, AsyncApp};

use super::{
    SessionState, Store,
    raw::{list, mapped},
    window,
};
use crate::{
    chat_message::merge::{self, Key, Options, RowsOrder},
    history_view::rows,
    rich_answer::js::{self, Js},
};

/// `MAX_LOAD_OLDER_PAGES`: one gesture reads until a visible prompt, at most ten pages.
const MAX_PAGES: usize = 10;

impl Store {
    /// `loadOlderMessages`: true only when the window moved back.
    pub fn load_older_messages(
        &self,
        id: &str,
        cx: &App,
    ) -> LocalBoxFuture<'static, Result<bool, String>> {
        let Some(view) = self.view(id).filter(|v| v.snapshot().ready) else {
            return self.load_older_raw(id, cx.to_async());
        };
        let before = view.snapshot().next_cursor;
        let epoch = self.epoch(id);
        let run = view.refresh(true, false);
        let (store, id) = (self.clone(), id.to_owned());
        async move {
            run.await;
            if !store.is_view(&id, &view) {
                return Ok(false);
            }
            let snapshot = view.snapshot();
            if let Some(error) = snapshot.error {
                log::warn!(target: "chatStore", "older history of {id} failed: {error}");
                return Err(error);
            }
            Ok(store.is_view(&id, &view)
                && snapshot.ready
                && view.is_active()
                && store.epoch(&id) == epoch
                && snapshot.next_cursor != before)
        }
        .boxed_local()
    }

    /// The raw pages: `before` the cursor, or before the main run's oldest row.
    fn load_older_raw(
        &self,
        id: &str,
        cx: AsyncApp,
    ) -> LocalBoxFuture<'static, Result<bool, String>> {
        let state = self.get_or_create(id);
        if state.is_loading_more || !state.has_more_messages {
            return async { Ok(false) }.boxed_local();
        }
        let first = if let Some(before) = state.oldest_message_id.clone() {
            rows::Opts {
                limit: Some(50.0),
                before: Some(before),
                ..rows::Opts::default()
            }
        } else {
            let islands = &state.history_window_islands;
            let oldest = match islands.is_empty() {
                true => state.messages.first(),
                false => {
                    window::main_run_oldest(&state.messages, islands).or(state.messages.first())
                }
            };
            let created = oldest.and_then(|m| merge::get(m, "createdAt"));
            let ms = crate::chat_message::time(created);
            if !js::truthy(created) || !ms.is_finite() {
                return async { Ok(false) }.boxed_local();
            }
            rows::Opts {
                limit: Some(50.0),
                before_ts: Some(ms),
                ..rows::Opts::default()
            }
        };
        self.set(id, |s| {
            Some(SessionState {
                is_loading_more: true,
                ..s.clone()
            })
        });
        let epoch = self.epoch(id);
        let (store, id) = (self.clone(), id.to_owned());
        let foreground = cx.foreground_executor().clone();
        let run = async move {
            let (mut collected, mut seen) = (Vec::new(), HashSet::<Key>::new());
            let (mut has_more, mut oldest_id) = (true, state.oldest_message_id.clone());
            let mut opts = Some(first);
            for _ in 0..MAX_PAGES {
                let Some(page) = opts.take() else { break };
                let rows = match list(&cx, &id, page).await {
                    Ok(rows) => rows,
                    Err(error) => {
                        log::warn!(target: "chatStore", "loadOlderMessages page fetch failed {id}: {error}");
                        break;
                    }
                };
                if rows.is_empty() {
                    has_more = false;
                    break;
                }
                for row in &rows {
                    if seen.insert(merge::key(row.obj().and_then(|r| r.get("clientId")))) {
                        collected.push(row.clone());
                    }
                }
                let oldest = window::oldest_row(&rows, RowsOrder::NewestFirst);
                if let Some(row) = oldest {
                    oldest_id = row.get("id").and_then(Js::str).map(str::to_owned);
                }
                has_more = window::page_has_more(&rows);
                let prompt = rows.iter().filter_map(Js::obj).any(|r| {
                    r.get("role").and_then(Js::str) == Some("user") && !window::synthetic_trigger(r)
                });
                if !has_more || prompt {
                    break;
                }
                opts = oldest.map(|row| rows::Opts {
                    limit: Some(50.0),
                    before: row.get("id").and_then(Js::str).map(str::to_owned),
                    ..rows::Opts::default()
                });
            }
            if store.epoch(&id) != epoch {
                log::info!(target: "chatStore", "older history of {id} dropped: its window changed");
                return Ok(false);
            }
            if collected.is_empty() {
                store.set(&id, |s| {
                    Some(SessionState {
                        has_more_messages: has_more && s.has_more_messages,
                        is_loading_more: false,
                        ..s.clone()
                    })
                });
                return Ok(false);
            }
            match store.commit_older(&id, &collected, oldest_id, has_more) {
                Ok(advanced) => Ok(advanced),
                Err(error) => {
                    log::warn!(target: "chatStore", "loadOlderMessages commit failed {id}: {error}");
                    if store.epoch(&id) == epoch {
                        store.set(&id, |s| Some(SessionState { is_loading_more: false, ..s.clone() }));
                    }
                    Ok(false)
                }
            }
        }
        .boxed_local()
        .shared();
        // The read outlives its waiter, as a promise does.
        foreground.spawn(run.clone()).detach();
        run.boxed_local()
    }

    /// The commit: merged newest first, the cursor moved, crossed islands absorbed.
    fn commit_older(
        &self,
        id: &str,
        collected: &[Js],
        oldest_id: Option<String>,
        has_more: bool,
    ) -> Result<bool, String> {
        let rows = mapped(collected)?;
        let s = self.get_or_create(id);
        let messages = merge::merge(
            &rows,
            &s.messages,
            &Options::default(),
            RowsOrder::NewestFirst,
        );
        let oldest = oldest_id.or_else(|| s.oldest_message_id.clone());
        let islands = window::absorb(&s.history_window_islands, &messages, oldest.as_deref())?;
        let advanced = messages.len() > s.messages.len() || oldest != s.oldest_message_id;
        log::debug!(target: "chatStore", "older history of {id}: {} rows, advanced {advanced}", rows.len());
        self.set(id, |s| {
            Some(SessionState {
                messages,
                oldest_message_id: oldest,
                has_more_messages: has_more,
                is_loading_more: false,
                history_window_islands: Rc::new(islands),
                ..s.clone()
            })
        });
        Ok(advanced)
    }
}
