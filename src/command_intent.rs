//! `commandIntent.ts`: a shell command's intent, only for shapes without side effects.

mod git;
mod rules;
mod shell;
mod single;
mod tools;

use shell::{basename, binary_name, split_top_level, strip_prefix_tokens, tokenize};

use crate::{
    rich_answer::js::{Js, non_empty},
    theme_import::color::js_trim,
};

/// `CommandIntent`: the action's name as Cindy spells it, with its target and path.
#[derive(Clone, Debug, PartialEq)]
pub struct Intent {
    pub action: &'static str,
    pub target: Option<String>,
    pub path: Option<String>,
}

impl Intent {
    fn new(action: &'static str) -> Self {
        Self {
            action,
            target: None,
            path: None,
        }
    }

    /// A target only when it is nonempty, as `...(x ? { target: x } : {})` adds it.
    fn target(mut self, target: Option<&str>) -> Self {
        self.target = target.filter(|t| !t.is_empty()).map(str::to_owned);
        self
    }

    fn path(mut self, path: Option<&str>) -> Self {
        self.path = path.filter(|p| !p.is_empty()).map(str::to_owned);
        self
    }
}

/// `basenameRemotePath(file) || file`.
pub fn base_or(file: &str) -> &str {
    let base = basename(file);
    if base.is_empty() { file } else { base }
}

/// `COMMAND_MAX_CHARS`, in UTF-16 units.
const MAX_CHARS: usize = 8192;

/// `READ_COMMAND_BINS`.
const READ_BINS: [&str; 8] = ["cat", "head", "tail", "less", "more", "sed", "bat", "nl"];

/// `PIPE_FILTERS`: display-only filters a pipeline may end in.
const PIPE_FILTERS: [&str; 17] = [
    "head", "tail", "wc", "sort", "uniq", "less", "more", "cat", "column", "nl", "sed", "cut",
    "tr", "grep", "egrep", "fgrep", "rg",
];

const CUT_VALUE_FLAGS: [&str; 9] = [
    "-b",
    "-c",
    "-d",
    "-f",
    "--bytes",
    "--characters",
    "--delimiter",
    "--fields",
    "--output-delimiter",
];

/// `commandIntentsFromActions`: Codex's `commandActions` that are safe to show.
pub fn intents_from_actions(raw: Option<&Js>, full: Option<&str>) -> Vec<Intent> {
    let Some(entries) = raw.and_then(Js::arr) else {
        return Vec::new();
    };
    let mut intents = Vec::new();
    for action in entries.iter().filter_map(Js::obj) {
        let command = non_empty(action.get("command"));
        let path = non_empty(action.get("path"));
        match action.get("type").and_then(Js::str) {
            Some("read") => {
                if !tools::is_known_read_command(command) {
                    continue;
                }
                let name = non_empty(action.get("name")).or_else(|| path.map(basename));
                if let Some(name) = name.filter(|n| !n.is_empty()) {
                    intents.push(Intent::new("read").target(Some(name)).path(path));
                }
            }
            Some("listFiles") if !tools::search_has_side_effects(command) => {
                intents.push(Intent::new("list").target(path));
            }
            Some("search") if !tools::search_has_side_effects(command) => {
                match (non_empty(action.get("query")), path) {
                    (Some(query), path) => {
                        intents.push(Intent::new("search").target(Some(query)).path(path))
                    }
                    (None, Some(path)) => intents.push(Intent::new("search").target(Some(path))),
                    (None, None) => {}
                }
            }
            _ => {}
        }
    }
    if let Some(full) = full
        && analyze(full).is_none()
        && !matches_safe_composite(full, &intents)
    {
        return Vec::new();
    }
    intents
}

/// `structuredActionsMatchSafeComposite`: a sequential chain of reads, lists and searches.
fn matches_safe_composite(full: &str, intents: &[Intent]) -> bool {
    let plain = |i: &Intent| matches!(i.action, "read" | "list" | "search");
    if intents.is_empty() || full.contains("||") || !intents.iter().all(plain) {
        return false;
    }
    composite(full).is_some_and(|c| c.action == "inspect" || plain(&c))
}

