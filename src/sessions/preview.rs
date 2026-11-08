//! `mapper.ts`'s `finalizePlainPreview`: a message's text as the card's one-line preview.

use std::sync::{LazyLock, Mutex};

use indexmap::IndexMap;

use crate::{markdown_text, rich_answer, theme_import::color::js_space};

/// `PREVIEW_MAX_CHARS`, in UTF-16 units.
const MAX_CHARS: usize = 140;
/// `PREVIEW_CONTENT_BOUND_CHARS`: longer texts are not cached.
const CACHED_CHARS: usize = 4096;
/// `PREVIEW_CACHE_ENTRIES`.
const CACHE_ENTRIES: usize = 1024;

const UI_ACTION_TRIGGER_PREFIX: &str = "[UI_ACTION_TRIGGER]";
const CITATION_OPEN: &str = "\u{E200}cite\u{E202}";
const CITATION_CLOSE: &str = "\u{E201}";
const STOP_TOKENS: [&str; 4] = ["<|endoftext|>", "<|eot_id|>", "<|im_end|>", "<|eos|>"];

/// `plainPreviewCache`, least recently used first.
static CACHE: LazyLock<Mutex<IndexMap<String, Option<String>>>> = LazyLock::new(Mutex::default);

pub fn finalize(text: Option<&str>, role: Option<&str>) -> Option<String> {
    let text = text.filter(|t| !t.is_empty())?;
    let assistant = role == Some("assistant");
    let key = (text.encode_utf16().count() <= CACHED_CHARS)
        .then(|| format!("{}:{text}", if assistant { "assistant" } else { "plain" }));
    let mut cache = CACHE.lock().expect("the preview cache never poisons");
    if let Some(key) = &key
        && let Some(cached) = cache.shift_remove(key)
    {
        cache.insert(key.clone(), cached.clone());
        return cached;
    }
    drop(cache);
    let preview = compute(text, assistant);
    if let Some(key) = key {
        let mut cache = CACHE.lock().expect("the preview cache never poisons");
        cache.insert(key, preview.clone());
        if cache.len() > CACHE_ENTRIES {
            cache.shift_remove_index(0);
        }
    }
    preview
}

/// `computePlainPreview`.
fn compute(text: &str, assistant: bool) -> Option<String> {
    if text.starts_with(UI_ACTION_TRIGGER_PREFIX) {
        return None;
    }
    let next = if assistant {
        rich_answer::text(&strip_internal_web_citations(text))
    } else {
        text.to_owned()
    };
    let collapsed = markdown_text::preview(&next);
    (!collapsed.is_empty()).then(|| truncate(&collapsed))
}

/// `slice(0, 140)`; a pair cut in half shows as Chromium shows a lone surrogate.
fn truncate(text: &str) -> String {
    let mut out = String::new();
    let mut units = 0;
    for c in text.chars() {
        if units + c.len_utf16() > MAX_CHARS {
            if units < MAX_CHARS {
                out.push('\u{FFFD}');
            }
            break;
        }
        units += c.len_utf16();
        out.push(c);
    }
    out
}

/// `stripInternalWebCitations`.
fn strip_internal_web_citations(text: &str) -> String {
    stop_token_only(&strip_citation_markers(text))
}

fn strip_citation_markers(text: &str) -> String {
    if !text.contains('\u{E200}') {
        return text.to_owned();
    }
    let stable = &text[..stable_boundary(text)];
    let mut out = String::new();
    let mut from = 0;
    loop {
        let Some(open) = stable[from..].find(CITATION_OPEN).map(|i| from + i) else {
            out.push_str(&stable[from..]);
            return out;
        };
        let body = open + CITATION_OPEN.len();
        let Some(close) = stable[body..].find(CITATION_CLOSE).map(|i| body + i) else {
            out.push_str(&stable[from..open]);
            return out;
        };
        out.push_str(&stable[from..open]);
        from = close + CITATION_CLOSE.len();
    }
}

/// `stableInternalWebCitationBoundary`: an unfinished marker, or a marker's start at the end.
fn stable_boundary(text: &str) -> usize {
    let mut from = 0;
    while let Some(open) = text[from..].find(CITATION_OPEN).map(|i| from + i) {
        let body = open + CITATION_OPEN.len();
        match text[body..].find(CITATION_CLOSE) {
            Some(close) => from = body + close + CITATION_CLOSE.len(),
            None => return open,
        }
    }
    let starts: Vec<usize> = CITATION_OPEN
        .char_indices()
        .map(|(i, _)| i)
        .skip(1)
        .collect();
    starts
        .iter()
        .rev()
        .map(|end| &CITATION_OPEN[..*end])
        .find(|prefix| text.ends_with(prefix))
        .map_or(text.len(), |prefix| text.len() - prefix.len())
}

/// `stripStandaloneModelStopToken`: a message of stop tokens and spaces alone is empty.
fn stop_token_only(text: &str) -> String {
    let mut rest = text;
    let mut consumed = false;
    loop {
        rest = rest.trim_start_matches(js_space);
        match STOP_TOKENS.iter().find(|t| rest.starts_with(**t)) {
            Some(token) => {
                consumed = true;
                rest = &rest[token.len()..];
            }
            None => break,
        }
    }
    if consumed && rest.is_empty() {
        String::new()
    } else {
        text.to_owned()
    }
}

#[cfg(test)]
mod tests;
