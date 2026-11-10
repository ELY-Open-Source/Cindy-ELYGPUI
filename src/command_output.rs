//! `commandOutputPaths.ts`: the files a command's text says it writes. Offsets are bytes.

mod converters;
mod explicit;
mod index;
mod tokens;

use std::{borrow::Cow, collections::HashSet, sync::LazyLock};

use regex::Regex;

use crate::{js_re, rich_answer::js::utf16_len};

/// `EXT_RE`.
static EXT: LazyLock<Regex> = LazyLock::new(|| js_re::regex(r"\.[A-Za-z][A-Za-z0-9]{0,7}$", false));
/// `TEMP_DIR_RE`.
static TEMP_DIR: LazyLock<Regex> = LazyLock::new(|| {
    js_re::regex(
        r"(^|[\\/])(tmp|temp)([\\/])|[\\/]AppData[\\/]Local[\\/]Temp[\\/]",
        true,
    )
});
static ARGUMENT: LazyLock<Regex> =
    LazyLock::new(|| js_re::regex(r#"'([^'\r\n]*)'|"([^"\r\n]*)"|([^\s]+)"#, false));

/// `CommandPathToken`.
#[derive(Clone, Debug, PartialEq)]
struct Token {
    path: String,
    start: usize,
    end: usize,
}

/// `CommandArgument`.
#[derive(Clone, Debug)]
struct Argument {
    value: String,
    start: usize,
    end: usize,
}

fn trim(text: &str) -> &str {
    crate::theme_import::color::js_trim(text)
}

fn ends_with_separator(text: &str) -> bool {
    text.ends_with(['/', '\\'])
}

/// `isPathCandidate`.
fn is_path_candidate(raw: &str) -> bool {
    let s = trim(raw);
    let length = utf16_len(s);
    if !(3..=512).contains(&length)
        || (!EXT.is_match(s) && !ends_with_separator(s))
        || TEMP_DIR.is_match(s)
    {
        return false;
    }
    let bytes = s.as_bytes();
    let drive = bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/');
    drive || s.starts_with('/') || s.contains(['/', '\\'])
}

/// `extractCommandArguments`: words and quoted runs, an odd trailing backslash joining the next.
fn arguments(text: &str, offset: usize) -> Vec<Argument> {
    let mut merged: Vec<(Argument, bool)> = Vec::new();
    for m in ARGUMENT.captures_iter(text) {
        let quoted = m.get(1).or(m.get(2));
        let value = quoted.or(m.get(3)).expect("one alternative");
        let start = offset + value.start();
        let argument = Argument {
            value: value.as_str().to_owned(),
            start,
            end: start + value.len(),
        };
        if let Some((previous, false)) = merged.last_mut()
            && quoted.is_none()
            && previous
                .value
                .bytes()
                .rev()
                .take_while(|b| *b == b'\\')
                .count()
                % 2
                == 1
        {
            let gap = &text[previous.end - offset..argument.start - offset];
            if !gap.is_empty() && gap.bytes().all(|b| b == b' ' || b == b'\t') {
                previous.value.pop();
                previous.value.push(gap.as_bytes()[0] as char);
                previous.value.push_str(&argument.value);
                previous.end = argument.end;
                continue;
            }
        }
        merged.push((argument, quoted.is_some()));
    }
    merged.into_iter().map(|(a, _)| a).collect()
}

/// `text.slice(at - units, at)` in UTF-16 units; a cut pair's half reads as U+FFFD.
fn before(text: &str, at: usize, units: usize) -> Cow<'_, str> {
    let mut left = units;
    let mut start = at;
    for c in text[..at].chars().rev() {
        let width = c.len_utf16();
        if width > left {
            return match left {
                0 => Cow::Borrowed(&text[start..at]),
                _ => Cow::Owned(format!("\u{FFFD}{}", &text[start..at])),
            };
        }
        left -= width;
        start -= c.len_utf8();
    }
    Cow::Borrowed(&text[start..at])
}

/// `text.slice(at, at + units)` in UTF-16 units; a cut pair's half reads as U+FFFD.
fn after(text: &str, at: usize, units: usize) -> Cow<'_, str> {
    let mut left = units;
    let mut end = at;
    for c in text[at..].chars() {
        let width = c.len_utf16();
        if width > left {
            return match left {
                0 => Cow::Borrowed(&text[at..end]),
                _ => Cow::Owned(format!("{}\u{FFFD}", &text[at..end])),
            };
        }
        left -= width;
        end += c.len_utf8();
    }
    Cow::Borrowed(&text[at..end])
}

/// `/^(['"])(.*)\1$/`'s `$2`, or the value itself.
fn unquote(value: &str) -> &str {
    let mut chars = value.chars();
    match (chars.next(), chars.next_back()) {
        (Some(q @ ('\'' | '"')), Some(last)) if last == q => {
            let inner = &value[1..value.len() - 1];
            match inner.contains(['\n', '\r', '\u{2028}', '\u{2029}']) {
                true => value,
                false => inner,
            }
        }
        _ => value,
    }
}

/// `extractCommandOutputPathCandidates`: the written paths, first seen first.
pub fn candidates(command: &str) -> Vec<String> {
    let tokens = tokens::tokens(command);
    let index = index::Index::new(command, &tokens);
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for token in &tokens {
        if !explicit::is_output(&index, token) {
            continue;
        }
        for output in explicit::transfer_outputs(&index, token) {
            if seen.insert(output.clone()) {
                out.push(output);
            }
        }
    }
    for output in converters::outputs(command) {
        if seen.insert(output.clone()) {
            out.push(output);
        }
    }
    out
}

#[cfg(test)]
pub(crate) mod tests;
