//! `urlTextBoundary.ts`: where a bare http(s) URL ends in chat text. Indexes are UTF-16 units.

#![cfg_attr(
    not(test),
    expect(dead_code, reason = "9.5b's linkify and 9.6's autolinks call it")
)]

use std::sync::LazyLock;

use regex::Regex;
use url::Url;

use crate::{js_re, theme_import::color::js_space};

mod host;

const WRAP_MARKERS: [&str; 6] = ["**", "__", "~~", "*", "_", "~"];

/// `BARE_HTTP_URL_RE_SOURCE`.
const BARE_HTTP: &str = concat!(
    r#"https?://[^\s<>"\u2013-\u2015\u2018-\u201F\u2022\u2026\u3000-\u3001\u3003\u3008-\u3011"#,
    r"\u3014-\u301F\u3030\u303D\u30FB\uFE10-\uFE19\uFF01-\uFF0D\uFF0F\uFF1A-\uFF20\uFF3B-\uFF40",
    r"\uFF5B-\uFF60\uFF62-\uFF65]+",
);

/// The scanner's match; `clip` decides where the address ends.
pub fn bare_http() -> &'static Regex {
    static RE: LazyLock<Regex> = LazyLock::new(|| js_re::regex(BARE_HTTP, false));
    &RE
}

fn re(source: &'static str) -> Regex {
    Regex::new(source).expect("a valid pattern")
}

fn is(unit: Option<&u16>, set: &str) -> bool {
    unit.is_some_and(|unit| set.encode_utf16().any(|c| c == *unit))
}

fn text(units: &[u16]) -> String {
    String::from_utf16_lossy(units)
}

fn find(raw: &[u16], set: &str, from: usize) -> Option<usize> {
    let rest = raw.get(from..)?;
    Some(from + rest.iter().position(|unit| is(Some(unit), set))?)
}

fn find_str(raw: &[u16], needle: &str, from: usize) -> Option<usize> {
    let needle: Vec<u16> = needle.encode_utf16().collect();
    let rest = raw.get(from..)?;
    Some(from + rest.windows(needle.len()).position(|w| w == needle)?)
}

fn count(raw: &[u16], c: char) -> usize {
    raw.iter().filter(|unit| **unit == c as u16).count()
}

fn ascii_alnum(unit: Option<&u16>) -> bool {
    unit.is_some_and(|unit| u8::try_from(*unit).is_ok_and(|c| c.is_ascii_alphanumeric()))
}

fn trim_end(raw: &[u16], cut: usize, set: &str) -> usize {
    let mut end = cut;
    while end > 0 && is(raw.get(end - 1), set) {
        end -= 1;
    }
    end
}

const PROSE: &str = "?!.,:;";
const FORMATTING: &str = "*_~";

/// More `)` than `(` before `end`.
fn closes_exceed(raw: &[u16], end: usize) -> bool {
    count(&raw[..end], ')') > count(&raw[..end], '(')
}

fn trim_unmatched_closers(raw: &[u16], cut: usize) -> usize {
    let mut end = cut;
    while end > 0 && raw[end - 1] == ')' as u16 && closes_exceed(raw, end) {
        end -= 1;
    }
    end
}

/// `countUnmatchedOpeningParens`.
fn unmatched_opening(text: &[u16]) -> usize {
    text.iter().fold(0, |open, unit| match *unit {
        unit if unit == '(' as u16 => open + 1,
        unit if unit == ')' as u16 => open.saturating_sub(1),
        _ => open,
    })
}

fn unmatched_closing(text: &[u16]) -> usize {
    let (mut open, mut close) = (0usize, 0);
    for unit in text {
        match *unit {
            unit if unit == '(' as u16 => open += 1,
            unit if unit == ')' as u16 && open > 0 => open -= 1,
            unit if unit == ')' as u16 => close += 1,
            _ => {}
        }
    }
    close
}

/// `isCodeHostNumericResourcePath`.
fn code_host_resource(prefix: &[u16]) -> bool {
    static GITHUB: LazyLock<Regex> =
        LazyLock::new(|| re(r"^/[^/]+/[^/]+/(?i-u:pulls?|issues?)/[0-9]+/?$"));
    static GITLAB: LazyLock<Regex> =
        LazyLock::new(|| re(r"^/.+/(?:-/)?(?i-u:issues?|merge_requests?)/[0-9]+/?$"));
    let Ok(parsed) = Url::parse(&text(prefix)) else {
        return false;
    };
    let host = parsed.host_str().unwrap_or_default().to_lowercase();
    match host.strip_prefix("www.").unwrap_or(&host) {
        "github.com" => GITHUB.is_match(parsed.path()),
        "gitlab.com" => GITLAB.is_match(parsed.path()),
        _ => false,
    }
}

