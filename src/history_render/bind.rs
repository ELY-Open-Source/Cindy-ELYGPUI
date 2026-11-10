//! `renderHistoryView`'s `bind` with the stream's `structure`.

use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

use indexmap::IndexMap;

use crate::{
    chat_message::merge::Message,
    history_view::{
        Summary,
        controller::{Message as _, Snapshot},
    },
    render_items::{Deferred, Item},
};

pub(super) struct Binder<'a> {
    pub rows: &'a [Message],
    pub placeholders: &'a HashSet<String>,
    pub references: &'a HashMap<String, &'a Summary>,
    pub snapshot: &'a Snapshot<Message>,
}

impl Binder<'_> {
    /// `sourceIds`.
    fn source_ids(&self, item: &Item) -> Vec<&str> {
        let id = |row: &usize| self.rows[*row].client_id();
        match item {
            Item::Message { message, .. } => vec![id(message)],
            Item::ToolSegment { tool_calls, .. } => tool_calls.iter().map(id).collect(),
            Item::AgentTask { tool_call, .. } => vec![id(tool_call)],
            _ => Vec::new(),
        }
    }

    /// A work group without its placeholders, tied to the ranges its children stand for.
    pub fn bind(&self, item: &Rc<Item>) -> Rc<Item> {
        let Item::WorkGroup {
            key,
            children,
            duration_ms,
            is_streaming,
            started_at_ms,
            ..
        } = &**item
        else {
            return item.clone();
        };
        let mut refs: IndexMap<&str, &Summary> = IndexMap::new();
        let mut next = Vec::new();
        for child in children {
            if matches!(**child, Item::WorkGroup { .. }) {
                next.push(self.bind(child));
                continue;
            }
            let ids = self.source_ids(child);
            let found = ids.iter().filter_map(|id| self.references.get(*id));
            for summary in found.filter(|summary| summary.parent_tool_use_id.is_none()) {
                refs.insert(&summary.key, summary);
            }
            if !ids.iter().any(|id| self.placeholders.contains(*id)) {
                next.push(child.clone());
            }
        }
        Rc::new(Item::WorkGroup {
            key: key.clone(),
            children: next,
            duration_ms: *duration_ms,
            is_streaming: *is_streaming,
            started_at_ms: *started_at_ms,
            deferred: (!refs.is_empty()).then(|| self.deferred(refs.into_values().collect())),
        })
    }

    fn deferred(&self, summaries: Vec<&Summary>) -> Deferred {
        let Snapshot {
            expanded, details, ..
        } = self.snapshot;
        let states = summaries.iter().map(|summary| {
            let preview = summary.preview.as_deref();
            let shown = preview.filter(|preview| expanded.contains(&preview.key));
            details.get(&shown.unwrap_or(summary).key)
        });
        let states: Vec<_> = states.collect();
        let join = |part: fn(&Summary) -> &str| {
            let parts: Vec<&str> = summaries.iter().map(|summary| part(summary)).collect();
            parts.join("|")
        };
        let whole = |summary: &&Summary| {
            let preview = summary.preview.as_deref();
            preview.is_some_and(|preview| preview.first_message_id == summary.first_message_id)
        };
        Deferred {
            key: join(|summary| &summary.key),
            expanded: summaries.iter().any(|s| expanded.contains(&s.key)),
            revision: join(|summary| &summary.revision),
            preview_complete: summaries.iter().all(whole),
            live_preview: summaries
                .iter()
                .any(|s| s.is_streaming && s.preview.is_some()),
            awaiting_details: states
                .iter()
                .any(|state| state.is_none_or(|s| !s.complete && s.messages.is_empty())),
            loading: states.iter().any(|state| state.is_some_and(|s| s.loading)),
            failed: states.iter().any(|state| {
                let error = state.and_then(|s| s.error.as_deref());
                error.is_some_and(|error| !error.is_empty())
            }),
            summaries: summaries.into_iter().cloned().collect(),
        }
    }
}
