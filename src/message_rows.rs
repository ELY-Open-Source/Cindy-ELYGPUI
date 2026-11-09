//! The row checks `projectHistoryView` reads, on stored rows.
#![cfg_attr(
    not(test),
    expect(dead_code, reason = "`projectHistoryView` reads them in 9.1b")
)]

use std::sync::LazyLock;

use regex::Regex;

use crate::{
    payload,
    rich_answer::{
        self,
        js::{self, Js, Obj, utf16_len},
    },
    theme_import::color::{JS_SPACE, js_space, js_trim},
};

const UI_ACTION_TRIGGER_PREFIX: &str = "[UI_ACTION_TRIGGER]";
const CONTINUE_COMMON: &str = " Review the conversation above to determine how far the task progressed, then continue from where it left off. Do not repeat steps that already completed and had external effects (messages or comments already sent, commits already pushed, files already written).";
const APP_EXIT: &str = " The previous turn was interrupted because the app exited mid-run.";
const ERROR: &str = " The previous turn errored partway through.";
const CHECKPOINT: &str = "\n\n[CINDY_RECOVERY_CHECKPOINT v1]";
const DELIVERY_PROSE_MIN_LENGTH: usize = 600;
const DELIVERY_PROSE_MIN_LIST_ITEMS: usize = 3;

fn not_space() -> String {
    JS_SPACE.replacen('[', "[^", 1)
}

fn pattern(source: &str) -> Regex {
    Regex::new(&source.replace(r"\s", JS_SPACE).replace(r"\S", &not_space()))
        .expect("valid pattern")
}

static HEADING: LazyLock<Regex> = LazyLock::new(|| pattern(r"^[ \t]{0,3}#{1,6}[ \t]+\S"));
static TABLE_DIVIDER: LazyLock<Regex> =
    LazyLock::new(|| pattern(r"^[ \t]{0,3}\|?[ \t]*:?-{3,}:?[ \t]*\|[-:| \t]*$"));
static LIST_ITEM: LazyLock<Regex> =
    LazyLock::new(|| pattern(r"^[ \t]{0,3}(?:[-*+][ \t]+|[0-9]{1,3}[.)][ \t]+)\S"));
static IMAGE: LazyLock<Regex> = LazyLock::new(|| {
    let ws = JS_SPACE.trim_start_matches('[').trim_end_matches(']');
    Regex::new(&format!(r"!\[[^\]\n]*\]\([^){ws}]+(?:{JS_SPACE}[^)]*)?\)")).expect("valid pattern")
});
// `/<img(?=[\s/>])[^<>]*\ssrc\s*=\s*["']?[^\s"'<>]+[^<>]*>/i`, the lookahead unrolled.
static HTML_IMAGE: LazyLock<Regex> = LazyLock::new(|| {
    let ws = JS_SPACE.trim_start_matches('[').trim_end_matches(']');
    Regex::new(&format!(
        r#"<[iI][mM][gG](?:[{ws}/][^<>]*)?{JS_SPACE}[sS][rR][cC]{JS_SPACE}*={JS_SPACE}*["']?[^{ws}"'<>]+[^<>]*>"#
    ))
    .expect("valid pattern")
});

static MEDIA_REFERENCE: LazyLock<Regex> =
    LazyLock::new(|| pattern(r"^(?:cindy-media|xdt-file)://\S+$"));

/// `parseMessageToolUse`'s result.
#[derive(Debug, PartialEq)]
pub struct ToolUse {
    pub tool_use_id: Option<String>,
    pub tool_name: String,
    pub input: Js,
}

/// `readNonEmptyString`: a nonempty string, untrimmed.
fn non_empty(value: Option<&Js>) -> Option<&str> {
    value?.str().filter(|s| !s.is_empty())
}

/// `parseMessageToolUse`'s gateway unwrap: Pi's MCP call becomes `mcp:server:tool`.
pub fn unwrap_gateway(name: &str, input: Option<&Js>) -> (String, Option<Js>) {
    if name == "cindy_mcp_call_tool"
        && let Some(gateway) = input.and_then(Js::obj)
        && let (Some(server), Some(tool)) = (
            non_empty(gateway.get("server")),
            non_empty(gateway.get("tool")),
        )
    {
        let args = match gateway.get("args") {
            None => Some(Js::Obj(Obj::new())),
            Some(args @ Js::Obj(_)) => Some(args.clone()),
            Some(_) => None,
        };
        if let Some(args) = args {
            return (format!("mcp:{server}:{tool}"), Some(args));
        }
    }
    (name.to_owned(), input.cloned())
}

/// `parseMessageToolUse` on a row's `toolUseId` and content.
pub fn tool_use(tool_use_id: Option<&str>, content: &Js) -> ToolUse {
    let record = content.obj();
    let id = tool_use_id
        .filter(|id| !id.is_empty())
        .or_else(|| non_empty(record.and_then(|r| r.get("toolUseId"))));
    let name = non_empty(record.and_then(|r| r.get("toolName"))).unwrap_or("");
    let input = record
        .and_then(|r| r.get("input"))
        .filter(|i| **i != Js::Null);
    let (tool_name, input) = unwrap_gateway(name, input);
    ToolUse {
        tool_use_id: id.map(str::to_owned),
        tool_name,
        input: input.unwrap_or(Js::Null),
    }
}

