//! `visibleMessageText`: what a message shows its reader, by role, from its parsed content.

use super::{forwarded, snippet::collapse};
use crate::{
    rich_answer::js::{self, Js, Obj},
    sessions::preview::{UI_ACTION_TRIGGER_PREFIX, strip_internal_web_citations},
    theme_import::color::js_trim,
};

const PREFERRED: [&str; 5] = ["text", "content", "message", "question", "plan"];

/// `visibleMessageTextForConversationSearch`: one line, a forwarded request decoded.
pub fn search_text(role: &str, content: &Js, agent_meta: Option<&str>) -> String {
    let text = visible_text(role, content, false);
    let meta = parse_like(agent_meta.unwrap_or(""));
    let from_session = meta
        .obj()
        .and_then(|m| m.get("origin")?.obj()?.get("kind")?.str())
        == Some("session");
    if role == "user" && from_session {
        return collapse(&forwarded::display_text(&text));
    }
    collapse(&text)
}

/// `visibleMessageText`; `preserve_whitespace` keeps the result card's indentation.
pub fn visible_text(role: &str, content: &Js, preserve_whitespace: bool) -> String {
    let reparsed;
    let parsed = match content {
        Js::Str(s) => {
            reparsed = parse_like(s);
            &reparsed
        }
        other => other,
    };
    let raw = match role {
        "user" => user_text(parsed),
        "assistant" => strip_internal_web_citations(&assistant_text(parsed)),
        "ask_user" => ask_user_text(parsed),
        "plan_review" => plan_review_text(parsed),
        _ => extract(Some(parsed)),
    };
    if preserve_whitespace {
        raw
    } else {
        js_trim(&raw).to_owned()
    }
}

/// `tryParseJsonLike`: an object or array when it parses, else the text itself.
fn parse_like(value: &str) -> Js {
    let trimmed = js_trim(value);
    if trimmed.starts_with(['{', '['])
        && let Ok(parsed) = js::parse(trimmed)
    {
        return parsed;
    }
    Js::Str(value.to_owned())
}

/// A hidden UI trigger is no visible text, as everywhere Cindy shows user text.
fn user_text(content: &Js) -> String {
    let text = user_text_raw(content);
    if text.starts_with(UI_ACTION_TRIGGER_PREFIX) {
        String::new()
    } else {
        text
    }
}

fn user_text_raw(content: &Js) -> String {
    if let Js::Str(s) = content {
        return s.clone();
    }
    let blocks = blocks_text(content);
    if !blocks.is_empty() {
        return blocks;
    }
    match content {
        Js::Obj(o) => match o.get("text") {
            Some(Js::Str(s)) => s.clone(),
            _ => preferred(o),
        },
        _ => String::new(),
    }
}

fn assistant_text(content: &Js) -> String {
    if let Js::Str(s) = content {
        return s.clone();
    }
    let blocks = blocks_text(content);
    if !blocks.is_empty() {
        return blocks;
    }
    if let Js::Obj(o) = content
        && o.get("type").and_then(Js::str) == Some("text")
        && let Some(Js::Str(text)) = o.get("text")
    {
        return text.clone();
    }
    extract(Some(content))
}

fn ask_user_text(content: &Js) -> String {
    let o = match content {
        Js::Str(s) => return s.clone(),
        Js::Obj(o) => o,
        _ => return String::new(),
    };
    let mut parts: Vec<&str> = Vec::new();
    if let Some(Js::Arr(questions)) = o.get("questions") {
        parts.extend(
            questions
                .iter()
                .filter_map(|q| q.obj()?.get("question")?.str()),
        );
    }
    parts.extend(o.get("question").and_then(Js::str));
    match o.get("answers") {
        Some(Js::Obj(answers)) => parts.extend(js::values(answers).filter_map(Js::str)),
        Some(Js::Arr(answers)) => parts.extend(answers.iter().filter_map(Js::str)),
        _ => {}
    }
    parts.extend(o.get("reply").and_then(Js::str));
    parts.join("\n")
}

fn plan_review_text(content: &Js) -> String {
    match content {
        Js::Str(s) => s.clone(),
        Js::Obj(o) => ["plan", "feedback"]
            .iter()
            .filter_map(|k| o.get(*k)?.str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// `textBlocksText`: the `text` blocks of a content array.
fn blocks_text(content: &Js) -> String {
    let Js::Arr(blocks) = content else {
        return String::new();
    };
    blocks
        .iter()
        .filter_map(|b| {
            let b = b.obj()?;
            (b.get("type")?.str()? == "text").then_some(())?;
            b.get("text")?.str()
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// `preferredObjectText`.
fn preferred(o: &Obj) -> String {
    joined(PREFERRED.iter().map(|k| extract(o.get(*k))))
}

fn joined(parts: impl Iterator<Item = String>) -> String {
    parts
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// `extractText`; none is `undefined`.
fn extract(value: Option<&Js>) -> String {
    js::deep(|| match value {
        Some(Js::Str(s)) => s.clone(),
        Some(Js::Arr(items)) => joined(items.iter().map(|v| extract(Some(v)))),
        Some(Js::Obj(o)) => {
            let first = preferred(o);
            if first.is_empty() {
                joined(js::values(o).map(|v| extract(Some(v))))
            } else {
                first
            }
        }
        _ => String::new(),
    })
}