/// `canTreatQueryOrHashParenAsStatusNote`.
fn status_note_place(raw: &[u16], open: usize, query: usize) -> bool {
    let ampersand = raw[..open].iter().rposition(|unit| *unit == '&' as u16);
    let start = (query + 1).max(ampersand.map_or(0, |at| at + 1));
    let segment = text(&raw[start.min(open)..open]);
    !segment.contains('=') || segment.eq_ignore_ascii_case("diff=split")
}

/// `isCodeHostTruncatedStatusNote`.
fn truncated_status_note(raw: &[u16], open: usize, note_end: usize, query: Option<usize>) -> bool {
    static NOTE: LazyLock<Regex> = LazyLock::new(|| re(r"(?i-u)^base(?: [a-z0-9._/-]*)?$"));
    let Some(query) = query.filter(|query| open >= *query) else {
        return false;
    };
    status_note_place(raw, open, query)
        && NOTE.is_match(&text(raw.get(open + 1..note_end).unwrap_or_default()))
        && code_host_resource(&raw[..query])
}

/// `isCodeHostParentheticalNote`.
fn parenthetical_note(raw: &[u16], open: usize, close: usize, query: Option<usize>) -> bool {
    static NOTE: LazyLock<Regex> =
        LazyLock::new(|| re(r"(?i-u)^base(?: [a-z0-9._/-]+)*,[a-z][a-z0-9_-]*$"));
    let Some(query) = query.filter(|query| open >= *query) else {
        return code_host_resource(&raw[..open]);
    };
    status_note_place(raw, open, query)
        && NOTE.is_match(&text(&raw[open + 1..close]))
        && code_host_resource(&raw[..query])
}

/// `cutBeforeUnbalancedParenProse`.
fn cut_before_paren_prose(raw: &[u16], cut: usize, wrapping: usize) -> usize {
    let end = trim_end(raw, cut.min(raw.len()), PROSE);
    let query = find(raw, "?#", 0);
    let note_end = trim_unmatched_closers(raw, trim_end(raw, end, FORMATTING));
    let mut opens: Vec<usize> = Vec::new();
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for i in 0..end {
        if raw[i] == '(' as u16 {
            opens.push(i);
            continue;
        }
        if raw[i] != ')' as u16 {
            continue;
        }
        if let Some(open) = opens.pop() {
            pairs.push((open, i));
            continue;
        }
        if i >= note_end {
            continue;
        }
        if query.is_some_and(|query| i >= query) {
            if wrapping > 0 && unmatched_closing(&raw[i + 1..end]) < wrapping {
                return i;
            }
            continue;
        }
        let next = raw.get(i + 1).map(|unit| char::from_u32(u32::from(*unit)));
        if next.is_some_and(|next| !next.is_some_and(js_space)) {
            return i;
        }
    }
    let before_query = |open: &&usize| query.is_none_or(|query| **open < query);
    let truncated = |open: &&usize| truncated_status_note(raw, **open, note_end, query);
    let note = |(open, close): &&(usize, usize)| {
        parenthetical_note(raw, *open, *close, query) && *close + 1 == note_end
    };
    let open = opens.iter().find(before_query);
    let open = open.or_else(|| opens.iter().find(truncated));
    let open = open.or_else(|| pairs.iter().find(note).map(|(open, _)| open));
    open.copied().unwrap_or(raw.len())
}

/// `canParseUrlPrefix`.
fn parses(prefix: &[u16]) -> bool {
    Url::parse(&text(prefix)).is_ok_and(|url| {
        matches!(url.scheme(), "http" | "https") && url.host_str().is_some_and(|h| !h.is_empty())
    })
}

/// `cutBeforeUnmatchedQueryBracket`.
fn cut_before_query_bracket(raw: &[u16], query: Option<usize>) -> usize {
    let Some(query) = query else { return raw.len() };
    let (mut square, mut brace) = (0usize, 0usize);
    for (i, unit) in raw.iter().enumerate().skip(query) {
        let Some(c) = char::from_u32(u32::from(*unit)) else {
            continue;
        };
        let counter = match c {
            '[' | ']' => &mut square,
            '{' | '}' => &mut brace,
            _ => continue,
        };
        match c {
            '[' | '{' => *counter += 1,
            _ if *counter == 0 => return i,
            _ => *counter -= 1,
        }
    }
    raw.len()
}

/// `cutBeforePathBracketProse`.
fn cut_before_path_bracket(raw: &[u16]) -> usize {
    let authority = find_str(raw, "://", 0).map_or(0, |scheme| scheme + 3);
    let path = find(raw, "/", authority);
    let query = find(raw, "?#", 0);
    for i in authority..query.unwrap_or(raw.len()) {
        if !is(raw.get(i), "[]{}") {
            continue;
        }
        if path.is_some_and(|path| i >= path) || parses(&raw[..i]) {
            return i;
        }
    }
    cut_before_query_bracket(raw, query)
}