/// `messageContentToPreview`.
pub fn content_to_preview(content: &Js) -> String {
    match content {
        Js::Null => String::new(),
        Js::Bool(b) => b.to_string(),
        Js::Num(n) => crate::cindy_json::js_number(*n),
        Js::Str(s) => s.clone(),
        Js::Arr(items) => items
            .iter()
            .map(content_to_preview)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        Js::Obj(record) => {
            let string = |key: &str| record.get(key).and_then(Js::str);
            let kind = string("type");
            match (kind, string("text"), string("thinking")) {
                (Some("text"), Some(text), _) => text.to_owned(),
                (Some("thinking"), _, Some(thinking)) => thinking.to_owned(),
                (_, Some(text), _) => text.to_owned(),
                _ => match string("content") {
                    Some(text) => text.to_owned(),
                    None => js::stringify(content, "  "),
                },
            }
        }
    }
}

/// `isOrcaCommunicationTool`.
pub fn is_orca_communication_tool(name: &str) -> bool {
    let prefixed = match name.strip_prefix("mcp__") {
        Some(rest) => format!("mcp:{rest}"),
        None => name.to_owned(),
    };
    let normalized = prefixed.replace("__", ":");
    matches!(
        normalized.as_str(),
        "mcp:orca_worker_bridge:send_to_lead"
            | "mcp:orca_worker_bridge:read_lead"
            | "mcp:orca_worker_bridge:lead_status"
    ) || matches!(name, "send_to_lead" | "read_lead" | "lead_status")
}

/// `isAgentPlanToolName`.
pub fn is_agent_plan_tool_name(name: &str) -> bool {
    matches!(
        name,
        "TodoWrite" | "update_plan" | "TaskCreate" | "TaskUpdate" | "TaskList" | "TaskGet"
    )
}

/// `isAgentTaskToolName`: a subagent spawn or any Codex `collab:` call.
pub fn is_agent_task_tool_name(name: &str) -> bool {
    matches!(
        name,
        "Agent" | "Task" | "subagent" | "collab:spawn" | "collab:spawnAgent"
    ) || name.starts_with("collab:")
}

/// `isSteerUserRow` on a stored row, which keeps `delivery` only in its meta.
pub fn is_steer_user_row(agent_meta: Option<&Js>) -> bool {
    agent_meta.and_then(Js::obj).and_then(|m| m.get("delivery")) == Some(&Js::Str("steer".into()))
}

/// `syntheticTriggerKind(text) === 'continue'`.
fn is_continue_trigger(text: &str) -> bool {
    if !text.starts_with(UI_ACTION_TRIGGER_PREFIX) {
        return false;
    }
    [APP_EXIT, ERROR].iter().any(|reason| {
        let prompt = format!("{UI_ACTION_TRIGGER_PREFIX}{reason}{CONTINUE_COMMON}");
        text == prompt || text.starts_with(&format!("{prompt}{CHECKPOINT}"))
    })
}

/// `isContinuationMessage` on a stored row, which carries no `systemCardType` or trigger flag.
pub fn is_continuation_message(role: &str, content: &Js, agent_meta: Option<&Js>) -> bool {
    if role != "user" {
        return false;
    }
    let meta = agent_meta.and_then(Js::obj);
    if meta.and_then(|m| m.get("autoResume")) == Some(&Js::Bool(true)) {
        return true;
    }
    let parsed;
    let content = match content {
        Js::Str(s) if s.trim_start_matches(js_space).starts_with('{') => match js::parse(s) {
            Ok(value) => {
                parsed = value;
                &parsed
            }
            Err(_) => return false,
        },
        other => other,
    };
    let text = match content {
        Js::Str(s) => Some(s.as_str()),
        other => other.obj().and_then(|o| o.get("text")).and_then(Js::str),
    };
    text.is_some_and(is_continue_trigger)
}

/// `hasVisibleHistoryResult`: a result that delivers media, a file or a card.
pub fn has_visible_history_result(content: &Js) -> bool {
    let text = content_to_preview(content);
    if !payload::tool_result_media(&text).is_empty()
        || !payload::tool_result_files(&text).is_empty()
        || !payload::tool_card_ids(&text).is_empty()
        || MEDIA_REFERENCE.is_match(js_trim(&text))
    {
        return true;
    }
    let Ok(parsed) = js::parse(&text) else {
        return false;
    };
    let value = parsed.obj();
    ["xdt_card_id", "xdt_anchor_card_id"]
        .iter()
        .any(|key| value.and_then(|v| v.get(*key)).and_then(Js::str).is_some())
}

/// JavaScript's line terminators, where a multiline `^` and `$` match.
fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.split(['\n', '\r', '\u{2028}', '\u{2029}'])
}

/// `isDeliveryProseText`.
pub fn is_delivery_prose_text(text: &str) -> bool {
    let trimmed = js_trim(text);
    if trimmed.is_empty() {
        return false;
    }
    rich_answer::has(trimmed)
        || utf16_len(trimmed) >= DELIVERY_PROSE_MIN_LENGTH
        || lines(trimmed).any(|l| HEADING.is_match(l) || TABLE_DIVIDER.is_match(l))
        || IMAGE.is_match(trimmed)
        || HTML_IMAGE.is_match(trimmed)
        || lines(trimmed).filter(|l| LIST_ITEM.is_match(l)).count() >= DELIVERY_PROSE_MIN_LIST_ITEMS
}

#[cfg(test)]
pub mod tests;
