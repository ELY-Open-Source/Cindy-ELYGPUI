//! The preview and the keyword snippet of a message's visible text.

use super::visible::search_text;
use crate::{
    localdb::cjk::letter_or_number,
    rich_answer::js::Js,
    theme_import::color::{js_space, js_trim},
};

#[derive(Debug, PartialEq)]
pub struct ContentPreview {
    pub preview: String,
    pub snippet: Option<String>,
    pub keyword_matched: bool,
}

/// A hit in UTF-16 offsets of the original text.
#[derive(Clone, Copy)]
pub(crate) struct Range {
    pub start: usize,
    pub end: usize,
}

/// `replace(/\s+/g, ' ').trim()`.
pub fn collapse(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut space = false;
    for c in text.chars() {
        if js_space(c) {
            space = true;
            continue;
        }
        if space && !out.is_empty() {
            out.push(' ');
        }
        space = false;
        out.push(c);
    }
    out
}

/// `textPreview` of a string: collapsed, cut to `max` code points with `...`.
pub fn text_preview(text: &str, max: usize) -> String {
    let text = collapse(text);
    if text.chars().count() <= max {
        return text;
    }
    let cut: String = text.chars().take(max.saturating_sub(1)).collect();
    format!("{}...", cut.trim_end_matches(js_space))
}

/// `normalizeConversationContentPreview`.
pub fn content_preview(
    role: &str,
    content: &Js,
    query: &str,
    max: usize,
    agent_meta: Option<&str>,
) -> ContentPreview {
    let visible = search_text(role, content, agent_meta);
    let ranges = keyword_ranges(&visible, query);
    ContentPreview {
        preview: text_preview(&visible, max),
        snippet: ranges.first().map(|r| snippet(&visible, *r, max)),
        keyword_matched: !ranges.is_empty(),
    }
}

/// The query's letter and digit runs, unique, longest first.
fn tokens(query: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut run = String::new();
    for c in query.chars().chain(['\0']) {
        if letter_or_number(c) {
            run.push(c);
        } else if !run.is_empty() {
            let token = std::mem::take(&mut run);
            if !out.contains(&token) {
                out.push(token);
            }
        }
    }
    out.sort_by_key(|t| std::cmp::Reverse(t.encode_utf16().count()));
    out
}

fn find(haystack: &[u16], needle: &[u16], from: usize) -> Option<usize> {
    haystack
        .get(from..)?
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|i| from + i)
}

/// `keywordRanges`: each code point folded alone, mapped back to the original offsets.
pub(crate) fn keyword_ranges(text: &str, query: &str) -> Vec<Range> {
    let tokens = tokens(query);
    if tokens.is_empty() || text.is_empty() {
        return Vec::new();
    }
    let (mut lower, mut origin, mut widths) = (Vec::new(), Vec::new(), Vec::new());
    let mut at = 0;
    for c in text.chars() {
        lower.extend(c.to_lowercase().collect::<String>().encode_utf16());
        origin.resize(lower.len(), at);
        widths.resize(lower.len(), c.len_utf16());
        at += c.len_utf16();
    }
    let mut ranges: Vec<Range> = Vec::new();
    for token in tokens {
        let needle: Vec<u16> = token.to_lowercase().encode_utf16().collect();
        let mut index = find(&lower, &needle, 0);
        while let Some(i) = index {
            let last = i + needle.len() - 1;
            let next = Range {
                start: origin[i],
                end: origin[last] + widths[last],
            };
            if !ranges
                .iter()
                .any(|r| r.start < next.end && next.start < r.end)
            {
                ranges.push(next);
            }
            index = find(&lower, &needle, i + needle.len());
        }
    }
    ranges.sort_by_key(|r| r.start);
    ranges
}

/// `utf16ToCodePointIndex`.
fn code_point_index(text: &str, utf16: usize) -> usize {
    let mut units = 0;
    text.chars()
        .take_while(|c| {
            let inside = units < utf16;
            units += c.len_utf16();
            inside
        })
        .count()
}

/// `visibleSnippet`: `max` code points around the hit.
fn snippet(text: &str, range: Range, max: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max {
        return text.to_owned();
    }
    let hit_start = code_point_index(text, range.start);
    let hit_end = code_point_index(text, range.end);
    let hit = hit_end.saturating_sub(hit_start).max(1);
    let side = max.saturating_sub(hit) / 2;
    let start = hit_start.saturating_sub(side);
    let end = chars.len().min(start + max);
    let start = end.saturating_sub(max);
    let body: String = chars[start..end].iter().collect();
    let prefix = if start > 0 { "..." } else { "" };
    let suffix = if end < chars.len() { "..." } else { "" };
    format!("{prefix}{}{suffix}", js_trim(&body))
}
