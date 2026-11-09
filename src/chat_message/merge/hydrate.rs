//! `hydratePersistedMessage`: a persisted row laid over the one the store holds.

use std::rc::Rc;

use super::{Fields, Message, Options, field, get, is_str, is_true, same, strict_field};
use crate::rich_answer::js::{self, Js};

const REMOTE_CONTENT_TRUNCATED_PLACEHOLDER: &str = "[remote content truncated: payload too large]";
/// V8's `String::kMaxLength`: a longer `JSON.stringify` throws, so the field counts 0.
const MAX_STRING_LENGTH: usize = (1 << 29) - 24;
const SENDER: [&str; 7] = [
    "automationOrigin",
    "sourceDevice",
    "sourcePlugin",
    "sourceGroup",
    "hookSource",
    "sharedAuthorName",
    "sharedAuthorMemberId",
];

/// `existing` itself when the result is shallow-equal to it.
pub fn hydrate(existing: &Message, persisted: &Message, options: &Options) -> Message {
    let (e, p) = (&**existing, &**persisted);
    let mut h = e.clone();
    for (key, value) in p {
        h.insert(key.clone(), value.clone());
    }
    for key in ["retryFiles", "retryMentions"] {
        if js::truthy(get(e, key)) {
            h.insert(key.into(), field(e, key));
        }
    }
    for key in SENDER {
        if get(p, key).is_none() {
            h.shift_remove(key);
        }
    }
    if options.preserve_codex_plan_content && codex_plan(e, p) {
        h.insert("toolInput".into(), field(e, "toolInput"));
        h.insert("content".into(), field(e, "content"));
    }
    if both(e, p, "ask_user") && live(get(e, "askUserStatus"), get(p, "askUserStatus")) {
        for key in ["askUserStatus", "askUserReply", "askUserAnswers"] {
            h.insert(key.into(), field(e, key));
        }
    }
    if both(e, p, "plan_review") && live_plan_review(e, p) {
        for key in [
            "planReviewStatus",
            "planReviewFeedback",
            "planReviewPlan",
            "planReviewFilePath",
        ] {
            h.insert(key.into(), field(e, key));
        }
    }
    let preserved = preserve_content(e, p, options);
    if preserved {
        h.insert("content".into(), field(e, "content"));
        if both(e, p, "tool_use") && is_true(p, "remoteContentTruncated") {
            for key in ["toolInput", "toolName", "toolUseId"] {
                h.insert(key.into(), field(e, key));
            }
        }
    }
    if !is_true(p, "remoteContentTruncated") || (preserved && !is_true(e, "remoteContentTruncated"))
    {
        h.shift_remove("remoteContentTruncated");
    }
    if !is_true(p, "remoteRowsTrimmed") {
        h.shift_remove("remoteRowsTrimmed");
    }
    h.shift_remove("isPendingPersist");
    match shallow_equal(e, &h) {
        true => existing.clone(),
        false => Rc::new(h),
    }
}

fn both(e: &Fields, p: &Fields, role: &str) -> bool {
    is_str(e, "role", role) && is_str(p, "role", role)
}

fn plan_array(m: &Fields) -> bool {
    match get(m, "toolInput") {
        Some(Js::Obj(input)) => matches!(input.get("plan"), Some(Js::Arr(_))),
        _ => false,
    }
}

/// A live Codex plan only Main's done-boundary stamp may close.
fn codex_plan(e: &Fields, p: &Fields) -> bool {
    both(e, p, "tool_use")
        && is_str(e, "toolName", "update_plan")
        && is_str(p, "toolName", "update_plan")
        && strict_field(e, p, "toolUseId")
        && plan_array(e)
        && plan_array(p)
        && !is_true(p, "terminalPlanSnapshot")
}

/// `shouldPreserveLiveAskUserState`, which `shouldPreserveLivePlanReviewState` repeats.
fn live(existing: Option<&Js>, persisted: Option<&Js>) -> bool {
    if !js::truthy(existing) {
        return false;
    }
    let is = |v: Option<&Js>, s: &str| v.and_then(Js::str) == Some(s);
    ["expired", "pending"]
        .into_iter()
        .find(|s| is(persisted, s))
        .is_some_and(|s| !is(existing, s))
}

fn live_plan_review(e: &Fields, p: &Fields) -> bool {
    live(get(e, "planReviewStatus"), get(p, "planReviewStatus"))
        || (is_str(e, "planReviewStatus", "pending")
            && is_str(p, "planReviewStatus", "pending")
            && strict_field(e, p, "planReviewRequestId"))
}

fn placeholder_only(m: &Fields) -> bool {
    is_true(m, "remoteContentTruncated")
        && is_str(m, "content", REMOTE_CONTENT_TRUNCATED_PLACEHOLDER)
}

/// `remoteTruncatedMessageReadableSize`: the stringified length of what a truncation cuts.
fn readable_size(m: &Fields) -> usize {
    ["content", "toolInput", "toolName", "toolUseId"]
        .into_iter()
        .filter_map(|key| get(m, key).filter(|v| !matches!(v, Js::Null)))
        .map(js::stringify_len)
        .map(|n| if n > MAX_STRING_LENGTH { 0 } else { n })
        .sum()
}

fn preserve_content(e: &Fields, p: &Fields, options: &Options) -> bool {
    if matches!(get(e, "content"), None | Some(Js::Null)) || !strict_field(e, p, "role") {
        return false;
    }
    if is_true(p, "remoteContentTruncated") {
        return !placeholder_only(e)
            && (placeholder_only(p)
                || !is_true(e, "remoteContentTruncated")
                || readable_size(e) >= readable_size(p));
    }
    both(e, p, "tool_result") && options.preserve_tool_result_content
}

/// `shallowEqualChatMessage`: the same key count and every value `Object.is`.
fn shallow_equal(a: &Fields, b: &Fields) -> bool {
    a.len() == b.len()
        && a.iter()
            .all(|(key, value)| same(value.as_ref(), b.get(key).and_then(Option::as_ref), false))
}
