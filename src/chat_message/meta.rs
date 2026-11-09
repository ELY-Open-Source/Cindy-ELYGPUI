//! Small agent-meta readers: Review runs, voice tasks, shared authors, automation origins.

use crate::{
    rich_answer::js::{Js, Obj, utf16_len},
    theme_import::color::js_trim,
};

const FAILURE_CODES: [&str; 10] = [
    "no-visible-result",
    "reviewer-closed",
    "cancelled-before-start",
    "interrupted",
    "source-workspace-changed",
    "source-conversation-changed",
    "source-files-changed",
    "artifact-changed",
    "artifact-unavailable",
    "provider-failed",
];

fn safe_positive(value: Option<&Js>) -> Option<f64> {
    value?
        .integer()
        .filter(|n| n.abs() <= 9_007_199_254_740_991.0 && *n > 0.0)
}

/// `readReviewRunOwner`.
fn review_owner(value: &Js) -> bool {
    let Some(owner) = value.obj() else {
        return false;
    };
    let named = owner
        .get("instanceId")
        .and_then(Js::str)
        .is_some_and(|i| !i.is_empty());
    if !named || safe_positive(owner.get("processId")).is_none() {
        return false;
    }
    let Some(liveness) = owner.get("liveness") else {
        return true;
    };
    let Some(l) = liveness.obj() else {
        return false;
    };
    let token = l.get("token").and_then(Js::str).map(utf16_len);
    l.get("version") == Some(&Js::Num(1.0))
        && safe_positive(l.get("port")).is_some_and(|p| p <= 65_535.0)
        && token.is_some_and(|t| (16..=256).contains(&t))
}

/// `readReviewRunMeta`: the record itself when it holds.
pub fn review_run(value: Option<&Js>) -> Option<&Obj> {
    let r = value?.obj()?;
    let text = |key: &str| matches!(r.get(key), Some(Js::Str(_)));
    let one_of = |key: &str, set: &[&str]| {
        r.get(key)
            .and_then(Js::str)
            .is_some_and(|v| set.contains(&v))
    };
    let valid = r.get("version") == Some(&Js::Num(1.0))
        && text("runId")
        && text("sourceSessionId")
        && one_of("status", &["running", "completed", "failed"])
        && one_of("targetKind", &["changes", "artifacts", "task", "mixed"])
        && matches!(r.get("startedAt"), Some(Js::Num(_)))
        && r.get("reviewerSessionId")
            .is_none_or(|s| s.str().is_some_and(|s| !s.is_empty()))
        && r.get("owner").is_none_or(review_owner)
        && r.get("failureCode")
            .is_none_or(|c| c.str().is_some_and(|c| FAILURE_CODES.contains(&c)));
    valid.then_some(r)
}

/// `isVoiceCallTaskMessage`: a voice handoff's own task row.
pub fn voice_call_task(
    client_id: &str,
    session_id: Option<&Js>,
    text: &str,
    receipt: Option<&Js>,
) -> bool {
    let Some(source) = receipt
        .and_then(Js::obj)
        .filter(|r| r.get("kind") == Some(&Js::Str("agent-request".into())))
        .and_then(|r| r.get("request"))
        .and_then(Js::obj)
    else {
        return false;
    };
    let Some(source_client) = source
        .get("clientId")
        .and_then(Js::str)
        .filter(|c| !c.is_empty())
    else {
        return false;
    };
    if source.get("sessionId") != session_id
        || source.get("userText") != Some(&Js::Str(text.into()))
    {
        return false;
    }
    let suffix = format!(":{source_client}");
    client_id.starts_with("voice-task:")
        && client_id.ends_with(&suffix)
        && utf16_len(client_id) > "voice-task:".len() + utf16_len(&suffix)
}

/// `value.slice(0, 128)` in UTF-16 units.
fn head(text: &str) -> String {
    let units: Vec<u16> = text.encode_utf16().take(128).collect();
    String::from_utf16_lossy(&units)
}

fn author(meta: Option<&Obj>) -> Option<&Obj> {
    meta?.get("sharedTaskAuthor")?.obj()
}

/// `sharedTaskAuthorName`.
pub fn shared_author_name(meta: Option<&Obj>) -> Option<String> {
    author(meta)?.get("displayName")?.str().map(head)
}

/// `sharedTaskAuthorMemberId`.
pub fn shared_author_member(meta: Option<&Obj>) -> Option<String> {
    let id = js_trim(author(meta)?.get("memberId")?.str()?);
    (!id.is_empty()).then(|| head(id))
}

fn trimmed(value: Option<&Js>) -> Option<&str> {
    let text = js_trim(value?.str()?);
    (!text.is_empty()).then_some(text)
}

/// `toMessageAutomationOrigin`.
pub fn automation_origin(value: Option<&Js>) -> Option<Js> {
    let origin = value?;
    let get = |key: &str| origin.obj().and_then(|o| o.get(key));
    let kind = get("kind").and_then(Js::str);
    if kind == Some("scheduler") {
        return Some(origin.clone());
    }
    let mut out = Obj::new();
    out.insert("kind".into(), Js::Str("session".into()));
    let sender = trimmed(get("senderSessionId"));
    match kind {
        Some("orca") => {
            let label = trimmed(get("senderLabel"));
            if sender.is_none() && label.is_none() {
                return None;
            }
            out.insert("orca".into(), Js::Bool(true));
            if let Some(sender) = sender {
                out.insert("senderSessionId".into(), Js::Str(sender.into()));
            }
            if let Some(label) = label {
                out.insert("orcaSenderLabel".into(), Js::Str(label.into()));
            }
        }
        Some("session") => {
            let Some(sender) = sender else {
                return Some(Js::Obj(out));
            };
            out.insert("senderSessionId".into(), Js::Str(sender.into()));
            if let Some(title) = trimmed(get("senderSessionTitle")) {
                out.insert("senderSessionTitle".into(), Js::Str(title.into()));
            }
            if let Some(bot) = trimmed(get("senderBotId")) {
                out.insert("senderBotId".into(), Js::Str(bot.into()));
                if let Some(name) = trimmed(get("senderBotName")) {
                    out.insert("senderBotName".into(), Js::Str(name.into()));
                }
            }
        }
        _ => return None,
    }
    Some(Js::Obj(out))
}
