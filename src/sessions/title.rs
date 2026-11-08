//! `sessionDisplayTitle.ts`: a task's stored title as the sidebar shows it.

use std::sync::LazyLock;

use regex::Regex;

use super::group::Session;
use crate::i18n::{Translator, cross_locale};

/// `DEFAULT_DRAFT_SESSION_TITLE`, the untitled sentinel.
const DRAFT: &str = "New Maker";
const SCHEDULE_PREFIX: &str = "[Schedule] ";
const MAKE: &str = "cindy-make";
const MAKE_MERGE: &str = "cindy-make-merge";
const MAKE_MERGE_TITLE_KEYS: [&str; 3] = [
    "cindyMake.merge.taskTitle",
    "cindyMake.history.mergeTaskTitle",
    "cindyMake.history.revertTaskTitle",
];

static WORKTREE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:^|[\\/])(?:merge-)?worktrees[\\/]([A-Za-z0-9-]{1,64})[\\/]?$")
        .expect("valid pattern")
});
static MAKE_TAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\[Cindy(?:-| )Make\][\s\x{FEFF}]*").expect("valid pattern"));

/// JavaScript's `trimStart`.
fn trim_start(text: &str) -> &str {
    text.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
}

/// `cindyMakeWorktreeName`.
fn worktree_name(working_dir: Option<&str>) -> Option<&str> {
    WORKTREE
        .captures(working_dir?)
        .map(|c| c.get(1).expect("one group").as_str())
}

/// `formatCindyMakeTitle`.
fn make_title(title: &str, worktree: Option<&str>) -> String {
    let text = MAKE_TAG.replace(title, "");
    let Some(worktree) = worktree else {
        return text.into_owned();
    };
    let tag = format!("[{}]", &worktree[..worktree.len().min(4)]);
    let body = match text.strip_prefix(&tag) {
        Some(rest) => trim_start(rest),
        None => &text,
    };
    if body.is_empty() {
        tag
    } else {
        format!("{tag} {body}")
    }
}

/// `formatCindyMakeMergeTitle`: the local creation time, `MM-DD HH:mm`, before the title.
fn merge_title(title: &str, created_at_ms: i64) -> String {
    match jiff::Timestamp::from_millisecond(created_at_ms) {
        Ok(at) => {
            let local = at.to_zoned(jiff::tz::TimeZone::system());
            format!("{} {title}", local.strftime("%m-%d %H:%M"))
        }
        Err(_) => title.to_owned(),
    }
}

/// `getLegacyMakeTitleKey`: an old default merge title in any locale it was made in.
fn legacy_make_key(s: &Session, worktree: Option<&str>) -> Option<&'static str> {
    if s.source != MAKE_MERGE {
        return None;
    }
    let title = make_title(&s.title, worktree);
    MAKE_MERGE_TITLE_KEYS.into_iter().find(|key| {
        title == make_title(key, worktree)
            || cross_locale(key)
                .iter()
                .any(|label| title == make_title(label, worktree))
    })
}

/// `getSessionDisplayTitle`, with `SessionItem`'s unnamed label.
pub fn display(s: &Session, t: &Translator) -> String {
    if s.title == DRAFT {
        let key = if s.source == MAKE {
            "cindyMake.code.taskName"
        } else {
            "ccAgent.common.unnamedSession"
        };
        return t.t(key, &[]);
    }
    if s.source == MAKE || s.source == MAKE_MERGE {
        let worktree = worktree_name(s.working_dir.as_deref());
        let title = match legacy_make_key(s, worktree) {
            Some(key) => merge_title(&t.t(key, &[]), s.created_at),
            None => s.title.clone(),
        };
        return make_title(&title, worktree);
    }
    s.title
        .strip_prefix(SCHEDULE_PREFIX)
        .unwrap_or(&s.title)
        .to_owned()
}

#[cfg(test)]
mod tests;
