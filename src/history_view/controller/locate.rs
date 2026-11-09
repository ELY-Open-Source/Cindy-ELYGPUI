//! `locate`: a row by its client id, through visible pages and folded ranges.

use std::collections::HashSet;

use super::{Detail, Item, Message, Patch, View, continuity, leaves, settled, summaries};
use crate::history_view::{Summary, reader::Ref};

impl<T: Message> View<T> {
    /// Still the same range in the current generation.
    fn same_range(&self, summary: &Summary, generation: u64) -> bool {
        let inner = self.inner.borrow();
        inner.active
            && generation == inner.generation
            && summaries(&inner.state.items).iter().any(|v| {
                v.key == summary.key
                    && v.revision == summary.revision
                    && v.first_message_id == summary.first_message_id
                    && v.last_message_id == summary.last_message_id
            })
    }

    /// `locate`: reads a folded range without opening it; older pages until none remain.
    pub async fn locate(&self, client_id: &str, created_at: &str) -> Result<Option<T>, String> {
        let generation = self.inner.borrow().generation;
        let target = crate::js_date::parse(created_at);
        let mut tried = HashSet::new();
        while self.current(generation) {
            let state = self.snapshot();
            for item in leaves(&state.items) {
                if let Item::Messages { messages, .. } = item
                    && let Some(found) = messages.iter().find(|r| r.client_id() == client_id)
                {
                    return Ok(Some(found.clone()));
                }
                let summary = match item {
                    Item::Work { summary, .. } => Some(summary),
                    Item::Messages { deferred, .. } => deferred.as_ref(),
                };
                let Some(summary) = summary.filter(|s| {
                    !tried.contains(&s.key) && target >= s.started_at_ms && target <= s.ended_at_ms
                }) else {
                    continue;
                };
                tried.insert(summary.key.clone());
                let cached = self.snapshot().details.get(&summary.key).cloned();
                let mut messages = match cached {
                    Some(d) if d.complete && d.revision == summary.revision => d.messages,
                    _ => Vec::new(),
                };
                if messages.is_empty() {
                    let transport = self.transport.clone();
                    let r = Ref::from(summary);
                    let read = move |cursor| settled(transport.details(r.clone(), cursor));
                    let current = || self.same_range(summary, generation);
                    let result =
                        continuity::work_details(read, current, |rows| messages.extend(rows)).await;
                    if !self.same_range(summary, generation) {
                        return Ok(None);
                    }
                    result?;
                    let mut details = self.snapshot().details;
                    let detail = Detail {
                        messages: messages.clone(),
                        revision: summary.revision.clone(),
                        last_message_id: summary.last_message_id.clone(),
                        loading: false,
                        complete: true,
                        error: None,
                    };
                    details.insert(summary.key.clone(), detail);
                    self.publish(
                        Patch {
                            details: Some(details),
                            ..Patch::default()
                        },
                        None,
                    );
                }
                if !self.same_range(summary, generation) {
                    return Ok(None);
                }
                if let Some(found) = messages.into_iter().find(|r| r.client_id() == client_id) {
                    return Ok(Some(found));
                }
            }
            if !self.snapshot().has_more {
                return Ok(None);
            }
            self.refresh(true, false).await;
            if let Some(error) = self.snapshot().error {
                return Err(error);
            }
        }
        Ok(None)
    }
}
