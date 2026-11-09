//! The bot readers: `botCollaboration.ts`, `botDirectMessage.ts`, `botAuthorization.ts`, `botStarterOptions.ts`.

use std::{collections::HashSet, sync::LazyLock};

use regex::Regex;

use crate::{
    rich_answer::js::{self, Js, Obj, utf16_len},
    theme_import::color::js_trim,
};

const ROLES: [&str; 6] = [
    "delegation-request",
    "delegation-result",
    "guest-result",
    "interjection",
    "guest-request",
    "result-mirror",
];

/// A property read as JavaScript does it; primitives and arrays hold no named field here.
fn prop<'a>(value: Option<&'a Js>, key: &str) -> Option<&'a Js> {
    value?.obj()?.get(key)
}

fn non_empty_str(value: Option<&Js>) -> bool {
    value.and_then(Js::str).is_some_and(|s| !s.is_empty())
}

fn safe_integer(value: Option<&Js>) -> bool {
    value
        .and_then(Js::integer)
        .is_some_and(|n| n.abs() <= 9_007_199_254_740_991.0)
}

/// `botInterjectionPreview`: the first nonblank line, 200 code points.
fn interjection_preview(text: &str) -> String {
    let line = text
        .split('\n')
        .map(|l| js_trim(l.strip_suffix('\r').unwrap_or(l)))
        .find(|l| !l.is_empty())
        .unwrap_or("");
    line.chars().take(200).collect()
}

fn valid_result(result: Option<&Js>) -> bool {
    let field = |key: &str| prop(result, key);
    let optional_text = |key: &str| field(key).is_none_or(|v| matches!(v, Js::Str(_)));
    js::truthy(result)
        && safe_integer(field("runSequence"))
        && field("runSequence")
            .and_then(Js::integer)
            .is_some_and(|n| n >= 1.0)
        && field("status")
            .and_then(Js::str)
            .is_some_and(|s| ["completed", "failed", "cancelled", "timed-out"].contains(&s))
        && optional_text("workingDir")
        && optional_text("title")
        && optional_text("error")
        && matches!(field("text"), Some(Js::Str(_)))
        && match field("artifacts") {
            Some(Js::Arr(files)) => files.iter().all(|f| {
                js::truthy(Some(f)) && matches!(prop(Some(f), "absolutePath"), Some(Js::Str(_)))
            }),
            _ => false,
        }
}

/// `readBotCollaborationMeta`.
pub fn collaboration(value: Option<&Js>) -> Option<Obj> {
    let raw = value?.obj()?;
    let role = raw.get("role")?.str().filter(|r| ROLES.contains(r))?;
    let to_bot = raw.get("toBotId");
    let ok = raw.get("v") == Some(&Js::Num(1.0))
        && non_empty_str(raw.get("delegationId"))
        && non_empty_str(raw.get("fromBotId"))
        && (to_bot == Some(&Js::Null) || non_empty_str(to_bot))
        && matches!(raw.get("fromBotName"), Some(Js::Str(_)))
        && matches!(raw.get("toBotName"), Some(Js::Str(_)))
        && matches!(raw.get("objective"), Some(Js::Str(_)));
    let id = |key: &str| match raw.get(key) {
        Some(v @ (Js::Null | Js::Str(_))) => Some(v.clone()),
        _ => None,
    };
    let (parent, child) = (id("parentSessionId")?, id("childSessionId")?);
    if !ok || (role == "delegation-result" && !valid_result(raw.get("result"))) {
        return None;
    }
    let mut out = Obj::new();
    out.insert("v".into(), Js::Num(1.0));
    out.insert("role".into(), Js::Str(role.into()));
    for key in [
        "delegationId",
        "fromBotId",
        "fromBotName",
        "toBotId",
        "toBotName",
    ] {
        out.insert(key.into(), raw[key].clone());
    }
    out.insert("parentSessionId".into(), parent);
    out.insert("childSessionId".into(), child);
    if let Some(result) = raw.get("result").filter(|r| js::truthy(Some(r))) {
        out.insert("result".into(), result.clone());
    }
    out.insert("objective".into(), raw["objective"].clone());
    if role == "interjection"
        && let Some(Js::Str(preview)) = raw.get("interjectionPreview")
    {
        let preview = interjection_preview(preview);
        if !preview.is_empty() {
            out.insert("interjectionPreview".into(), Js::Str(preview));
        }
    }
    Some(out)
}

/// `readBotTaskResults`: delegation results, one per run.
pub fn task_results(value: Option<&Js>) -> Vec<Js> {
    let Some(Js::Arr(items)) = value else {
        return Vec::new();
    };
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in items {
        let Some(card) = collaboration(Some(item)) else {
            continue;
        };
        let Some(result) = card
            .get("result")
            .filter(|_| card["role"] == Js::Str("delegation-result".into()))
        else {
            continue;
        };
        let run = js::string(prop(Some(result), "runSequence"));
        let key = format!(
            "bot-delegation-result:{}:{run}",
            js::string(card.get("delegationId"))
        );
        if seen.insert(key) {
            out.push(Js::Obj(card));
        }
    }
    out
}

