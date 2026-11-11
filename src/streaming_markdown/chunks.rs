//! `splitStreamingMarkdownChunks`: the closed top-level blocks and the one that grows.

use super::{fence, fenced, lines, terminator};
use crate::theme_import::color::{js_space, js_trim};

/// A piece of the text and the byte it starts at, its stable key.
#[derive(Debug, PartialEq)]
pub struct Chunk<'a> {
    pub start: usize,
    pub content: &'a str,
}

fn blank(c: char) -> bool {
    matches!(c, ' ' | '\t')
}

/// A line's text after up to three spaces.
fn indented(line: &str) -> Option<&str> {
    let text = line.trim_start_matches(' ');
    (line.len() - text.len() <= 3).then_some(text)
}

/// `isTopLevelContinuation`: indented, a list item or a quote.
fn continues(line: &str) -> bool {
    let digits = line.trim_start_matches(|c: char| c.is_ascii_digit());
    let numbered = (1..=9).contains(&(line.len() - digits.len()));
    let item = match (line.strip_prefix(['-', '+', '*']), numbered) {
        (Some(rest), _) => Some(rest),
        (None, true) => digits.strip_prefix(['.', ')']),
        (None, false) => None,
    };
    line.starts_with([' ', '\t', '>']) || item.is_some_and(|rest| rest.starts_with(blank))
}

/// Where a multiline regex's `^` matches.
fn starts(md: &str) -> impl Iterator<Item = &str> {
    let after = md.char_indices().filter(|(_, c)| terminator(*c));
    let after = after.map(|(at, c)| &md[at + c.len_utf8()..]);
    std::iter::once(md).chain(after)
}

/// `hasGlobalMarkdownDefinitions`: a line that reads as a reference definition.
fn defined(md: &str) -> bool {
    let label = |text: &str| {
        let mut chars = text.strip_prefix('[')?.chars();
        let mut count = 0;
        loop {
            match chars.next()? {
                ']' => return (count > 0 && chars.next() == Some(':')).then_some(()),
                '\n' | '\r' => return None,
                '\\' => chars
                    .next()
                    .filter(|c| !matches!(c, '\n' | '\r'))
                    .map(drop)?,
                _ => {}
            }
            count += 1;
        }
    };
    starts(md).any(|line| indented(line).and_then(label).is_some())
}

/// `hasMultipleHeadings`, outside fences.
fn headed(md: &str) -> bool {
    let lines: Vec<&str> = lines(md).collect();
    let (mut count, mut open) = (0, None);
    for (nth, line) in lines.iter().enumerate() {
        if let Some((run, rest)) = fence(line, |c| c == ' ') {
            open = fenced(open, run, js_trim(rest).is_empty());
            continue;
        }
        if open.is_some() {
            continue;
        }
        let atx = indented(line).is_some_and(|text| {
            let rest = text.trim_start_matches('#');
            let marks = text.len() - rest.len();
            (1..=6).contains(&marks) && (rest.is_empty() || rest.starts_with(blank))
        });
        let under = lines.get(nth + 1).and_then(|next| indented(next));
        let setext = !js_trim(line).is_empty()
            && under.is_some_and(|text| {
                let rule = text.trim_end_matches(blank);
                let mark = rule.chars().next().filter(|c| matches!(c, '=' | '-'));
                mark.is_some_and(|mark| rule.chars().all(|c| c == mark))
            });
        count += usize::from(atx || setext);
        if count > 1 {
            return true;
        }
    }
    false
}

/// `hasPotentialHtmlBlock`: a line that may open an HTML block.
fn tagged(md: &str) -> bool {
    let opens = |text: &str| {
        let Some(text) = text.strip_prefix('<') else {
            return false;
        };
        if text.starts_with("!--") || text.starts_with('?') || text.starts_with("![CDATA[") {
            return true;
        }
        if let Some(rest) = text.strip_prefix('!') {
            return rest.starts_with(|c: char| c.is_ascii_alphabetic());
        }
        let name = text.strip_prefix('/').unwrap_or(text);
        let rest = name.trim_start_matches(|c: char| c.is_ascii_alphanumeric() || c == '-');
        let named = name.starts_with(|c: char| c.is_ascii_alphabetic());
        let ends = |c: char| matches!(c, '\t' | ' ' | '/' | '>') || terminator(c);
        named && (rest.is_empty() || rest.starts_with(ends))
    };
    starts(md).any(|line| indented(line).is_some_and(opens))
}

/// `splitStreamingMarkdownChunks`.
pub fn split(md: &str) -> Vec<Chunk<'_>> {
    let whole = || {
        vec![Chunk {
            start: 0,
            content: md,
        }]
    };
    let parted = md.contains("\n\n") || md.contains("\r\n\r\n");
    if !parted || defined(md) || headed(md) || tagged(md) {
        return whole();
    }
    let (mut bounds, mut pending) = (Vec::new(), None);
    let (mut open, mut math, mut depth) = (None, false, 0);
    let mut start = 0;
    loop {
        let feed = md[start..].find('\n').map(|at| start + at);
        let line = &md[start..feed.unwrap_or(md.len())];
        let line = line.strip_suffix('\r').unwrap_or(line);
        let trimmed = line.trim_start_matches(js_space);
        if let Some(bound) = pending.filter(|_| !trimmed.is_empty()) {
            if !continues(line) {
                bounds.push(bound);
            }
            pending = None;
        }
        let dollars = indented(line).and_then(|text| text.strip_prefix("$$"));
        let colons = trimmed.strip_prefix(":::");
        let named = colons.is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_alphabetic()));
        if let Some((run, rest)) = fence(line, |c| c == ' ') {
            open = fenced(open, run, js_trim(rest).is_empty());
        } else if open.is_none()
            && depth == 0
            && dollars.is_some_and(|rest| rest.chars().all(blank))
        {
            math = !math;
        } else if open.is_none() && !math && named {
            depth += 1;
        } else if open.is_none() && !math && depth > 0 {
            depth -= usize::from(colons.is_some_and(|rest| rest.chars().all(blank)));
        }
        let Some(feed) = feed else {
            break;
        };
        if open.is_none() && !math && depth == 0 && trimmed.is_empty() {
            pending = Some(feed + 1);
        }
        start = feed + 1;
    }
    if bounds.is_empty() {
        return whole();
    }
    let mut out = Vec::new();
    let mut from = 0;
    for bound in bounds {
        out.push(Chunk {
            start: from,
            content: &md[from..bound],
        });
        from = bound;
    }
    out.push(Chunk {
        start: from,
        content: &md[from..],
    });
    out
}
