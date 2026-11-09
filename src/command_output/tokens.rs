//! `extractCommandPathTokens` and `extractTransferPlainFilenameDestinations`.

use std::{collections::HashSet, sync::LazyLock};

use regex::Regex;

use super::{
    EXT, TEMP_DIR, Token, arguments, ends_with_separator, is_path_candidate, js_re::regex, trim,
    unquote,
};
use crate::theme_import::color::js_space;

static SINGLE: LazyLock<Regex> = LazyLock::new(|| regex(r"'([^'\r\n]+)'", false));
static DOUBLE: LazyLock<Regex> = LazyLock::new(|| regex(r#""([^"\r\n]+)""#, false));
static ABSOLUTE: LazyLock<Regex> =
    LazyLock::new(|| regex(r#"(?:^|[\s=(,>])(\/[^\s'"<>|?*:]+)"#, false));
static URL: LazyLock<Regex> = LazyLock::new(|| regex(r"^https?:\/\/", true));
static TRANSFER: LazyLock<Regex> =
    LazyLock::new(|| regex(r"\b(Copy-Item|Move-Item|copy|move|cp|mv)\b", true));
static TARGET_DIRECTORY: LazyLock<Regex> =
    LazyLock::new(|| regex(r"^(?:-t|--target-directory(?:=|$))", true));
static SWITCH: LazyLock<Regex> = LazyLock::new(|| regex(r"^\/[A-Za-z]+$", false));
static DESTINATION: LazyLock<Regex> =
    LazyLock::new(|| regex(r"^-(?:Destination|LiteralDestination|Target)$", true));
static NAMED_SOURCE: LazyLock<Regex> = LazyLock::new(|| regex(r"^-(?:Path|LiteralPath)$", true));
static FLAG: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r"^-(?:Force|Recurse|PassThru|Container|Confirm|WhatIf)$",
        true,
    )
});

/// A path character in the bare scans: not JS space, a quote, `<>|?*`.
fn bare(c: char) -> bool {
    !js_space(c) && !matches!(c, '\'' | '"' | '<' | '>' | '|' | '?' | '*')
}

fn ends_here(rest: &str, extra: &[char]) -> bool {
    rest.chars().next().is_none_or(|c| {
        js_space(c) || matches!(c, '\'' | '"' | '<' | '>' | '|') || extra.contains(&c)
    })
}

/// `/(?<![A-Za-z0-9])[A-Za-z]:[\\/][^\s'"<>|?*]+/g`.
fn drives(command: &str) -> Vec<(usize, usize)> {
    let b = command.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 2 < b.len() {
        let starts = b[i].is_ascii_alphabetic()
            && b[i + 1] == b':'
            && matches!(b[i + 2], b'\\' | b'/')
            && (i == 0 || !b[i - 1].is_ascii_alphanumeric());
        if starts {
            let run = command[i + 3..]
                .find(|c: char| !bare(c))
                .unwrap_or(b.len() - i - 3);
            if run > 0 {
                out.push((i, i + 3 + run));
                i += 3 + run;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// `RELATIVE_PATH_TOKEN_RE`: a bare run after a start or `\s=(,>`, ending in a separator.
fn relative(command: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < command.len() {
        let c = command[i..].chars().next().expect("in bounds");
        let lead = if i == 0 && bare(c) {
            0
        } else if js_space(c) || "=(,>".contains(c) {
            c.len_utf8()
        } else {
            0
        };
        if lead == 0 && i != 0 {
            i += c.len_utf8();
            continue;
        }
        let from = i + lead;
        let run = command[from..]
            .find(|c: char| !bare(c))
            .unwrap_or(command.len() - from);
        if run > 0
            && ends_with_separator(&command[from..from + run])
            && ends_here(&command[from + run..], &[])
        {
            out.push((from, from + run));
            i = from + run;
        } else if lead == 0 {
            i += c.len_utf8();
        } else {
            i += lead;
        }
    }
    out
}

/// The option or redirect a relative output follows, as its byte length at `i`.
fn output_prefix(command: &str, i: usize) -> Option<usize> {
    let rest = &command[i..];
    let gt = rest.bytes().take_while(|b| *b == b'>').count().min(2);
    if gt > 0 {
        let spaces = rest[gt..]
            .find(|c: char| !js_space(c))
            .unwrap_or(rest.len() - gt);
        return Some(gt + spaces);
    }
    let c = rest.chars().next()?;
    let lead = if js_space(c) { c.len_utf8() } else { 0 };
    let candidates = |at: usize| -> Option<usize> {
        let word = &command[at..];
        let option = [
            "--output-document",
            "--output-file",
            "--outfile",
            "--output",
            "-o",
        ]
        .into_iter()
        .find(|o| word.starts_with(o))?;
        let tail = &word[option.len()..];
        let spaces = tail.find(|c: char| !js_space(c)).unwrap_or(tail.len());
        match (spaces, tail.starts_with('=')) {
            (0, true) => Some(at - i + option.len() + 1),
            (0, false) => None,
            (n, _) => Some(at - i + option.len() + n),
        }
    };
    let at_start = (i == 0).then(|| candidates(0)).flatten();
    at_start.or_else(|| (lead > 0).then(|| candidates(i + lead)).flatten())
}

/// `EXPLICIT_OUTPUT_RELATIVE_TOKEN_RE`, its backtracking unrolled: the group's slash and end.
fn explicit_group(command: &str, from: usize) -> Option<usize> {
    let first = command[from..]
        .find(|c: char| !bare(c) || c == '=')
        .unwrap_or(command.len() - from);
    let second = command[from..]
        .find(|c: char| !bare(c))
        .unwrap_or(command.len() - from);
    let slashes = command[from..from + first]
        .char_indices()
        .filter(|(k, c)| *k > 0 && matches!(c, '/' | '\\'));
    for (k, _) in slashes.collect::<Vec<_>>().into_iter().rev() {
        let ends = (from + k + 1..=from + second)
            .rev()
            .filter(|j| command.is_char_boundary(*j));
        if let Some(j) = ends
            .into_iter()
            .find(|j| ends_here(&command[*j..], &[';', '&']))
        {
            return Some(j);
        }
    }
    None
}

/// The bare relative paths an output option or redirect names.
fn explicit(command: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < command.len() {
        let found = output_prefix(command, i)
            .and_then(|p| explicit_group(command, i + p).map(|end| (i + p, end)));
        match found {
            Some((from, end)) => {
                out.push((from, end));
                i = end;
            }
            None => i += command[i..].chars().next().expect("in bounds").len_utf8(),
        }
    }
    out
}

/// `extractCommandPathTokens`, sorted by place and unique.
pub(super) fn tokens(command: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut quoted: Vec<(usize, usize)> = Vec::new();
    let push = |out: &mut Vec<Token>, raw: &str, start: usize, end: usize| {
        if is_path_candidate(raw) {
            out.push(Token {
                path: trim(raw).to_owned(),
                start,
                end,
            });
        }
    };
    for re in [&*SINGLE, &*DOUBLE] {
        for m in re.captures_iter(command) {
            let (whole, raw) = (m.get(0).expect("match"), m.get(1).expect("group"));
            quoted.push((whole.start(), whole.end()));
            push(&mut out, raw.as_str(), raw.start(), raw.end());
        }
    }
    quoted.sort_by_key(|q| q.0);
    let mut reach = Vec::with_capacity(quoted.len());
    for q in &quoted {
        reach.push(reach.last().copied().unwrap_or(0).max(q.1));
    }
    let inside = |at: usize| {
        let n = quoted.partition_point(|q| q.0 <= at);
        n > 0 && at < reach[n - 1]
    };
    for (start, end) in drives(command) {
        if !inside(start) {
            push(&mut out, &command[start..end], start, end);
        }
    }
    for m in ABSOLUTE.captures_iter(command) {
        let raw = m.get(1).expect("group");
        if !inside(raw.start()) {
            push(&mut out, raw.as_str(), raw.start(), raw.end());
        }
    }
    for (start, end) in relative(command).into_iter().chain(explicit(command)) {
        let raw = &command[start..end];
        if !inside(start) && !URL.is_match(raw) {
            push(&mut out, raw, start, end);
        }
    }
    out.extend(transfer_destinations(command));
    out.sort_by_key(|t| (t.start, t.end));
    let mut seen = HashSet::new();
    out.retain(|t| seen.insert((t.start, t.end, t.path.clone())));
    out
}

fn plain_filename(value: &str) -> bool {
    EXT.is_match(value) && !value.contains(['\\', '/', '<', '>', '|', '?', '*'])
}

/// The arguments of one transfer command: up to the first `|`, `;` or line end.
fn transfer_commands(command: &str) -> Vec<(String, usize, &str)> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(m) = TRANSFER.find_at(command, at) {
        let from = m.end();
        let stop = ['|', ';', '\r', '\n', '\u{2028}', '\u{2029}'];
        let end = command[from..]
            .find(stop)
            .map_or(command.len(), |n| from + n);
        out.push((m.as_str().to_lowercase(), from, &command[from..end]));
        at = end;
    }
    out
}

/// `extractTransferPlainFilenameDestinations`.
fn transfer_destinations(command: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let token = |a: &super::Argument| Token {
        path: a.value.clone(),
        start: a.start,
        end: a.end,
    };
    for (name, start, text) in transfer_commands(command) {
        let args = arguments(text, start);
        if name != "copy-item" && name != "move-item" {
            let option = matches!(name.as_str(), "cp" | "mv")
                .then(|| {
                    args.iter()
                        .position(|a| TARGET_DIRECTORY.is_match(&a.value))
                })
                .flatten();
            if let Some(at) = option {
                let option = &args[at];
                let (raw, from) = match option.value.find('=') {
                    Some(eq) => (Some(&option.value[eq + 1..]), option.start + eq + 1),
                    None => (
                        args.get(at + 1).map(|a| a.value.as_str()),
                        args.get(at + 1).map_or(option.end, |a| a.start),
                    ),
                };
                let destination = raw.map(unquote);
                if let Some(d) = destination.filter(|d| {
                    !d.is_empty() && !TEMP_DIR.is_match(d) && !d.contains(['<', '>', '|', '?', '*'])
                }) {
                    let path = if ends_with_separator(d) {
                        d.to_owned()
                    } else {
                        format!("{d}/")
                    };
                    out.push(Token {
                        path,
                        start: from,
                        end: from + d.len(),
                    });
                }
                continue;
            }
            let positional: Vec<_> = args
                .iter()
                .filter(|a| {
                    !a.value.starts_with('-')
                        && !(matches!(name.as_str(), "copy" | "move") && SWITCH.is_match(&a.value))
                })
                .collect();
            if positional.len() >= 2 && plain_filename(&positional[positional.len() - 1].value) {
                out.push(token(positional[positional.len() - 1]));
            }
            continue;
        }
        if let Some(at) = args.iter().position(|a| DESTINATION.is_match(&a.value)) {
            if let Some(d) = args.get(at + 1).filter(|d| plain_filename(&d.value)) {
                out.push(token(d));
            }
            continue;
        }
        let (mut positional, mut named) = (Vec::new(), false);
        let mut index = 0;
        while index < args.len() {
            let arg = &args[index];
            if !arg.value.starts_with('-') {
                positional.push(arg);
            } else {
                named |= NAMED_SOURCE.is_match(&arg.value);
                if !FLAG.is_match(&arg.value) {
                    index += 1;
                }
            }
            index += 1;
        }
        if let Some(d) = positional
            .get(if named { 0 } else { 1 })
            .filter(|d| plain_filename(&d.value))
        {
            out.push(token(d));
        }
    }
    out
}