/// `readBotDirectMessageMeta`.
pub fn direct_message(value: Option<&Js>) -> Option<Obj> {
    let raw = value?.obj()?;
    let ok = raw.get("v") == Some(&Js::Num(1.0))
        && non_empty_str(raw.get("threadId"))
        && non_empty_str(raw.get("viewerBotId"))
        && non_empty_str(raw.get("peerBotId"))
        && matches!(raw.get("peerBotName"), Some(Js::Str(_)))
        && raw
            .get("direction")
            .and_then(Js::str)
            .is_some_and(|d| matches!(d, "sent" | "received"))
        && raw
            .get("sequence")
            .and_then(Js::integer)
            .is_some_and(|n| n >= 1.0)
        && matches!(raw.get("preview"), Some(Js::Str(_)));
    if !ok {
        return None;
    }
    let mut out = Obj::new();
    out.insert("v".into(), Js::Num(1.0));
    for key in [
        "threadId",
        "viewerBotId",
        "peerBotId",
        "peerBotName",
        "direction",
        "sequence",
        "preview",
    ] {
        out.insert(key.into(), raw[key].clone());
    }
    Some(out)
}

static NAMESPACE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-z0-9](?:[a-z0-9-]{0,126}[a-z0-9])?$").expect("valid pattern")
});

/// `isBotAuthorizationHostId`.
fn host_id(id: &str) -> bool {
    id == "grok"
        || id
            .strip_prefix("im-")
            .is_some_and(|c| ["telegram", "feishu", "wecom", "dingtalk", "discord"].contains(&c))
}

const PHASES: [&str; 7] = [
    "pending",
    "action_running",
    "waiting_external",
    "verifying",
    "satisfied",
    "failed",
    "cancelled",
];
const ACTIONS: [&str; 5] = [
    "oauth_connect",
    "open_plugin_settings",
    "manage_connection",
    "open_client_settings",
    "inline_form",
];

/// `fields[0]` and `fields.length`; a string's first unit has no `type`, so it fails as none.
fn first_and_length(fields: Option<&Js>) -> (Option<&Js>, Option<Js>) {
    match fields {
        Some(Js::Arr(items)) => (items.first(), Some(Js::Num(items.len() as f64))),
        Some(Js::Obj(o)) => (o.get("0"), o.get("length").cloned()),
        _ => (None, None),
    }
}

fn valid_step(step: &Js) -> bool {
    let Some(s) = step.obj() else {
        return false;
    };
    let text = |key: &str| matches!(s.get(key), Some(Js::Str(_)));
    if !(text("id")
        && text("groupId")
        && s.get("groupMode") == Some(&Js::Str("any_of".into()))
        && text("title")
        && text("description")
        && s.get("phase")
            .and_then(Js::str)
            .is_some_and(|p| PHASES.contains(&p)))
    {
        return false;
    }
    let action = s.get("action");
    if !js::truthy(action) {
        return true;
    }
    let kind = prop(action, "kind").and_then(Js::str);
    if !matches!(prop(action, "id"), Some(Js::Str(_)))
        || !kind.is_some_and(|k| ACTIONS.contains(&k))
    {
        return false;
    }
    if kind != Some("inline_form") {
        return true;
    }
    let (field, length) = first_and_length(prop(prop(action, "form"), "fields"));
    js::truthy(field)
        && length == Some(Js::Num(1.0))
        && prop(field, "type") == Some(&Js::Str("secret".into()))
        && matches!(prop(field, "label"), Some(Js::Str(_)))
        && matches!(prop(field, "id"), Some(Js::Str(_)))
        && prop(field, "maxLength")
            .and_then(Js::integer)
            .is_some_and(|n| n >= 1.0)
}

/// `readBotAuthorizationCard`: the card itself when every field holds.
pub fn authorization(value: Option<&Js>) -> Option<&Obj> {
    let v = value?.obj()?;
    let target = v.get("target");
    let kind = prop(target, "kind").and_then(Js::str);
    let token = v.get("pluginApprovalToken");
    let snapshot = v.get("snapshot");
    let ghost = prop(snapshot, "ghost");
    let steps = prop(snapshot, "steps").and_then(Js::arr);
    let namespace_ok = match target
        .and_then(Js::obj)
        .filter(|t| t.contains_key("namespace"))
    {
        None => true,
        Some(t) => {
            kind == Some("plugin")
                && match &t["namespace"] {
                    Js::Null => true,
                    Js::Str(n) => NAMESPACE.is_match(n),
                    _ => false,
                }
        }
    };
    let valid = v.get("v") == Some(&Js::Num(1.0))
        && token.is_none_or(|t| kind == Some("plugin") && t.str().is_some_and(|t| !t.is_empty()))
        && matches!(v.get("sessionId"), Some(Js::Str(_)))
        && matches!(v.get("createdAt"), Some(Js::Num(_)))
        && js::truthy(target)
        && kind.is_some_and(|k| matches!(k, "plugin" | "host"))
        && prop(target, "id")
            .and_then(Js::str)
            .is_some_and(|id| kind != Some("host") || host_id(id))
        && namespace_ok
        && js::truthy(snapshot)
        && prop(snapshot, "kind") == Some(&Js::Str("plugin_setup".into()))
        && matches!(prop(snapshot, "requestId"), Some(Js::Str(_)))
        && prop(snapshot, "revision")
            .and_then(Js::integer)
            .is_some_and(|r| r >= 0.0)
        && js::truthy(ghost)
        && matches!(prop(ghost, "name"), Some(Js::Str(_)))
        && matches!(prop(ghost, "id"), Some(Js::Str(_)))
        && steps.is_some_and(|s| s.len() <= 88 && s.iter().all(valid_step));
    valid.then_some(v)
}

/// `readBotStarterOptions`: up to four, only when every option is short and nonblank.
pub fn starter_options(value: Option<&Js>) -> Vec<Js> {
    let Some(Js::Arr(items)) = value else {
        return Vec::new();
    };
    let valid = items.iter().all(|o| {
        o.str()
            .is_some_and(|s| !js_trim(s).is_empty() && utf16_len(s) <= 60)
    });
    match valid {
        true => items.iter().take(4).cloned().collect(),
        false => Vec::new(),
    }
}
