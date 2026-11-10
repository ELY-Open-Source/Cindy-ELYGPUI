//! `chatQuotes.ts`: quoted selections as Markdown block quotes in a message's text.

#![cfg_attr(
    not(test),
    expect(dead_code, reason = "9.5k's quote chips read the rest")
)]

use std::sync::LazyLock;

use regex::Regex;

use crate::{cindy_json::js_number, theme_import::color::js_space};

const SOURCE: &str = "— source: ";
const MARKER: &str = "> <!-- cindy-composer-quote -->";

/// `ChatQuote`.
#[derive(Debug, Default, PartialEq)]
pub struct Quote {
    pub text: String,
    pub source_path: Option<String>,
    pub start_line: Option<f64>,
    pub end_line: Option<f64>,
}

/// `ChatQuoteSegment`.
#[derive(Debug, PartialEq)]
pub enum Segment {
    Text(String),
    Quote(Quote),
}

fn marked(line: &str) -> bool {
    line.trim_start_matches(js_space) == MARKER
}

/// `stripChatQuoteMarkerLines`.
pub fn strip_marker_lines(content: &str) -> String {
    let kept: Vec<&str> = content.split('\n').filter(|line| !marked(line)).collect();
    kept.join("\n")
}

/// `isValidLine`.
fn valid(line: Option<f64>) -> Option<f64> {
    line.filter(|line| line.fract() == 0.0 && *line > 0.0)
}

/// `formatSourceLine`.
fn source_line(quote: &Quote) -> Option<String> {
    let path = quote
        .source_path
        .as_deref()
        .filter(|path| !path.is_empty())?;
    let Some(start) = valid(quote.start_line) else {
        return Some(path.to_owned());
    };
    let end = valid(quote.end_line).filter(|end| *end >= start);
    Some(match end.filter(|end| *end != start) {
        Some(end) => format!("{path}#L{}-L{}", js_number(start), js_number(end)),
        None => format!("{path}#L{}", js_number(start)),
    })
}

/// `parseSourceLine`.
fn parse_source(raw: &str, quote: &mut Quote) {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        let line = r"[^\n\r\x{2028}\x{2029}]";
        Regex::new(&format!(r"\A({line}*)#L([0-9]+)(?:-L([0-9]+))?\z")).expect("a valid pattern")
    });
    quote.source_path = Some(raw.to_owned());
    let Some(found) = RE.captures(raw) else {
        return;
    };
    let number = |at: usize| {
        found
            .get(at)
            .and_then(|digits| digits.as_str().parse::<f64>().ok())
    };
    let start = valid(number(2));
    let end = match found.get(3) {
        Some(_) => valid(number(3)),
        None => start,
    };
    if let (false, Some(start), Some(end)) = (found[1].is_empty(), start, end) {
        quote.source_path = Some(found[1].to_owned());
        quote.start_line = Some(start);
        quote.end_line = Some(start.max(end));
    }
}

/// `formatQuoteForSend`: one quote as a block that may stand anywhere in the text.
pub fn format(quote: &Quote) -> String {
    let mut out = String::from(MARKER);
    let source = source_line(quote).map(|line| format!("{SOURCE}{line}"));
    let lines = quote.text.trim_matches('\n').split('\n');
    for line in lines.chain(source.as_deref()) {
        out.push_str(if line.is_empty() { "\n>" } else { "\n> " });
        out.push_str(line);
    }
    out
}

/// `quoteFromLines`.
fn from_lines(lines: &[&str]) -> Quote {
    let mut quote = Quote::default();
    match lines.split_last() {
        Some((last, rest)) if !rest.is_empty() && last.starts_with(SOURCE) => {
            quote.text = rest.join("\n");
            parse_source(&last[SOURCE.len()..], &mut quote);
        }
        _ => quote.text = lines.join("\n"),
    }
    quote
}

/// `flushText`: the lines between quotes, less the blank line that parts them.
fn flush(pending: &mut Vec<&str>, segments: &mut Vec<Segment>, before_quote: bool) {
    let follows = matches!(segments.last(), Some(Segment::Quote(_)));
    let lines = std::mem::take(pending);
    if lines.is_empty() {
        return;
    }
    if lines.iter().all(|line| line.is_empty()) {
        let breaks = lines.len() - usize::from(follows || before_quote);
        if breaks > 0 {
            segments.push(Segment::Text("\n".repeat(breaks)));
        }
        return;
    }
    let start = usize::from(follows && lines[0].is_empty());
    let mut end = lines.len();
    if before_quote && end > start && lines[end - 1].is_empty() {
        end -= 1;
    }
    let text = lines[start..end].join("\n");
    if !text.is_empty() {
        segments.push(Segment::Text(text));
    }
}

/// `parseChatQuoteSegments`: quotes and the user's text, in the message's order.
pub fn segments(content: &str) -> Vec<Segment> {
    if !content.contains("> ") {
        let text = (!content.is_empty()).then(|| Segment::Text(content.to_owned()));
        return text.into_iter().collect();
    }
    let lines: Vec<&str> = content.split('\n').collect();
    let mut segments = Vec::new();
    let mut text: Vec<&str> = Vec::new();
    // Quotes without a marker count at the start alone, and only in a message that has none.
    let mut legacy = !content.contains(MARKER);
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        let indent = marked(line).then(|| &line[..line.len() - MARKER.len()]);
        if indent.is_none() && !(legacy && line.starts_with("> ")) {
            text.push(line);
            legacy &= line.is_empty();
            index += 1;
            continue;
        }
        flush(&mut text, &mut segments, true);
        index += usize::from(indent.is_some());
        let prefix = format!("{}>", indent.unwrap_or_default());
        let mut quoted = Vec::new();
        while let Some(rest) = lines.get(index).and_then(|line| line.strip_prefix(&prefix)) {
            match rest.strip_prefix(' ') {
                Some(body) => quoted.push(body),
                None if rest.is_empty() => quoted.push(""),
                None => break,
            }
            index += 1;
        }
        segments.push(Segment::Quote(from_lines(&quoted)));
    }
    flush(&mut text, &mut segments, false);
    segments
}

/// `joinChatQuoteTextSegments`: the text outside the quotes, a blank line between its parts.
pub fn join_text(segments: &[Segment]) -> String {
    let mut texts = segments.iter().filter_map(|segment| match segment {
        Segment::Text(text) => Some(text.as_str()),
        Segment::Quote(_) => None,
    });
    let mut body = texts.next().unwrap_or_default().to_owned();
    for text in texts {
        let trailing = body
            .bytes()
            .rev()
            .take(2)
            .take_while(|b| *b == b'\n')
            .count();
        let leading = text.bytes().take(2).take_while(|b| *b == b'\n').count();
        body.push_str(&"\n".repeat(2usize.saturating_sub(trailing + leading)));
        body.push_str(text);
    }
    body
}

#[cfg(test)]
pub mod tests;
