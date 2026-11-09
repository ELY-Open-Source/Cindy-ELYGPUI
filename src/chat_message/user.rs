//! `mapServerMessages`'s user branch: synthetic triggers, auto-resume and the user's own turn.

use std::sync::LazyLock;

use regex::Regex;

use super::{Out, base, cards::spread, meta, parent, source, user_content};
use crate::{
    message_rows::{self, UI_ACTION_TRIGGER_PREFIX},
    rich_answer::js::{self, Js, Obj},
    theme_import::color::{JS_SPACE, js_trim},
};

const INPUT_HANDOFF_ENDED_PREFIX: &str = "[Input handoff ended]";

/// `readInputHandoffEndRecord`'s text: the raw string or the envelope's `text`.
fn row_text(content: &Js) -> Option<&str> {
    match content {
        Js::Str(s) => Some(s),
        Js::Obj(o) => o.get("text").and_then(Js::str),
        _ => None,
    }
}

/// `isSyntheticTriggerRow`.
pub fn synthetic(row: &Obj) -> bool {
    row["role"] == Js::Str("user".into())
        && row_text(&row["content"]).is_some_and(|t| t.starts_with(UI_ACTION_TRIGGER_PREFIX))
}

static STATUS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r" status=([A-Za-z0-9_]+)").expect("valid pattern"));
static STEPS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r" steps=([0-9]+)").expect("valid pattern"));
static ORIGIN: LazyLock<Regex> = LazyLock::new(|| {
    let space = JS_SPACE.trim_start_matches('[').trim_end_matches(']');
    Regex::new(&format!(r#" origin=([^{space}"]+)"#)).expect("valid pattern")
});
static APP: LazyLock<Regex> = LazyLock::new(|| quoted_pattern("app"));
static ANSWERED_NAME: LazyLock<Regex> = LazyLock::new(|| quoted_pattern("answered_on"));

/// `(?:^| )key=("(?:[^"\\]|\\.)*")`, with JavaScript's `.` that stops at line terminators.
fn quoted_pattern(key: &str) -> Regex {
    Regex::new(&format!(
        r#"(?:^| ){key}=("(?:[^"\\]|\\[^\n\r\x{{2028}}\x{{2029}}])*")"#
    ))
    .expect("valid pattern")
}
static ANSWERED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r" answered_on=(this_computer|phone|another_device)(?:[^A-Za-z0-9_]|$)")
        .expect("valid pattern")
});
static NOTHING_TO_FILL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r" failure=nothing_to_fill(?:[^A-Za-z0-9_]|$)").expect("valid pattern")
});

/// `readQuoted`: a JSON string after `key=`, trimmed to 200 units.
fn quoted(text: &str, pattern: &Regex) -> Option<String> {
    let value = js::parse(pattern.captures(text)?.get(1)?.as_str()).ok()?;
    let trimmed = js_trim(value.str()?);
    let units: Vec<u16> = trimmed.encode_utf16().take(200).collect();
    (!trimmed.is_empty()).then(|| String::from_utf16_lossy(&units))
}

/// `parseInputHandoffEndRecord`.
fn handoff_record(content: &Js) -> Option<Obj> {
    let text = row_text(content)?;
    let text = text.strip_prefix(UI_ACTION_TRIGGER_PREFIX).unwrap_or(text);
    let facts = text.strip_prefix(INPUT_HANDOFF_ENDED_PREFIX)?;
    let status = STATUS.captures(facts)?.get(1)?.as_str();
    let steps = STEPS
        .captures(facts)
        .map_or(f64::NAN, |c| crate::cindy_json::js_to_number(&c[1]));
    if !["succeeded", "cancelled", "failed", "expired"].contains(&status)
        || !(steps.is_finite() && steps <= 9_007_199_254_740_991.0)
    {
        return None;
    }
    let mut out = Obj::new();
    out.insert("status".into(), Js::Str(status.into()));
    out.insert(
        "nothingToFill".into(),
        Js::Bool(NOTHING_TO_FILL.is_match(facts)),
    );
    out.insert("steps".into(), Js::Num(steps));
    let origin = ORIGIN
        .captures(facts)
        .map(|c| c[1].strip_suffix('.').unwrap_or(&c[1]).to_owned());
    if let Some(origin) = origin.filter(|o| !o.is_empty()) {
        let units: Vec<u16> = origin.encode_utf16().take(300).collect();
        out.insert("origin".into(), Js::Str(String::from_utf16_lossy(&units)));
    }
    if let Some(app) = quoted(facts, &APP) {
        out.insert("app".into(), Js::Str(app));
    }
    let answered = ANSWERED.captures(facts).map(|c| c[1].to_owned());
    let on = match answered {
        Some(kind) => Some(Obj::from([("kind".into(), Js::Str(kind))])),
        None => quoted(facts, &ANSWERED_NAME).map(|name| {
            Obj::from([
                ("kind".into(), Js::Str("named".into())),
                ("name".into(), Js::Str(name)),
            ])
        }),
    };
    if let Some(on) = on {
        out.insert("answeredOn".into(), Js::Obj(on));
    }
    Some(out)
}

