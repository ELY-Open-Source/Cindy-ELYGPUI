//! The controller's work details: expansion, holds, the warm cache and the reads.

use std::{cell::RefCell, rc::Rc};

use futures::FutureExt;
use indexmap::{IndexMap, IndexSet};

use super::{Detail, Item, Message, Patch, Run, View, continuity, settled, summaries};
use crate::history_view::{Summary, reader::Ref};

/// A collapsed range over this many characters is not kept warm.
const LARGE_DETAIL: usize = 1024 * 1024;
const WARM_DETAILS: usize = 32 * 1024 * 1024;

/// What a detail update sets over the loading defaults.
#[derive(Default)]
struct Update {
    loading: Option<bool>,
    complete: bool,
    error: Option<String>,
}

impl<T: Message> View<T> {
    /// `setExpanded`.
    pub fn set_expanded(&self, key: &str, expanded: bool) {
        let state = self.snapshot();
        if state.expanded.contains(key) == expanded {
            return;
        }
        let mut next = state.expanded.clone();
        match expanded {
            true => {
                next.insert(key.to_owned());
            }
            false => {
                next.shift_remove(key);
                self.inner.borrow_mut().detail_runs.shift_remove(key);
            }
        }
        let details = (!expanded).then(|| self.trim_collapsed(&next, &state.items));
        self.publish(
            Patch {
                expanded: Some(next),
                details,
                ..Patch::default()
            },
            None,
        );
        if expanded
            && let Some(summary) = summaries(&state.items).into_iter().find(|s| s.key == key)
        {
            drop(self.load_details(summary.clone(), false));
        }
    }

    /// `retainExpanded`: each key stays open until its last holder lets go.
    pub fn retain_expanded(&self, keys: Vec<String>) -> impl FnOnce() + use<T> {
        for key in &keys {
            *self
                .inner
                .borrow_mut()
                .expanded_holds
                .entry(key.clone())
                .or_insert(0) += 1;
            self.set_expanded(key, true);
        }
        let view = self.clone();
        move || {
            for key in &keys {
                let holds = {
                    let mut inner = view.inner.borrow_mut();
                    let holds = inner.expanded_holds.get(key).copied().unwrap_or(1) - 1;
                    match holds {
                        0 => inner.expanded_holds.shift_remove(key),
                        _ => inner.expanded_holds.insert(key.clone(), holds),
                    };
                    holds
                };
                if holds == 0 {
                    view.set_expanded(key, false);
                }
            }
        }
    }

    /// `trimCollapsedDetails`: open, running and loading ranges stay; others within a budget.
    pub(super) fn trim_collapsed(
        &self,
        expanded: &IndexSet<String>,
        items: &[Item<T>],
    ) -> IndexMap<String, Detail<T>> {
        let inner = self.inner.borrow();
        let mut details = inner.state.details.clone();
        let all = summaries(items);
        let present: IndexSet<&str> = all.iter().map(|s| s.key.as_str()).collect();
        let running: IndexSet<&str> = all
            .iter()
            .filter(|s| s.is_streaming)
            .map(|s| s.key.as_str())
            .collect();
        let mut retained = 0;
        for key in inner.state.details.keys().rev() {
            if !present.contains(key.as_str()) {
                details.shift_remove(key);
                continue;
            }
            let detail = &inner.state.details[key];
            if expanded.contains(key)
                || running.contains(key.as_str())
                || (detail.loading && inner.detail_runs.contains_key(key))
            {
                continue;
            }
            let size: usize = detail.messages.iter().map(Message::characters).sum();
            if size >= LARGE_DETAIL || retained + size > WARM_DETAILS {
                details.shift_remove(key);
            } else {
                retained += size;
            }
        }
        details
    }

