//! `userMessageDisplayText.ts`: the text a user message shows, apart from what it stores.

#![cfg_attr(
    not(test),
    expect(dead_code, reason = "9.5r's hook card reads the rest")
)]

use std::sync::LazyLock;

use regex::Regex;

use crate::{
    localdb::search::forwarded,
    rich_answer::js::{self, Js, truthy},
    theme_import::color::{JS_SPACE, js_trim},
};

pub mod collapse;
pub mod sent;

/// `OrcaCommunicationContent`.
#[derive(Debug, PartialEq)]
pub struct Orca {
    /// `lead` or `worker`.
    pub source: String,
    pub content: String,
}

/// A field of a value that may be no object, as `value?.key` reads it.
fn field<'a>(value: Option<&'a Js>, key: &str) -> Option<&'a Js> {
    value?.obj()?.get(key).filter(|value| **value != Js::Null)
}

/// `parseOrcaCommunicationContent`.
pub fn orca(content: &str) -> Option<Orca> {
    let parsed = js::parse(content).ok()?;
    let source = field(Some(&parsed), "orcaSource")?.str()?;
    let content = field(Some(&parsed), "content")?.str()?;
    matches!(source, "lead" | "worker").then(|| Orca {
        source: source.to_owned(),
        content: content.to_owned(),
    })
}

/// `THREAD_CONTEXT_PREFIX_RE`.
fn thread_context() -> &'static Regex {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        let line = r"[^\n\r\x{2028}\x{2029}]";
        let source = format!(
            r"\A<thread_context>(?s:.*?)</thread_context>{JS_SPACE}*(?:\(thread 历史中的{line}*?\){JS_SPACE}*)?"
        );
        Regex::new(&source).expect("a valid pattern")
    });
    &RE
}

/// `captureImContext`'s group context, from a prefix the host wrote.
fn captured_group(prefix: &str) -> Option<&str> {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        let heads = "自你上次请求后群里新增的消息|群里最近的消息|本话题里最近的消息";
        let source = format!(
            r"\A<group_chat_context>\r?\n\[(?:{heads})\]\r?\n((?s:.*?))\r?\n</group_chat_context>(?:\r?\n|\z)"
        );
        Regex::new(&source).expect("a valid pattern")
    });
    let context = RE.captures(prefix)?.get(1)?.as_str();
    (!js_trim(context).is_empty()).then_some(context)
}

/// `resolveHookGroupContext`: the group context saved with an IM message.
pub fn hook_group_context<'a>(content: &'a str, hook_source: Option<&'a Js>) -> Option<&'a str> {
    let im = field(hook_source, "im")?.str()?;
    if js_trim(im).is_empty() {
        return None;
    }
    match field(hook_source, "contextSnapshot").filter(|snapshot| truthy(Some(snapshot))) {
        Some(snapshot) => field(Some(snapshot), "groupContext")?.str(),
        None => captured_group(content),
    }
}

/// `resolveUserDisplayText`; `forwarded` says the automation origin is a task.
pub fn display_text(
    content: &str,
    hook_source: Option<&Js>,
    forwarded: bool,
    orca: Option<&Orca>,
) -> String {
    let raw = orca.map_or(content, |orca| &orca.content);
    if !truthy(hook_source) {
        return match forwarded {
            true => forwarded::display_text(raw),
            false => raw.to_owned(),
        };
    }
    match field(hook_source, "userText") {
        Some(text) => js::string(Some(text)),
        None => js_trim(&thread_context().replace(raw, "")).to_owned(),
    }
}

#[cfg(test)]
mod tests;
