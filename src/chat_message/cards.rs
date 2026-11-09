//! `mapServerMessages`'s system-card branches, kept in Cindy's order.

use super::{Out, base, bot, meta::review_run};
use crate::{
    localdb::search::mcp_app,
    rich_answer::js::{self, Js, Obj},
};

const CINDY_MAKE_CARD_MARKER: &str = "__cindyMakeCard";

/// `{ ...value }`: an object's fields, an array's or a string's indices, nothing for the rest.
pub fn spread(value: &Js) -> Obj {
    match value {
        Js::Obj(o) => o.clone(),
        Js::Arr(items) => items
            .iter()
            .enumerate()
            .map(|(i, v)| (i.to_string(), v.clone()))
            .collect(),
        Js::Str(s) => s
            .encode_utf16()
            .enumerate()
            .map(|(i, unit)| (i.to_string(), Js::Str(String::from_utf16_lossy(&[unit]))))
            .collect(),
        _ => Obj::new(),
    }
}

/// `value && typeof value === 'object'`.
fn object(value: Option<&Js>) -> Option<&Js> {
    value.filter(|v| matches!(v, Js::Obj(_) | Js::Arr(_)))
}

fn card(row: &Obj, role: &str, kind: &str, data: Js) -> Out {
    let mut out = base(row, &Js::Str(role.into()), Js::Str(String::new()));
    out.insert("isStreaming".into(), Js::Bool(false));
    out.insert("systemCardType".into(), Js::Str(kind.into()));
    out.insert("systemCardData".into(), data);
    out
}

/// The voice call, App and Cindy Make cards, which come before the role branches.
pub fn leading(row: &Obj, meta: Option<&Obj>) -> Result<Option<Out>, String> {
    let assistant = row["role"] == Js::Str("assistant".into());
    let field = |key: &str| meta.and_then(|m| m.get(key));
    if !assistant {
        return Ok(None);
    }
    if let Some(record) = field("voiceCall")
        && record.obj().and_then(|r| r.get("version")) == Some(&Js::Num(1.0))
    {
        let at = row
            .get("createdAt")
            .and_then(Js::str)
            .map_or(f64::NAN, crate::js_date::parse);
        let iso = crate::js_date::iso(at as i64)
            .filter(|_| at.is_finite())
            .ok_or("Invalid time value")?;
        let mut out = card(
            row,
            "assistant",
            "voice-call",
            Js::Obj(Obj::from([("record".into(), record.clone())])),
        );
        out.insert("createdAt".into(), Js::Str(iso));
        return Ok(Some(out));
    }
    let content = &row["content"];
    if field("mcpApp") == Some(&Js::Bool(true))
        && let Some((title, text)) = mcp_app::history(content)
    {
        let mut data = Obj::from([
            ("title".into(), Js::Str(title)),
            ("fallbackText".into(), Js::Str(text)),
        ]);
        let plugin = mcp_app::record(content).and_then(|r| {
            r.get("tool")?
                .obj()?
                .get("_meta")?
                .obj()?
                .get("cindy/plugin")
        });
        if let Some(plugin) = plugin {
            data.insert("plugin".into(), plugin.clone());
        }
        let mut out = card(row, "assistant", "mcp-app", Js::Obj(data));
        out.insert("mcpAppClientId".into(), row["clientId"].clone());
        return Ok(Some(out));
    }
    if let Some(found) = object(content.obj().and_then(|c| c.get(CINDY_MAKE_CARD_MARKER))) {
        let kind =
            match found.obj().and_then(|c| c.get("type")) == Some(&Js::Str("cindy-make".into())) {
                true => "cindy-make",
                false => "cindy-make-doctor",
            };
        let data = object(found.obj().and_then(|c| c.get("data")));
        let data = data.cloned().unwrap_or_else(|| Js::Obj(Obj::new()));
        return Ok(Some(card(row, "assistant", kind, data)));
    }
    Ok(None)
}

fn text_or(value: Option<&Js>, fallback: &str) -> Js {
    Js::Str(value.and_then(Js::str).unwrap_or(fallback).into())
}