    /// `loadDetails`: every page of a range, joined by later callers, reread on a new revision.
    pub fn load_details(&self, summary: Summary, allow_collapsed: bool) -> Run {
        let done = || async {}.boxed_local().shared();
        let existing = {
            let inner = self.inner.borrow();
            if !inner.active || (!allow_collapsed && !inner.state.expanded.contains(&summary.key)) {
                return done();
            }
            if let Some((_, run)) = inner.detail_runs.get(&summary.key) {
                return run.clone();
            }
            inner.state.details.get(&summary.key).cloned()
        };
        if existing
            .as_ref()
            .is_some_and(|d| d.complete && d.revision == summary.revision)
        {
            return done();
        }
        let (token, generation) = {
            let mut inner = self.inner.borrow_mut();
            inner.next_run += 1;
            let token = inner.next_run;
            inner
                .detail_runs
                .insert(summary.key.clone(), (token, done()));
            (token, inner.generation)
        };
        let collected: Rc<RefCell<Vec<T>>> = Rc::default();
        self.update(
            &summary,
            existing.as_ref(),
            &collected.borrow(),
            Update::default(),
        );
        let view = self.clone();
        let key = summary.key.clone();
        let run = async move {
            let current = {
                let view = view.clone();
                let key = summary.key.clone();
                move || view.details_current(&key, token, generation, allow_collapsed)
            };
            let transport = view.transport.clone();
            let r = Ref::from(&summary);
            let read = move |cursor| settled(transport.details(r.clone(), cursor));
            let on_page = |rows: Vec<T>| {
                {
                    let mut all = collected.borrow_mut();
                    let incoming: IndexSet<String> =
                        rows.iter().map(|r| r.client_id().to_owned()).collect();
                    all.retain(|r| !incoming.contains(r.client_id()));
                    all.extend(rows);
                }
                view.update(
                    &summary,
                    existing.as_ref(),
                    &collected.borrow(),
                    Update::default(),
                );
            };
            let result = continuity::work_details(read, &current, on_page).await;
            if current() {
                let update = match result {
                    Ok(()) => Update {
                        loading: Some(false),
                        complete: true,
                        error: None,
                    },
                    Err(error) => Update {
                        loading: Some(false),
                        complete: false,
                        error: Some(error),
                    },
                };
                view.update(&summary, existing.as_ref(), &collected.borrow(), update);
            }
            let ours = view
                .inner
                .borrow()
                .detail_runs
                .get(&summary.key)
                .is_some_and(|(t, _)| *t == token);
            if ours {
                let still = current();
                view.inner
                    .borrow_mut()
                    .detail_runs
                    .shift_remove(&summary.key);
                let items = view.snapshot().items;
                let latest = summaries(&items)
                    .into_iter()
                    .find(|s| s.key == summary.key)
                    .cloned();
                if let Some(latest) = latest.filter(|l| still && l.revision != summary.revision) {
                    view.load_details(latest, false).await;
                }
            }
        }
        .boxed_local()
        .shared();
        if let Some(entry) = self.inner.borrow_mut().detail_runs.get_mut(&key)
            && entry.0 == token
        {
            entry.1 = run.clone();
        }
        // Runs to its first read now and settles unawaited, as an async function does.
        let _ = run.clone().now_or_never();
        self.drive(run.clone());
        run
    }

    fn details_current(
        &self,
        key: &str,
        token: u64,
        generation: u64,
        allow_collapsed: bool,
    ) -> bool {
        let inner = self.inner.borrow();
        inner.active
            && generation == inner.generation
            && inner.detail_runs.get(key).is_some_and(|(t, _)| *t == token)
            && (allow_collapsed || inner.state.expanded.contains(key))
    }

    /// `update`: a reread keeps the previous window until it settles.
    fn update(
        &self,
        summary: &Summary,
        existing: Option<&Detail<T>>,
        collected: &[T],
        patch: Update,
    ) {
        let mut details = self.inner.borrow().state.details.clone();
        if let Some(completed) = details.get(&summary.key)
            && completed.complete
            && completed.revision == summary.revision
            && !patch.complete
        {
            return;
        }
        let kept = existing.map(|e| e.messages.clone()).unwrap_or_default();
        let messages = match (patch.complete, kept.is_empty(), collected.is_empty()) {
            (false, false, _) => kept,
            (_, _, false) => collected.to_vec(),
            _ => kept,
        };
        let detail = Detail {
            messages,
            revision: summary.revision.clone(),
            last_message_id: summary.last_message_id.clone(),
            loading: patch.loading.unwrap_or(true),
            complete: patch.complete,
            error: patch.error,
        };
        details.insert(summary.key.clone(), detail);
        let source = patch.complete.then_some("details");
        self.publish(
            Patch {
                details: Some(details),
                ..Patch::default()
            },
            source,
        );
    }
}
