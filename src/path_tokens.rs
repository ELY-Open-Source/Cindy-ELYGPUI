//! `bareFilePathTokens.ts`: the bare file paths a plain text holds. Ranges are bytes.
#![allow(dead_code)] // The tree's reader comes with 9.6e.

use std::ops::Range;

use crate::theme_import::color::js_space;

#[cfg(test)]
mod tests;

/// `CJK`: ideographs, kana and hangul, which a path segment may hold.
fn cjk(c: char) -> bool {
    matches!(c, '\u{4e00}'..='\u{9fff}' | '\u{3400}'..='\u{4dbf}' | '\u{3040}'..='\u{30ff}' | '\u{ac00}'..='\u{d7af}')
}

fn sep(c: char) -> bool {
    matches!(c, '/' | '\\')
}

/// `TOKEN_CHAR_RE`, also `RIGHT_BOUNDARY`'s class.
fn token(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '~' | '@' | '+' | '-') || cjk(c) || sep(c)
}

/// `SEG`'s class.
fn segment(c: char) -> bool {
    c == '.' || (token(c) && !sep(c))
}

/// `ALLOWED_BEFORE_RE`.
fn allowed_before(c: char) -> bool {
    "([{（【「『\"'`:：,;，；、>".contains(c)
}

/// The text as characters, each with its byte offset.
struct Text {
    chars: Vec<char>,
    bytes: Vec<usize>,
}

impl Text {
    fn new(text: &str) -> Self {
        let mut bytes: Vec<usize> = text.char_indices().map(|(at, _)| at).collect();
        bytes.push(text.len());
        Self {
            chars: text.chars().collect(),
            bytes,
        }
    }

    fn is(&self, at: usize, test: impl Fn(char) -> bool) -> bool {
        self.chars.get(at).is_some_and(|c| test(*c))
    }

    /// The end of the run of `test` from `at`.
    fn run(&self, mut at: usize, test: impl Fn(char) -> bool) -> usize {
        while self.is(at, &test) {
            at += 1;
        }
        at
    }

    /// `SEG\.[A-Za-z0-9]{1,10}`, the longest segment first; `ends_mid` is its right boundary.
    fn last_segment(&self, start: usize) -> Option<usize> {
        let run = self.run(start, segment);
        (start + 1..run).rev().find_map(|dot| {
            let end = self.run(dot + 1, |c| c.is_ascii_alphanumeric());
            let fits = self.chars[dot] == '.' && (1..=10).contains(&(end - dot - 1));
            fits.then_some(end)
        })
    }

    /// `(?:SEG SEP)*` from `start`, at least `least` of them, then the last segment.
    fn tail(&self, start: usize, least: usize) -> Option<usize> {
        let mut starts = vec![start];
        loop {
            let from = starts[starts.len() - 1];
            let run = self.run(from, segment);
            if run == from || !self.is(run, sep) {
                break;
            }
            starts.push(run + 1);
        }
        starts[least.min(starts.len())..]
            .iter()
            .rev()
            .find_map(|from| self.last_segment(*from))
    }

    /// `ANCHOR`'s ends at `at`: a drive or a root; a dotted one reads the same as a segment.
    fn anchors(&self, at: usize) -> impl Iterator<Item = usize> + '_ {
        let drive = self.is(at, |c| c.is_ascii_alphabetic()) && self.is(at + 1, |c| c == ':');
        let drive = (drive && self.is(at + 2, sep)).then_some(at + 3);
        let root = self.is(at, sep).then_some(at + 1);
        drive.into_iter().chain(root)
    }

    /// `LINE_SUFFIX` once: a colon, then a number of up to seven digits with no leading zero.
    fn number(&self, at: usize) -> usize {
        if !self.is(at, |c| c == ':') || !self.is(at + 1, |c| matches!(c, '1'..='9')) {
            return at;
        }
        self.run(at + 2, |c| c.is_ascii_digit()).min(at + 8)
    }

    /// `PATH_RE` at `at`: the end of the match.
    fn path(&self, at: usize) -> Option<usize> {
        let before = |c: char| segment(c) && c != '-';
        if at > 0 && (before(self.chars[at - 1]) || sep(self.chars[at - 1])) {
            return None;
        }
        if self.chars[at] == '~' && self.is(at + 1, sep) {
            return None;
        }
        let body =
            (self.anchors(at).find_map(|end| self.tail(end, 0))).or_else(|| self.tail(at, 1))?;
        let line = self.number(body);
        Some(match line == body {
            true => body,
            false => self.number(line),
        })
    }

    /// `startsMidPathToken`.
    fn starts_mid(&self, start: usize) -> bool {
        if start == 0 {
            return false;
        }
        if !js_space(self.chars[start - 1]) {
            return !allowed_before(self.chars[start - 1]);
        }
        let blank = |c: &char| matches!(c, ' ' | '\t');
        let end = start
            - self.chars[..start]
                .iter()
                .rev()
                .take_while(|c| blank(c))
                .count();
        let run = end
            - self.chars[..end]
                .iter()
                .rev()
                .take_while(|c| !js_space(**c))
                .count();
        let run = &self.chars[run..end];
        if !run.iter().any(|c| sep(*c)) {
            return false;
        }
        let ext = run
            .iter()
            .rev()
            .take_while(|c| c.is_ascii_alphanumeric())
            .count();
        !((1..=10).contains(&ext) && ext < run.len() && run[run.len() - ext - 1] == '.')
    }

    /// `endsMidPathToken`.
    fn ends_mid(&self, end: usize) -> bool {
        self.is(self.run(end, |c| matches!(c, '.' | ':')), token)
    }
}

/// `findPathMatches`.
pub fn find(text: &str) -> Vec<Range<usize>> {
    let text = Text::new(text);
    let mut found = Vec::new();
    let mut at = 0;
    while at < text.chars.len() {
        let Some(end) = text.path(at) else {
            at += 1;
            continue;
        };
        if !text.starts_mid(at) && !text.ends_mid(end) {
            found.push(text.bytes[at]..text.bytes[end]);
        }
        at = end;
    }
    found
}
