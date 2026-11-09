//! `CommandScanIndex`: cmdlets, separators and segments, scanned once per command.

use std::{cell::RefCell, collections::HashMap, sync::LazyLock};

use regex::Regex;

use super::{Token, js_re::regex, trim};
use crate::theme_import::color::js_space;

static CMDLET: LazyLock<Regex> =
    LazyLock::new(|| regex(r"\b[A-Za-z][A-Za-z0-9]*-[A-Za-z][A-Za-z0-9-]*\b", false));
const WRITE_CMDLETS: [&str; 6] = [
    "out-file",
    "set-content",
    "add-content",
    "export-csv",
    "export-clixml",
    "new-item",
];
static COPY_MOVE_ITEM: LazyLock<Regex> =
    LazyLock::new(|| regex(r"(?:^|\|\s*)(?:Copy-Item|Move-Item)\b", true));
static EXPLICIT_DESTINATION: LazyLock<Regex> =
    LazyLock::new(|| regex(r"-(?:Destination|LiteralDestination|Target)\s+", true));
static CP_MV: LazyLock<Regex> = LazyLock::new(|| regex(r"(?:^|\|\s*)(?:cp|mv)\s+", true));
static TARGET_OPTION: LazyLock<Regex> =
    LazyLock::new(|| regex(r"(?:^|\s)(?:-t(?:\s+|$)|--target-directory(?:\s+|=))", true));
static TRANSFER: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r"(?:^|\|\s*)(?:cp|copy|mv|move|Copy-Item|Move-Item)\s+",
        true,
    )
});

#[derive(Clone, Copy)]
struct Cmdlet {
    start: usize,
    end: usize,
}

/// `PowerShellTailScan`: the first top-level separator and each bracket's depth after it.
struct Tail {
    terminator: usize,
    events: Vec<(usize, usize)>,
}

/// `CommandSegmentInfo`.
#[derive(Clone)]
pub(super) struct Segment {
    pub start: usize,
    pub end: usize,
    pub copy_move_item: bool,
    pub explicit_destination: bool,
    pub target_directory_transfer: bool,
    pub transfer_command: bool,
    pub last_path: Option<Token>,
}

pub(super) struct Index<'a> {
    pub command: &'a str,
    tokens: &'a [Token],
    cmdlets: Vec<Cmdlet>,
    writes: Vec<Cmdlet>,
    /// Separator start and length: `;`, a newline, `&&` or `||`.
    separators: Vec<(usize, usize)>,
    segments: RefCell<HashMap<(usize, usize), Segment>>,
    tails: RefCell<HashMap<usize, std::rc::Rc<Tail>>>,
}

/// The last cmdlet ending at or before `limit`.
fn last_ending_before(list: &[Cmdlet], limit: usize) -> Option<Cmdlet> {
    let n = list.partition_point(|c| c.end <= limit);
    n.checked_sub(1).map(|i| list[i])
}

/// `endsWithPowerShellPathSwitch` over `command[from..to]`.
fn ends_with_path_switch(fragment: &str) -> bool {
    let text = fragment.strip_suffix(['\'', '"']).unwrap_or(fragment);
    let body = text.trim_end_matches(js_space);
    body.len() < text.len()
        && ["-FilePath", "-LiteralPath", "-Path"].iter().any(|s| {
            body.len() >= s.len()
                && body.as_bytes()[body.len() - s.len()..].eq_ignore_ascii_case(s.as_bytes())
        })
}

/// `isBlankOrQuote`: `/^\s*['"]?$/`.
fn blank_or_quote(fragment: &str) -> bool {
    let rest = fragment.trim_start_matches(js_space);
    rest.is_empty() || rest == "'" || rest == "\""
}

/// `scanPowerShellTail` over `command[from..to]`.
fn scan_tail(command: &str, from: usize, to: usize) -> Tail {
    let mut tail = Tail {
        terminator: usize::MAX,
        events: Vec::new(),
    };
    let (mut depth, mut quote) = (0usize, None);
    let mut chars = command[from..to].char_indices();
    while let Some((offset, c)) = chars.next() {
        let at = from + offset;
        if c == '`' {
            chars.next();
            continue;
        }
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '\'' | '"' => quote = Some(c),
            '(' | '[' | '{' => {
                depth += 1;
                tail.events.push((at, depth));
            }
            ')' | ']' | '}' => {
                depth = depth.saturating_sub(1);
                tail.events.push((at, depth));
            }
            ';' | '|' | '\r' | '\n' if depth == 0 => {
                tail.terminator = at;
                break;
            }
            _ => {}
        }
    }
    tail
}