/// `cutBeforeClosingMarkdownWrap`.
fn cut_before_closing_wrap(raw: &[u16], marker: Option<&str>) -> usize {
    let Some(marker) = marker.filter(|marker| !marker.is_empty()) else {
        return raw.len();
    };
    let size = marker.encode_utf16().count();
    let mut at = find_str(raw, marker, 0);
    while let Some(index) = at {
        let inside_word = matches!(marker, "_" | "__")
            && ascii_alnum(index.checked_sub(1).and_then(|before| raw.get(before)))
            && ascii_alnum(raw.get(index + size));
        if !inside_word {
            return index;
        }
        at = find_str(raw, marker, index + size);
    }
    raw.len()
}

/// `markdownWrapMarkerFromPrefix`.
pub fn wrap_marker(prefix: &str) -> Option<&'static str> {
    let marker = WRAP_MARKERS.into_iter().find(|m| prefix.ends_with(m))?;
    let before = prefix[..prefix.len() - marker.len()].chars().next_back();
    before
        .is_none_or(|c| !c.is_ascii_alphanumeric())
        .then_some(marker)
}

struct Shrink {
    formatting: bool,
    apostrophe: bool,
    parens: usize,
}

/// `shrinkAutolinkTrailingJunk`.
fn shrink(url: &[u16], cut: usize, options: Shrink) -> usize {
    let mut end = cut.min(url.len());
    let query = find(url, "?#", 0);
    let mut parens = options
        .parens
        .saturating_sub(unmatched_closing(&url[end..]));
    while end > 0 {
        let unit = url.get(end - 1);
        let in_query = query.is_some_and(|query| end > query);
        if is(unit, PROSE)
            || (options.formatting && is(unit, FORMATTING))
            || (is(unit, "(") && !in_query)
            || (is(unit, "'") && options.apostrophe)
        {
            end -= 1;
            continue;
        }
        if !is(unit, ")") || !closes_exceed(url, end) {
            break;
        }
        if in_query {
            if parens == 0 {
                break;
            }
            parens -= 1;
        }
        end -= 1;
    }
    end
}

/// `stripMarkdownFormattingPunct`.
#[derive(Clone, Copy, Default, PartialEq)]
pub enum Strip {
    #[default]
    Never,
    Always,
    Auto,
}

/// `ClipBareHttpAutolinkOptions`.
pub struct Options<'a> {
    /// The text before the URL.
    pub prefix: &'a str,
    /// `markdownWrapMarker`; `None` reads it from the prefix.
    pub wrap_marker: Option<Option<&'a str>>,
    pub strip_formatting: Strip,
    pub cut_path_brackets: bool,
}

impl Default for Options<'_> {
    fn default() -> Self {
        Options {
            prefix: "",
            wrap_marker: None,
            strip_formatting: Strip::Never,
            cut_path_brackets: true,
        }
    }
}

/// `hasTrailingMultiCharMarkdownMarkerAfterCodeHostResource`.
fn marker_after_resource(raw: &[u16], cut: usize) -> bool {
    let two = |marker: &str| {
        cut >= 2 && text(&raw[cut - 2..cut]) == marker && code_host_resource(&raw[..cut - 2])
    };
    ["**", "__", "~~"].into_iter().any(two)
}

/// `MARKDOWN_FORMATTING_STRIP_BOUNDARY`.
fn strip_boundary(unit: u16) -> bool {
    matches!(
        unit,
        0x3000..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xAC00..=0xD7AF | 0xFF00..=0xFFEF
    )
}

/// `clipBareHttpAutolink`: the byte at which `raw`'s address ends.
pub fn clip(raw: &str, options: &Options) -> usize {
    let units: Vec<u16> = raw.encode_utf16().collect();
    let raw_units = units.as_slice();
    let prefix: Vec<u16> = options.prefix.encode_utf16().collect();
    let wrapping = unmatched_opening(&prefix);
    let marker = options
        .wrap_marker
        .unwrap_or_else(|| wrap_marker(options.prefix));
    let boundary = host::prose_boundary(raw_units);
    let wrap = cut_before_closing_wrap(raw_units, marker);
    let limited = boundary.min(wrap);
    let brackets = match options.cut_path_brackets {
        true => cut_before_path_bracket(raw_units),
        false => units.len(),
    };
    let prose = limited
        .min(cut_before_paren_prose(raw_units, limited, wrapping))
        .min(brackets);
    let auto = || {
        wrap < units.len()
            || marker_after_resource(raw_units, prose)
            || (boundary < units.len() && prose == boundary && strip_boundary(units[boundary]))
    };
    let formatting = match options.strip_formatting {
        Strip::Always => true,
        Strip::Auto => auto(),
        Strip::Never => false,
    };
    let junk = Shrink {
        formatting,
        apostrophe: options.prefix.ends_with('\''),
        parens: wrapping,
    };
    let end = shrink(raw_units, prose, junk);
    String::from_utf16(&units[..end])
        .expect("a cut between characters")
        .len()
}

#[cfg(test)]
mod tests;
