//! The prose boundary of a bare URL, with the IDN dots a hostname keeps.

use std::sync::LazyLock;

use super::{find, is};
use crate::v8_classes::within;

/// A code point of `raw` with its length in units; a lone surrogate is `None`.
fn point(raw: &[u16], index: usize) -> Option<(Option<char>, usize)> {
    let first = *raw.get(index)?;
    let pair = raw.get(index..index + 2).and_then(|pair| {
        let mut decoded = char::decode_utf16(pair.iter().copied());
        decoded.next()?.ok().filter(|c| c.len_utf16() == 2)
    });
    Some(match pair {
        Some(c) => (Some(c), 2),
        None => (char::from_u32(u32::from(first)), 1),
    })
}

fn unit(raw: &[u16], index: usize) -> Option<Option<char>> {
    raw.get(index).map(|unit| char::from_u32(u32::from(*unit)))
}

/// V8's `[\p{P}\p{Z}]` and `\p{L}`, from `scripts/sync-golden.mjs`.
#[derive(serde::Deserialize)]
struct Classes {
    boundary: Vec<[u32; 2]>,
    letter: Vec<[u32; 2]>,
}

fn classes() -> &'static Classes {
    static CLASSES: LazyLock<Classes> = LazyLock::new(|| {
        let text = include_str!("../../resources/url-classes.json");
        serde_json::from_str(text).expect("the bundled character classes parse")
    });
    &CLASSES
}

/// `isAutolinkPunctuationBoundary`.
fn punctuation(c: char) -> bool {
    match c {
        '"' | '`' => true,
        c if c.is_ascii() => false,
        c => within(&classes().boundary, c),
    }
}

/// `isIdnDomainDot`.
fn idn_dot(c: Option<char>) -> bool {
    matches!(c, Some('\u{3002}' | '\u{ff0e}' | '\u{ff61}'))
}

/// `isHostLabelChar`, where a lone surrogate is a label character.
fn label(c: Option<char>) -> bool {
    !idn_dot(c) && c.is_none_or(|c| !"/?#:@".contains(c) && !punctuation(c))
}

/// `isCjkLetter`.
fn cjk(c: char) -> bool {
    matches!(c, '\u{3040}'..='\u{30ff}' | '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}'
        | '\u{ac00}'..='\u{d7af}' | '\u{f900}'..='\u{faff}')
}

/// `hostnameRange`.
fn hostname(raw: &[u16]) -> (usize, usize) {
    let head = String::from_utf16_lossy(&raw[..raw.len().min(8)]).to_ascii_lowercase();
    let scheme = ["https://", "http://"]
        .into_iter()
        .find(|scheme| head.starts_with(scheme));
    let mut start = scheme.map_or(0, str::len);
    let end = find(raw, "/?#", start).unwrap_or(raw.len());
    let at = raw[..end].iter().rposition(|unit| *unit == '@' as u16);
    if let Some(at) = at.filter(|at| *at >= start) {
        start = at + 1;
    }
    if is(raw.get(start), "[") {
        let close = find(raw, "]", start + 1).filter(|close| *close < end);
        return (start, close.map_or(end, |close| close + 1));
    }
    let port = find(&raw[..end], ":", start);
    (start, port.unwrap_or(end))
}

/// `nextHostLabelEnd`.
fn label_end(raw: &[u16], from: usize, host_end: usize) -> usize {
    let mut index = from;
    while index < host_end {
        let Some((c, size)) = point(raw, index) else {
            break;
        };
        if c == Some('.') || !label(c) {
            break;
        }
        index += size;
    }
    index
}

/// `isIdnDotInHostname`.
fn dot_in_hostname(raw: &[u16], index: usize, (start, end): (usize, usize)) -> bool {
    if index < start || index >= end {
        return false;
    }
    let dot = unit(raw, index).is_some_and(idn_dot);
    if !dot || !unit(raw, index + 1).is_some_and(label) {
        return false;
    }
    let stop = label_end(raw, index + 1, end);
    let chars: Vec<char> = char::decode_utf16(raw[index + 1..stop].iter().copied())
        .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect();
    if chars.is_empty() {
        return false;
    }
    let script = |c: &char| within(&classes().letter, *c) && !cjk(*c);
    if chars.iter().any(script) || chars.iter().any(char::is_ascii_alphanumeric) {
        return true;
    }
    let after = unit(raw, stop).flatten();
    after == Some('.') || idn_dot(after) || end < raw.len() || chars.len() <= 3
}

/// `findAutolinkProseBoundary`.
pub fn prose_boundary(raw: &[u16]) -> usize {
    let host = hostname(raw);
    let mut index = 0;
    while let Some((c, size)) = point(raw, index) {
        if c.is_some_and(punctuation) && !dot_in_hostname(raw, index, host) {
            return index;
        }
        index += size;
    }
    raw.len()
}
