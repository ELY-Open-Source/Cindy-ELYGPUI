//! `repairStreamingMarkdown.ts` and `streamingMarkdownChunks.ts`: a growing answer's Markdown.
use crate::theme_import::color::{js_space, js_trim};

pub mod chunks;
#[cfg(test)]
mod tests;

/// JavaScript's line terminators: where a regex's `.` stops.
fn terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// `split(/\r?\n/)`.
fn lines(md: &str) -> impl Iterator<Item = &str> {
    let last = md.matches('\n').count();
    let pieces = md.split('\n').enumerate();
    pieces.map(move |(nth, line)| match nth < last {
        true => line.strip_suffix('\r').unwrap_or(line),
        false => line,
    })
}

/// A fence line's run of marks and the rest, after up to three of `lead`.
fn fence(line: &str, lead: fn(char) -> bool) -> Option<(&str, &str)> {
    let from = line.trim_start_matches(lead);
    if line[..line.len() - from.len()].chars().count() > 3 {
        return None;
    }
    let mark = from.chars().next().filter(|c| matches!(c, '`' | '~'))?;
    let rest = from.trim_start_matches(mark);
    let run = &from[..from.len() - rest.len()];
    (run.len() >= 3 && !rest.contains(terminator)).then_some((run, rest))
}

/// The fence a line opens or closes: the open fence after it.
fn fenced<'a>(open: Option<&'a str>, run: &'a str, closes: bool) -> Option<&'a str> {
    match open {
        None => Some(run),
        Some(at) if run.starts_with(&at[..1]) && run.len() >= at.len() && closes => None,
        Some(at) => Some(at),
    }
}

/// `scanFences`: the fence left open and the text outside every fence.
fn scan(md: &str) -> (Option<&str>, String) {
    let (mut open, mut outside) = (None, Vec::new());
    for line in lines(md) {
        match fence(line, js_space) {
            Some((run, rest)) => {
                let closes = rest.chars().all(|c| matches!(c, ' ' | '\t'));
                open = fenced(open, run, closes);
            }
            None if open.is_none() => outside.push(line),
            None => {}
        }
    }
    (open, outside.join("\n"))
}

/// `isEscaped`: an odd run of backslashes stands before `at`.
fn escaped(text: &str, at: usize) -> bool {
    let before = text.as_bytes()[..at].iter().rev();
    before.take_while(|b| **b == b'\\').count() % 2 == 1
}

/// `isBulletMarker`: the star starts its line and a space or tab follows.
fn bullet(text: &str, at: usize) -> bool {
    let b = text.as_bytes();
    if !matches!(b.get(at + 1), Some(b' ' | b'\t')) {
        return false;
    }
    let before = b[..at].iter().rev();
    let lead = before.take_while(|c| matches!(c, b' ' | b'\t')).count();
    lead == at || b[at - lead - 1] == b'\n'
}

/// `scanDelim`: how many runs of `width` stars, where the last starts, none after a longer run.
fn stars(text: &str, width: usize) -> Option<(usize, usize)> {
    let b = text.as_bytes();
    let (mut count, mut last, mut i) = (0, 0, 0);
    while i < b.len() {
        if b[i] != b'*' || escaped(text, i) {
            i += 1;
            continue;
        }
        let run = b[i..].iter().take_while(|c| **c == b'*').count();
        if run >= 3 {
            return None;
        }
        if run == width && (width == 2 || !bullet(text, i)) {
            (count, last) = (count + 1, i);
        }
        i += run;
    }
    Some((count, last))
}

/// `balanceEmphasis`: the mark an open emphasis on the last line lacks.
fn balance(md: &mut String, delim: &str) {
    let Some((_, last)) = stars(md, delim.len()).filter(|(count, _)| count % 2 == 1) else {
        return;
    };
    let rest = &md[last + delim.len()..];
    let grows = !js_trim(rest).is_empty() && !rest.contains(['\n', '`']);
    if grows && !rest.starts_with(js_space) {
        md.push_str(delim);
    }
}

/// Where a label's text ends when `](` follows it and no `)` or line feed after that.
fn labelled(md: &str, from: usize) -> Option<usize> {
    let end = from + md[from..].find([']', '\n'])?;
    let tail = md[end..].strip_prefix("](")?;
    (!tail.contains([')', '\n'])).then_some(end)
}

/// `repairTrailingImage`: an image still being written reads as its alt text.
fn image(md: &mut String) {
    let marks = |md: &str| md.match_indices("![").map(|(at, _)| at).collect::<Vec<_>>();
    let whole = (marks(md).into_iter()).find_map(|at| Some((at, labelled(md, at + 2)?)));
    if let Some((at, end)) = whole {
        *md = format!("{}{}", &md[..at], &md[at + 2..end]);
    }
    let half = (marks(md).into_iter()).find(|at| !md[at + 2..].contains([']', '\n']));
    if let Some(at) = half {
        md.replace_range(at..at + 2, "");
    }
}

/// `repairTrailingLink`: a link still being written reads as its text.
fn link(md: &mut String) {
    let found = md.match_indices('[').find_map(|(at, _)| {
        let before = md[..at].chars().next_back();
        let opens = !matches!(before, Some('!' | '\\'));
        Some((at, labelled(md, at + 1).filter(|_| opens)?))
    });
    if let Some((at, end)) = found {
        *md = format!("{}{}", &md[..at], &md[at + 1..end]);
    }
}

/// `repairStreamingMarkdown`: the text with its last, unfinished syntax mended.
pub fn repair(md: &str) -> String {
    let (open, outside) = scan(md);
    if let Some(mark) = open {
        let feed = if md.ends_with('\n') { "" } else { "\n" };
        return format!("{md}{feed}{mark}");
    }
    let ticks = outside.match_indices('`');
    if ticks.filter(|(at, _)| !escaped(&outside, *at)).count() % 2 == 1 {
        return md.to_owned();
    }
    let mut text = md.to_owned();
    image(&mut text);
    link(&mut text);
    balance(&mut text, "**");
    balance(&mut text, "*");
    text
}

/// `getStreamingMarkdownThrottleInterval`, in milliseconds.
pub fn throttle(md: &str) -> u64 {
    match md.encode_utf16().count() {
        0..=12_000 => 100,
        12_001..=32_000 => 125,
        32_001..=80_000 => 175,
        80_001..=160_000 => 250,
        160_001..=320_000 => 325,
        _ => 400,
    }
}