/// `commandIntentFromActions`: the first of them.
pub fn from_actions(raw: Option<&Js>, full: Option<&str>) -> Option<Intent> {
    intents_from_actions(raw, full).into_iter().next()
}

/// `commandIntentFromCommand`.
pub fn from_command(command: &str) -> Option<Intent> {
    composite(command).or_else(|| single::intent(command))
}

/// `/[`\n]|\$\(|<</` without the newline, which the caller adds.
fn substitution(text: &str) -> bool {
    text.contains('`') || text.contains("$(") || text.contains("<<")
}

/// `/^cd(\s|$)/`.
fn is_cd(segment: &str) -> bool {
    segment.strip_prefix("cd").is_some_and(|rest| {
        rest.chars()
            .next()
            .is_none_or(crate::theme_import::color::js_space)
    })
}

/// `analyzeCommandShape`: the first segment's argv when every check passes.
fn analyze(command: &str) -> Option<Vec<String>> {
    let trimmed = js_trim(command);
    if trimmed.is_empty() || trimmed.encode_utf16().count() > MAX_CHARS {
        return None;
    }
    if trimmed.contains('\n') || substitution(trimmed) {
        return None;
    }
    let mut chain = split_top_level(trimmed, &["&&", ";", "&"])?;
    while chain.len() > 1 && is_cd(&chain[0]) {
        if !clean_cd(&chain[0]) {
            return None;
        }
        chain.remove(0);
    }
    let [segment] = chain.as_slice() else {
        return None;
    };
    let pipeline = split_top_level(segment, &["|"])?;
    let (first, tails) = pipeline.split_first()?;
    for tail in tails {
        pipe_tail_ok(tail)?;
    }
    let argv = strip_prefix_tokens(&tokenize(first)?)?;
    (!argv.is_empty()).then_some(argv)
}

/// One pipeline tail: a display filter that only reads its stdin.
fn pipe_tail_ok(segment: &str) -> Option<()> {
    let words = tokenize(segment).filter(|w| !w.is_empty())?;
    let tail = strip_prefix_tokens(&words).filter(|t| !t.is_empty())?;
    let bin = binary_name(&tail[0]);
    let rest = &tail[1..];
    if !PIPE_FILTERS.contains(&bin.as_str()) {
        return None;
    }
    match bin.as_str() {
        "sed" => tools::sed_pipeline_filter_is_read_only(rest).then_some(()),
        "grep" | "egrep" | "fgrep" | "rg" => {
            let intent = tools::grep(rest)?;
            let ok = intent.action == "search"
                && intent.path.is_none()
                && !tools::search_has_side_effects(Some(segment));
            ok.then_some(())
        }
        "cut" => shell::positionals(rest, &CUT_VALUE_FLAGS)
            .is_empty()
            .then_some(()),
        "tr" => Some(()),
        _ => {
            let writes = rest.iter().any(|token| {
                let lower = token.to_lowercase();
                ["-o", "--output", "--log-file"]
                    .iter()
                    .any(|p| lower.starts_with(p))
            });
            let counts = shell::positionals(rest, &["-n", "-c"])
                .iter()
                .all(|t| signed_integer(t));
            (!writes && counts).then_some(())
        }
    }
}

/// `/^[+-]?\d+$/`.
fn signed_integer(token: &str) -> bool {
    let digits = token.strip_prefix(['+', '-']).unwrap_or(token);
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

/// `isCleanCdSegment`: `cd` and at most one argument, no redirect.
fn clean_cd(segment: &str) -> bool {
    tokenize(segment)
        .and_then(|words| strip_prefix_tokens(&words))
        .is_some_and(|argv| argv.len() <= 2 && binary_name(argv.first().map_or("", |a| a)) == "cd")
}

mod composite;
use composite::composite;

#[cfg(test)]
mod tests;
