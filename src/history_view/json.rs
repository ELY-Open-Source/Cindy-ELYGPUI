//! Items as the history page's JSON writes them, in Cindy's key order.

use super::{Item, Summary};
use crate::rich_answer::js::{Js, Obj};

fn put(out: &mut Obj, key: &str, value: Js) {
    out.insert(key.into(), value);
}

fn string(text: &str) -> Js {
    Js::Str(text.to_owned())
}

/// A `HistoryWorkSummary` object.
pub fn summary(s: &Summary) -> Js {
    let mut out = Obj::new();
    put(&mut out, "key", string(&s.key));
    put(&mut out, "anchorClientId", string(&s.anchor_client_id));
    if let Some(id) = &s.parent_tool_use_id {
        put(&mut out, "parentToolUseId", string(id));
    }
    if let Some(model) = &s.model {
        put(&mut out, "model", string(model));
    }
    put(&mut out, "firstMessageId", string(&s.first_message_id));
    put(&mut out, "lastMessageId", string(&s.last_message_id));
    if let Some(stored) = &s.stored {
        if let Some(first) = &stored.first {
            put(&mut out, "firstStoredMessageId", string(first));
        }
        if let Some(last) = &stored.last {
            put(&mut out, "lastStoredMessageId", string(last));
        }
        put(
            &mut out,
            "liveMessageIds",
            Js::Arr(stored.live.iter().map(|i| string(i)).collect()),
        );
    }
    put(&mut out, "startedAtMs", Js::Num(s.started_at_ms));
    put(&mut out, "endedAtMs", Js::Num(s.ended_at_ms));
    if let Some(artifacts) = &s.artifacts {
        put(&mut out, "artifacts", Js::Arr(artifacts.clone()));
    }
    put(&mut out, "messageCount", Js::Num(s.message_count as f64));
    put(&mut out, "toolCount", Js::Num(s.tool_count as f64));
    put(&mut out, "isStreaming", Js::Bool(s.is_streaming));
    put(&mut out, "revision", string(&s.revision));
    if let Some(preview) = &s.preview {
        put(&mut out, "preview", summary(preview));
    }
    Js::Obj(out)
}

/// A `HistoryViewItem` object.
pub fn item(item: &Item) -> Js {
    let mut out = Obj::new();
    match item {
        Item::Messages {
            key,
            messages,
            deferred,
        } => {
            put(&mut out, "type", string("messages"));
            put(&mut out, "key", string(key));
            put(&mut out, "messages", Js::Arr(messages.clone()));
            if let Some(deferred) = deferred {
                put(&mut out, "deferred", summary(deferred));
            }
        }
        Item::Work {
            key,
            summary: s,
            children,
        } => {
            put(&mut out, "type", string("work"));
            put(&mut out, "key", string(key));
            put(&mut out, "summary", summary(s));
            if let Some(children) = children {
                put(
                    &mut out,
                    "children",
                    Js::Arr(children.iter().map(self::item).collect()),
                );
            }
        }
    }
    Js::Obj(out)
}