/// `isTopLevelPowerShellTailAt`.
fn top_level_at(tail: &Tail, to: usize) -> bool {
    if tail.terminator < to {
        return false;
    }
    let n = tail.events.partition_point(|e| e.0 < to);
    n.checked_sub(1).is_none_or(|i| tail.events[i].1 == 0)
}

impl<'a> Index<'a> {
    pub fn new(command: &'a str, tokens: &'a [Token]) -> Self {
        let cmdlets: Vec<Cmdlet> = CMDLET
            .find_iter(command)
            .map(|m| Cmdlet {
                start: m.start(),
                end: m.end(),
            })
            .collect();
        let writes = CMDLET
            .find_iter(command)
            .filter(|m| WRITE_CMDLETS.contains(&m.as_str().to_ascii_lowercase().as_str()))
            .map(|m| Cmdlet {
                start: m.start(),
                end: m.end(),
            })
            .collect();
        let b = command.as_bytes();
        let mut separators = Vec::new();
        for i in 0..b.len() {
            if b[i] == b';' || b[i] == b'\n' {
                separators.push((i, 1));
            } else if (b[i] == b'&' || b[i] == b'|') && b.get(i + 1) == Some(&b[i]) {
                separators.push((i, 2));
            }
        }
        Index {
            command,
            tokens,
            cmdlets,
            writes,
            separators,
            segments: RefCell::default(),
            tails: RefCell::default(),
        }
    }

    /// `isPowerShellOutputPosition`.
    pub fn powershell_output(&self, token_start: usize) -> bool {
        let (Some(last), Some(write)) = (
            last_ending_before(&self.cmdlets, token_start),
            last_ending_before(&self.writes, token_start),
        ) else {
            return false;
        };
        if ends_with_path_switch(&self.command[write.end..token_start])
            && top_level_at(&self.tail(write), token_start)
        {
            return true;
        }
        last.start == write.start && blank_or_quote(&self.command[last.end..token_start])
    }

    /// The scan after a write cmdlet, up to the next one; computed once per cmdlet.
    fn tail(&self, write: Cmdlet) -> std::rc::Rc<Tail> {
        if let Some(tail) = self.tails.borrow().get(&write.start) {
            return tail.clone();
        }
        let next = self
            .writes
            .iter()
            .find(|c| c.start > write.start)
            .map_or(self.command.len(), |c| c.start);
        let tail = std::rc::Rc::new(scan_tail(self.command, write.end, next));
        self.tails.borrow_mut().insert(write.start, tail.clone());
        tail
    }

    /// `tokensWithin`: tokens starting in `[from, to)` and ending by `to`.
    pub fn within(&self, from: usize, to: usize) -> impl Iterator<Item = &'a Token> + 'a {
        let first = self.tokens.partition_point(|t| t.start < from);
        self.tokens[first..]
            .iter()
            .take_while(move |t| t.start < to)
            .filter(move |t| t.end <= to)
    }

    /// `segmentFor`: the token's command, between the separators around it.
    pub fn segment(&self, start: usize, end: usize) -> Segment {
        let before = self.separators.partition_point(|s| s.0 < start);
        let from = before
            .checked_sub(1)
            .map_or(0, |i| self.separators[i].0 + self.separators[i].1);
        let after = self.separators.partition_point(|s| s.0 < end);
        let to = self
            .separators
            .get(after)
            .map_or(self.command.len(), |s| s.0);
        if let Some(segment) = self.segments.borrow().get(&(from, to)) {
            return segment.clone();
        }
        let text = &self.command[from..to];
        let trimmed = trim(text);
        let segment = Segment {
            start: from,
            end: to,
            copy_move_item: COPY_MOVE_ITEM.is_match(trimmed),
            explicit_destination: EXPLICIT_DESTINATION.is_match(text),
            target_directory_transfer: CP_MV.is_match(trimmed) && TARGET_OPTION.is_match(text),
            transfer_command: TRANSFER.is_match(trimmed),
            last_path: self.within(from, to).last().cloned(),
        };
        self.segments
            .borrow_mut()
            .insert((from, to), segment.clone());
        segment
    }
}
