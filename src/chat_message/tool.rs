//! `mapServerMessages`'s tool, ask, thinking, plan and error branches.

use super::{Out, base, str_of, subagent};
use crate::{
    message_rows,
    rich_answer::js::{self, Js, Obj, utf16_len},
};

/// `Object.prototype`'s names, which `keyParamMap[toolName]` also finds.
const PROTOTYPE: [&str; 12] = [
    "constructor",
    "__defineGetter__",
    "__defineSetter__",
    "hasOwnProperty",
    "__lookupGetter__",
    "__lookupSetter__",
    "isPrototypeOf",
    "propertyIsEnumerable",
    "toString",
    "valueOf",
    "__proto__",
    "toLocaleString",
];

/// `formatToolUseSummary`: the key parameter, cut to 57 units and an ellipsis past 60.
fn summary(tool: &str, input: &Js) -> Result<String, String> {
    if js::truthy(Some(input)) && PROTOTYPE.contains(&tool) {
        return Err("paramKeys is not iterable".into());
    }
    let keys: &[&str] = match tool {
        "Read" => &["file_path", "path"],
        "Edit" | "Write" => &["file_path"],
        "Bash" => &["command"],
        "Glob" | "Grep" => &["pattern"],
        _ => &[],
    };
    let record = input.obj().filter(|_| js::truthy(Some(input)));
    for key in keys {
        if let Some(value) = record
            .and_then(|r| r.get(*key))
            .filter(|v| js::truthy(Some(v)))
        {
            let value = js::try_string(Some(value))?;
            let display = match utf16_len(&value) > 60 {
                true => {
                    let units: Vec<u16> = value.encode_utf16().take(57).collect();
                    format!("{}...", String::from_utf16_lossy(&units))
                }
                false => value,
            };
            return Ok(format!("{tool}({display})"));
        }
    }
    Ok(format!("{tool}()"))
}

pub fn tool_use(row: &Obj, meta: Option<&Obj>) -> Result<Out, String> {
    let content = &row["content"];
    let c = content.obj();
    let tool = message_rows::tool_use(str_of(row, "toolUseId"), content);
    let mut out = base(
        row,
        &row["role"],
        Js::Str(summary(&tool.tool_name, &tool.input)?),
    );
    if let Some(id) = tool.tool_use_id {
        out.insert("toolUseId".into(), Js::Str(id));
    }
    out.insert("toolName".into(), Js::Str(tool.tool_name.clone()));
    out.insert("toolInput".into(), tool.input);
    if let Some(status) = meta
        .and_then(|m| str_of(m, "agentTaskStatus"))
        .filter(|s| matches!(*s, "completed" | "failed" | "stopped"))
    {
        out.insert("agentTaskStatus".into(), Js::Str(status.into()));
    }
    let field = |key: &str| c.and_then(|c| c.get(key));
    let plan = tool.tool_name == "update_plan";
    if plan && field("terminalPlanSnapshot") == Some(&Js::Bool(true)) {
        out.insert("terminalPlanSnapshot".into(), Js::Bool(true));
        if let Some(at @ Js::Num(_)) = field("terminalPlanAtMs") {
            out.insert("terminalPlanAtMs".into(), at.clone());
        }
    }
    if plan && field("turnCompleted") == Some(&Js::Bool(false)) {
        out.insert("turnCompleted".into(), Js::Bool(false));
    }
    out.insert("isStreaming".into(), Js::Bool(false));
    subagent(meta, &mut out);
    Ok(out)
}

/// `formatAskUserReply`.
fn ask_reply(answers: &Js) -> Result<String, String> {
    let entries: Vec<(String, &Js)> = match answers {
        Js::Arr(items) => items
            .iter()
            .enumerate()
            .map(|(i, v)| (i.to_string(), v))
            .collect(),
        Js::Obj(fields) => js::entries(fields)
            .map(|(k, v)| (k.to_owned(), v))
            .collect(),
        _ => unreachable!("an object"),
    };
    let lines = entries
        .into_iter()
        .map(|(q, a)| match js::truthy(Some(a)) {
            true => Ok(format!("{q}: {}", js::try_string(Some(a))?)),
            false => Ok(format!("{q}: (skipped)")),
        })
        .collect::<Result<Vec<String>, String>>()?;
    Ok(lines.join("\n"))
}

pub fn ask_user(row: &Obj, c: &Js) -> Result<Out, String> {
    let get = |key: &str| c.obj().and_then(|c| c.get(key));
    let status = match get("status") == Some(&Js::Str("answered".into())) {
        true => "answered",
        false => "expired",
    };
    let questions = get("questions").filter(|q| matches!(q, Js::Arr(_)));
    let answers = get("answers").filter(|a| matches!(a, Js::Arr(_) | Js::Obj(_)));
    let display = match questions.and_then(Js::arr) {
        Some(items) => {
            let mut questions = Vec::new();
            for q in items {
                questions.push(match q {
                    Js::Null => {
                        return Err("Cannot read properties of null (reading 'question')".into());
                    }
                    Js::Obj(o) => o.get("question"),
                    _ => None,
                });
            }
            let mut parts = Vec::new();
            for question in questions {
                parts.push(match question {
                    None | Some(Js::Null) => String::new(),
                    v => js::try_string(v)?,
                });
            }
            parts.join(" / ")
        }
        None => get("question").and_then(Js::str).unwrap_or("").to_owned(),
    };
    let reply = match answers {
        Some(a) => Js::Str(ask_reply(a)?),
        None => get("reply")
            .filter(|r| matches!(r, Js::Str(_)))
            .cloned()
            .unwrap_or(Js::Null),
    };
    let mut out = base(row, &row["role"], Js::Str(display));
    out.insert("isStreaming".into(), Js::Bool(false));
    out.insert("askUserStatus".into(), Js::Str(status.into()));
    if let Some(id @ Js::Str(_)) = get("requestId") {
        out.insert("askUserRequestId".into(), id.clone());
    }
    out.insert("askUserReply".into(), reply);
    for (key, value) in [
        ("askUserQuestions", questions),
        ("askUserAnswers", answers),
        ("askUserOptions", get("options")),
        (
            "askUserPageIndicator",
            get("pageIndicator").filter(|p| matches!(p, Js::Str(_))),
        ),
    ] {
        if let Some(value) = value {
            out.insert(key.into(), value.clone());
        }
    }
    Ok(out)
}