/// The model change, import, rebuild, engine switch and bot cards, after the error branch.
pub fn middle(row: &Obj, meta: Option<&Obj>) -> Option<Out> {
    let role = row["role"].str().unwrap_or("");
    let assistant = role == "assistant";
    let field = |key: &str| meta.and_then(|m| m.get(key));
    if assistant && let Some(change) = field("modelChange").filter(|c| js::truthy(Some(c))) {
        return Some(card(
            row,
            "assistant",
            "agent-switch",
            Js::Obj(spread(change)),
        ));
    }
    if assistant && let Some(import) = object(field("sessionImport")) {
        return Some(card(
            row,
            "assistant",
            "session-import",
            Js::Obj(spread(import)),
        ));
    }
    if assistant && let Some(rebuild) = object(field("contextRebuild")) {
        let get = |key: &str| rebuild.obj().and_then(|r| r.get(key));
        let data = Obj::from([
            ("reason".into(), text_or(get("reason"), "context-overflow")),
            ("handoff".into(), text_or(get("handoff"), "")),
        ]);
        return Some(card(row, "assistant", "context-rebuild", Js::Obj(data)));
    }
    if role == "agent_switch" {
        let get = |key: &str| row["content"].obj().and_then(|c| c.get(key));
        let nullable = |key: &str| match get(key) {
            Some(v @ Js::Str(_)) => v.clone(),
            _ => Js::Null,
        };
        let mut data = Obj::from([
            ("fromAgentKind".into(), text_or(get("fromAgentKind"), "")),
            ("toAgentKind".into(), text_or(get("toAgentKind"), "")),
            ("fromModel".into(), nullable("fromModel")),
            ("toModel".into(), nullable("toModel")),
            ("handoff".into(), text_or(get("handoff"), "")),
            (
                "resumed".into(),
                Js::Bool(get("resumed") == Some(&Js::Bool(true))),
            ),
        ]);
        if let Some(v @ Js::Str(s)) = get("nativeHistory")
            && matches!(s.as_str(), "restored" | "fallback")
        {
            data.insert("nativeHistory".into(), v.clone());
        }
        if let Some(v @ (Js::Null | Js::Str(_))) = get("toAgentDeviceId") {
            data.insert("toAgentDeviceId".into(), v.clone());
        }
        if let Some(v @ Js::Str(_)) = get("toAgentDeviceName") {
            data.insert("toAgentDeviceName".into(), v.clone());
        }
        if get("switchReason") == Some(&Js::Str("quota-rebalance".into())) {
            data.insert("switchReason".into(), Js::Str("quota-rebalance".into()));
        }
        return Some(card(row, "assistant", "agent-switch", Js::Obj(data)));
    }
    let collaboration = bot::collaboration(field("botCollaboration"));
    let kind = collaboration.as_ref().and_then(|c| match c["role"].str()? {
        "delegation-request" => Some("bot-session-task"),
        "delegation-result" => Some("bot-session-task-result"),
        "interjection" => Some("bot-session-task-message"),
        _ => None,
    });
    if assistant && let (Some(mut data), Some(kind)) = (collaboration, kind) {
        data.insert("text".into(), Js::Str(String::new()));
        return Some(card(row, role, kind, Js::Obj(data)));
    }
    if assistant && let Some(authorization) = bot::authorization(field("botAuthorization")) {
        return Some(card(
            row,
            role,
            "bot-authorization",
            Js::Obj(authorization.clone()),
        ));
    }
    if assistant && let Some(message) = bot::direct_message(field("botDirectMessage")) {
        return Some(card(row, role, "bot-direct-message", Js::Obj(message)));
    }
    None
}

/// The goal, Cindy Make, plugin route, goal notice and Review cards, after the user branch.
pub fn trailing(row: &Obj, meta: Option<&Obj>) -> Option<Out> {
    if row["role"] != Js::Str("assistant".into()) {
        return None;
    }
    let field = |key: &str| {
        meta.and_then(|m| m.get(key))
            .filter(|v| js::truthy(Some(v)))
    };
    for (key, kind) in [
        ("goalCompletion", "goal-complete"),
        ("cindyMakeCompletion", "cindy-make-complete"),
        ("pluginModelRoute", "plugin-model-route"),
    ] {
        if let Some(value) = field(key) {
            return Some(card(row, "assistant", kind, Js::Obj(spread(value))));
        }
    }
    if let Some(notice) = field("goalNotice") {
        return Some(card(
            row,
            "assistant",
            "goal-resumed",
            Js::Obj(Obj::from([("kind".into(), notice.clone())])),
        ));
    }
    let review = review_run(meta.and_then(|m| m.get("reviewRun")))?;
    let mut data = review.clone();
    let result = row["content"].str().unwrap_or("");
    data.insert("result".into(), Js::Str(result.into()));
    Some(card(row, "assistant", "review", Js::Obj(data)))
}
