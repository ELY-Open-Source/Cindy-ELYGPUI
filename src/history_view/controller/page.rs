//! The page reads: `refresh`, `invalidate` and `readPage` with its settle and failure.

use std::time::Duration;

use futures::{FutureExt, future::LocalBoxFuture};
use indexmap::{IndexMap, IndexSet};

use super::{Message, Patch, Run, View, continuity, settled, summaries, unavailable};
use crate::history_view::reader::Page;

impl<T: Message> View<T> {
    /// `refresh`: one page read at a time; a caller of another direction or generation waits.
    pub fn refresh(&self, older: bool, fresh: bool) -> Run {
        let (running, generation) = {
            let inner = self.inner.borrow();
            if inner
                .state
                .error
                .as_deref()
                .is_some_and(|e| e.contains("UNSUPPORTED_CAPABILITY"))
            {
                return async {}.boxed_local().shared();
            }
            (
                inner
                    .running_page()
                    .map(|r| (r, inner.page_older, inner.page_generation)),
                inner.generation,
            )
        };
        if let Some((run, page_older, page_generation)) = running {
            if !fresh && older == page_older && page_generation == generation {
                return run;
            }
            let view = self.clone();
            let after = async move {
                run.await;
                let again = {
                    let inner = view.inner.borrow();
                    inner.active
                        && generation == inner.generation
                        && (fresh || inner.state.error.is_none())
                };
                if again {
                    view.refresh(older, false).await;
                }
            }
            .boxed_local()
            .shared();
            self.drive(after.clone());
            return after;
        }
        {
            let mut inner = self.inner.borrow_mut();
            inner.page_older = older;
            inner.page_generation = generation;
        }
        let run = self.read_page(older).boxed_local().shared();
        self.inner.borrow_mut().page_run = Some(run.clone());
        self.drive(run.clone());
        run
    }

    /// `invalidate`: activity notices coalesce into one read 500ms later.
    pub fn invalidate(&self) {
        if !self.is_active() || self.inner.borrow().refresh_timer.is_some() {
            return;
        }
        let view = self.clone();
        let timer = self.background.timer(Duration::from_millis(500));
        let task = self.foreground.spawn(async move {
            timer.await;
            let running = {
                let mut inner = view.inner.borrow_mut();
                // Dropping its own handle would cancel this task.
                if let Some(me) = inner.refresh_timer.take() {
                    me.detach();
                }
                inner.running_page()
            };
            match running {
                Some(run) => {
                    run.await;
                    view.invalidate();
                }
                None => {
                    view.refresh(false, false).await;
                }
            }
        });
        self.inner.borrow_mut().refresh_timer = Some(task);
    }

    /// `readPage`: the loading flag is set before the first await, as Cindy's is.
    fn read_page(&self, older: bool) -> LocalBoxFuture<'static, ()> {
        let (generation, before) = {
            let inner = self.inner.borrow();
            let s = &inner.state;
            if !inner.active || s.loading || (older && !s.has_more) {
                return async {}.boxed_local();
            }
            (
                inner.generation,
                older.then(|| s.next_cursor.clone()).flatten(),
            )
        };
        let started = crate::js_date::now();
        if !older {
            self.inner.borrow_mut().latest_read_started_ms = started;
        }
        self.publish(
            Patch {
                loading: Some(true),
                error: Some(None),
                ..Patch::default()
            },
            None,
        );
        let view = self.clone();
        let page = settled(self.transport.page(before));
        async move {
            let result = view
                .settle_page(page.await, older, generation, started)
                .await;
            if let Err(error) = result {
                view.fail_page(error, generation);
            }
        }
        .boxed_local()
    }

    pub(super) fn current(&self, generation: u64) -> bool {
        let inner = self.inner.borrow();
        inner.active && generation == inner.generation
    }

    async fn settle_page(
        &self,
        page: Result<Page<T>, String>,
        older: bool,
        generation: u64,
        started: f64,
    ) -> Result<(), String> {
        let page = page?;
        if !self.current(generation) {
            return Ok(());
        }
        let previous = self.snapshot();
        let retained = match older {
            true => page,
            false => {
                let transport = self.transport.clone();
                let read = move |cursor: String| settled(transport.page(Some(cursor)));
                let view = self.clone();
                let Some(page) =
                    continuity::continuous(page, &previous, read, move || view.current(generation))
                        .await?
                else {
                    return Ok(());
                };
                continuity::retain_prefix(page, &self.snapshot())
            }
        };
        let mut items = retained.items;
        let state = self.snapshot();
        if older {
            let keys: std::collections::HashSet<String> =
                items.iter().map(|i| i.key().to_owned()).collect();
            items.extend(
                state
                    .items
                    .iter()
                    .filter(|i| !keys.contains(i.key()))
                    .cloned(),
            );
        }
        let present: IndexSet<String> = summaries(&items).iter().map(|s| s.key.clone()).collect();
        self.inner
            .borrow_mut()
            .detail_runs
            .retain(|key, _| present.contains(key));
        let details = self.trim_collapsed(&state.expanded, &items);
        self.publish(
            Patch {
                items: Some(items.clone()),
                ready: Some(true),
                loading: Some(false),
                details: Some(details),
                stale: (!older).then_some(false),
                has_more: Some(retained.has_more),
                next_cursor: Some(retained.next_cursor),
                ..Patch::default()
            },
            Some(if older { "older" } else { "latest" }),
        );
        if !older {
            self.inner.borrow_mut().latest_read_ok_started_ms = started;
        }
        let expanded = self.snapshot().expanded;
        for summary in summaries(&items) {
            if expanded.contains(&summary.key) {
                drop(self.load_details(summary.clone(), false));
            }
        }
        Ok(())
    }

    fn fail_page(&self, error: String, generation: u64) {
        if !self.current(generation) {
            return;
        }
        if !unavailable(&error) {
            self.publish(
                Patch {
                    loading: Some(false),
                    error: Some(Some(error)),
                    ..Patch::default()
                },
                None,
            );
            return;
        }
        log::warn!(target: "history", "history view unavailable: {error}");
        {
            let mut inner = self.inner.borrow_mut();
            inner.generation += 1;
            inner.detail_runs.clear();
            inner.refresh_timer = None;
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
                error: Some(Some(error)),
                ..Patch::default()
            },
            Some("fallback"),
        );
    }
}