pub fn thinking(row: &Obj, c: &Js, meta: Option<&Obj>) -> Out {
    let get = |key: &str| c.obj().and_then(|c| c.get(key));
    let text = get("text").and_then(Js::str).unwrap_or("");
    let duration = match get("durationMs") {
        Some(Js::Num(n)) => *n,
        _ => 0.0,
    };
    let finished = match get("finishedAt") {
        Some(Js::Num(n)) => Some(*n),
        Some(Js::Str(s)) => Some(crate::js_date::parse(s)),
        _ => None,
    };
    let mut out = base(row, &row["role"], Js::Str(text.into()));
    out.insert("isStreaming".into(), Js::Bool(false));
    out.insert("thinkingDurationMs".into(), Js::Num(duration));
    out.insert(
        "thinkingRedacted".into(),
        Js::Bool(get("isRedacted") == Some(&Js::Bool(true))),
    );
    if let Some(at) = finished.filter(|f| f.is_finite()) {
        out.insert("thinkingFinishedAtMs".into(), Js::Num(at));
    }
    subagent(meta, &mut out);
    out
}

pub fn plan_review(row: &Obj, c: &Js) -> Out {
    let get = |key: &str| c.obj().and_then(|c| c.get(key)).and_then(Js::str);
    let status = get("status")
        .filter(|s| matches!(*s, "approved" | "revised" | "cancelled"))
        .unwrap_or("expired");
    let mut out = base(row, &row["role"], Js::Str(String::new()));
    out.insert("isStreaming".into(), Js::Bool(false));
    out.insert("planReviewStatus".into(), Js::Str(status.into()));
    if let Some(id) = get("requestId") {
        out.insert("planReviewRequestId".into(), Js::Str(id.into()));
    }
    out.insert(
        "planReviewPlan".into(),
        Js::Str(get("plan").unwrap_or("").into()),
    );
    out.insert(
        "planReviewFilePath".into(),
        Js::Str(get("planFilePath").unwrap_or("").into()),
    );
    if let Some(feedback) = get("feedback") {
        out.insert("planReviewFeedback".into(), Js::Str(feedback.into()));
    }
    out
}

/// `Number.isSafeInteger`.
fn safe_integer(value: Option<&Js>) -> Option<f64> {
    value?
        .integer()
        .filter(|n| n.abs() <= 9_007_199_254_740_991.0)
}

/// `parseToolLoopErrorDetails`.
fn tool_loop(value: Option<&Js>) -> Option<Js> {
    let raw = value?;
    let get = |key: &str| match raw {
        Js::Obj(o) => o.get(key),
        _ => None,
    };
    let kind = get("kind")?
        .str()
        .filter(|k| matches!(*k, "consecutive" | "pingpong" | "rotation" | "contract"))?;
    let count = safe_integer(get("count")).filter(|c| (1.0..=100_000.0).contains(c))?;
    let mut out = Obj::new();
    out.insert("kind".into(), Js::Str(kind.into()));
    out.insert("count".into(), Js::Num(count));
    Some(Js::Obj(out))
}

/// `parseOutputLimitDetails`.
fn output_limit(value: Option<&Js>) -> Option<Js> {
    let raw = value?.obj()?;
    let output = safe_integer(raw.get("outputTokens")).filter(|n| *n >= 1.0)?;
    let mut out = Obj::new();
    out.insert("outputTokens".into(), Js::Num(output));
    if let Some(reasoning) =
        safe_integer(raw.get("reasoningTokens")).filter(|r| *r >= 0.0 && *r <= output)
    {
        out.insert("reasoningTokens".into(), Js::Num(reasoning));
    }
    Some(Js::Obj(out))
}

pub fn error(row: &Obj) -> Out {
    let content = &row["content"];
    if let Js::Str(text) = content {
        let mut out = base(row, &row["role"], Js::Str(text.clone()));
        out.insert("isStreaming".into(), Js::Bool(false));
        return out;
    }
    let get = |key: &str| content.obj().and_then(|c| c.get(key));
    let message = get("message").and_then(Js::str).unwrap_or("");
    let mut out = base(row, &row["role"], Js::Str(message.into()));
    out.insert("isStreaming".into(), Js::Bool(false));
    if let Some(reason) = get("reason").and_then(Js::str).filter(|r| !r.is_empty()) {
        out.insert("errorReason".into(), Js::Str(reason.into()));
    }
    if let Some(details) = tool_loop(get("toolLoop")) {
        out.insert("toolLoop".into(), details);
    }
    if let Some(details) = output_limit(get("outputLimit")) {
        out.insert("outputLimit".into(), details);
    }
    if let Some(id) = get("providerId")
        .and_then(Js::str)
        .filter(|p| !p.is_empty())
    {
        out.insert("errorProviderId".into(), Js::Str(id.into()));
    }
    if get("dismissed") == Some(&Js::Bool(true)) {
        out.insert("errorDismissed".into(), Js::Bool(true));
    }
    out
}