fn auto_resume_data(meta: &Obj) -> Obj {
    let mut data = meta
        .get("autoResumeInfo")
        .map_or_else(Obj::new, |i| match i {
            Js::Null => Obj::new(),
            info => spread(info),
        });
    if let Some(outcome) = meta
        .get("autoResumeOutcome")
        .filter(|o| js::truthy(Some(o)))
    {
        data.insert("outcome".into(), outcome.clone());
    }
    data
}

fn system(out: &mut Out, kind: &str, data: Obj) {
    out.insert("systemCardType".into(), Js::Str(kind.into()));
    out.insert("systemCardData".into(), Js::Obj(data));
}

fn trigger(row: &Obj, meta: Option<&Obj>) -> Out {
    let mut out = base(row, &row["role"], Js::Str(String::new()));
    out.insert("isStreaming".into(), Js::Bool(false));
    out.insert("isSyntheticTrigger".into(), Js::Bool(true));
    let auto = meta.and_then(|m| m.get("autoResume")) == Some(&Js::Bool(true));
    let continuation =
        message_rows::is_continuation_message("user", &row["content"], row.get("agentMeta"));
    if !auto && let Some(record) = handoff_record(&row["content"]) {
        out.insert("isContinuationTrigger".into(), Js::Bool(continuation));
        system(&mut out, "input-handoff", record);
        return out;
    }
    out.insert("isContinuationTrigger".into(), Js::Bool(continuation));
    if let Some(meta) = meta.filter(|_| auto) {
        system(&mut out, "auto-resume", auto_resume_data(meta));
    }
    out
}

pub fn user(row: &Obj, meta: Option<&Obj>) -> Out {
    if synthetic(row) {
        return trigger(row, meta);
    }
    let field = |key: &str| meta.and_then(|m| m.get(key));
    if let Some(meta) = meta.filter(|_| field("autoResume") == Some(&Js::Bool(true))) {
        let mut out = base(row, &row["role"], Js::Str(String::new()));
        out.insert("isStreaming".into(), Js::Bool(false));
        out.insert("isSyntheticTrigger".into(), Js::Bool(true));
        out.insert("systemCardType".into(), Js::Str("auto-resume".into()));
        out.insert("isContinuationTrigger".into(), Js::Bool(true));
        out.insert("systemCardData".into(), Js::Obj(auto_resume_data(meta)));
        return out;
    }
    let parsed = user_content::parse(&row["content"]);
    let mut out = Out::new();
    if let Some(name) = meta::shared_author_name(meta) {
        out.insert("sharedAuthorName".into(), Js::Str(name));
    }
    if let Some(member) = meta::shared_author_member(meta) {
        out.insert("sharedAuthorMemberId".into(), Js::Str(member));
    }
    out.extend(base(row, &row["role"], Js::Str(parsed.text.clone())));
    out.insert("isStreaming".into(), Js::Bool(false));
    let client = row["clientId"].str().unwrap_or("");
    if meta::voice_call_task(
        client,
        row.get("sessionId"),
        &parsed.text,
        field("autoReviewUserText"),
    ) {
        let data = Obj::from([("text".into(), Js::Str(parsed.text.clone()))]);
        system(&mut out, "voice-call-task", data);
    }
    let info = field("autoResumeInfo");
    if info.and_then(Js::obj).and_then(|i| i.get("reason"))
        == Some(&Js::Str("usage-limit-reset".into()))
    {
        out.insert(
            "usageLimitAutoResumeInfo".into(),
            Js::Obj(spread(info.expect("checked"))),
        );
    }
    let lists = [("images", parsed.images), ("files", parsed.files)];
    for (key, list) in lists {
        if !list.is_empty() {
            out.insert(key.into(), Js::Arr(list));
        }
    }
    if parsed.quotes_encoded {
        out.insert("quotesEncoded".into(), Js::Bool(true));
    }
    if let Some(quote) = parsed.reply_quote {
        out.insert("replyQuote".into(), quote);
    }
    for (key, list) in [
        ("agentReferences", parsed.agent_references),
        ("pastedTextRanges", parsed.pasted_text_ranges),
    ] {
        if !list.is_empty() {
            out.insert(key.into(), Js::Arr(list));
        }
    }
    if let Some(ranges) = parsed.slash_command_ranges {
        out.insert("slashCommandRanges".into(), Js::Arr(ranges));
    }
    if !parsed.session_references.is_empty() {
        out.insert(
            "sessionReferences".into(),
            Js::Arr(parsed.session_references),
        );
    }
    if let Some(origin) = meta::automation_origin(field("origin")) {
        out.insert("automationOrigin".into(), origin);
    }
    if let Some(device) = source::device(meta) {
        out.insert("sourceDevice".into(), device);
    }
    if let Some(plugin) = source::plugin(meta) {
        out.insert("sourcePlugin".into(), plugin);
    }
    if let Some(delivery @ Js::Str(d)) = field("delivery")
        && matches!(d.as_str(), "turn" | "steer")
    {
        out.insert("delivery".into(), delivery.clone());
    }
    if let Some(goal) = field("goalObjective").filter(|g| js::truthy(Some(g))) {
        out.insert("goalBadge".into(), goal.clone());
    }
    let hook = field("imSource")
        .filter(|v| *v != &Js::Null)
        .or(field("hookSource"));
    if let Some(hook) = hook.filter(|h| js::truthy(Some(h))) {
        out.insert("hookSource".into(), hook.clone());
    }
    parent(meta, &mut out);
    out
}
