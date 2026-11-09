//! `commandIntentFromCompositeCommand`: a chain summed up only when every part is known.

use super::{
    Intent, MAX_CHARS, analyze, clean_cd, is_cd, shell::binary_name, shell::split_top_level,
    single, substitution,
};
use crate::theme_import::color::js_trim;

const CONTENT: [&str; 5] = ["read", "list", "search", "parseJson", "count"];

const REPOSITORY: [&str; 29] = [
    "gitStatus",
    "gitDiff",
    "gitLog",
    "gitShow",
    "gitRemote",
    "gitRevParse",
    "gitBranch",
    "gitGrep",
    "gitMergeBase",
    "gitLsFiles",
    "gitRevList",
    "gitLsRemote",
    "gitWorktreeList",
    "ghPrList",
    "ghPrView",
    "ghPrChecks",
    "ghPrStatus",
    "ghPrDiff",
    "ghIssueList",
    "ghIssueView",
    "ghIssueStatus",
    "ghAuthStatus",
    "ghRunList",
    "ghRunView",
    "ghRunWatch",
    "ghSearch",
    "ghRepoList",
    "ghRepoView",
    "ghApiQuery",
];

const MUTATION: [&str; 15] = [
    "gitAdd",
    "gitCommit",
    "gitFetch",
    "gitPull",
    "gitPush",
    "gitRebase",
    "gitMerge",
    "gitCherryPick",
    "gitStash",
    "gitRestore",
    "gitSubmodule",
    "gitWorktreeAdd",
    "gitWorktreeRemove",
    "gitWorktreeMove",
    "gitWorktreePrune",
];

const ENVIRONMENT: [&str; 4] = [
    "inspectEnvironment",
    "showVersion",
    "locateCommand",
    "ghAuthStatus",
];

const VERIFICATION: [&str; 6] = [
    "test",
    "build",
    "lint",
    "typecheck",
    "checkSyntax",
    "checkFormatting",
];

/// `hasTopLevelBackgroundOperator`: a lone `&` outside quotes that is no redirect.
fn background(input: &str) -> bool {
    let chars: Vec<char> = input.chars().collect();
    let at = |i: Option<usize>| i.and_then(|i| chars.get(i).copied());
    let mut quote: Option<char> = None;
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        let (before, after) = (at(index.checked_sub(1)), at(Some(index + 1)));
        index += 1;
        if let Some(q) = quote {
            if ch == q && (q != '"' || before != Some('\\')) {
                quote = None;
            }
            continue;
        }
        if ch == '\\' && after.is_some() {
            index += 1;
            continue;
        }
        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            continue;
        }
        let near = |c: char| before == Some(c) || after == Some(c);
        if ch == '&' && !near('&') && !near('>') {
            return true;
        }
    }
    false
}

/// `isCompositePresentationSegment`: `true`, `echo` or `printf` without `-v`.
fn presentation(segment: &str) -> bool {
    let Some(argv) = analyze(segment) else {
        return false;
    };
    match binary_name(&argv[0]).as_str() {
        "true" | "echo" => true,
        "printf" => !argv[1..].iter().any(|t| t.starts_with("-v")),
        _ => false,
    }
}

pub fn composite(command: &str) -> Option<Intent> {
    let trimmed = js_trim(command);
    if trimmed.is_empty() || trimmed.encode_utf16().count() > MAX_CHARS {
        return None;
    }
    if substitution(trimmed) || background(trimmed) {
        return None;
    }
    let mut chain = split_top_level(trimmed, &["&&", "||", ";", "\n"]).filter(|c| c.len() >= 2)?;
    while chain.len() > 1 && is_cd(&chain[0]) {
        if !clean_cd(&chain[0]) {
            return None;
        }
        chain.remove(0);
    }
    if chain.len() < 2 {
        return None;
    }
    let mut resolved = Vec::new();
    for segment in chain.iter().filter(|s| !presentation(s)) {
        resolved.push(single::intent(segment)?);
    }
    let first = resolved.first()?.clone();
    let meaningful: Vec<Intent> = if resolved.len() > 1 {
        resolved
            .into_iter()
            .filter(|i| i.action != "showCurrentDirectory")
            .collect()
    } else {
        resolved
    };
    match meaningful.as_slice() {
        [] => return Some(first),
        [only] => return Some(only.clone()),
        _ => {}
    }
    let all = |set: &[&str]| meaningful.iter().all(|i| set.contains(&i.action));
    let mutates = meaningful.iter().any(|i| MUTATION.contains(&i.action));
    let repository = meaningful
        .iter()
        .all(|i| REPOSITORY.contains(&i.action) || MUTATION.contains(&i.action));
    if mutates && repository {
        return Some(Intent::new("modifyRepository"));
    }
    let groups: [(&[&str], &'static str); 4] = [
        (&CONTENT, "inspect"),
        (&REPOSITORY, "inspectRepository"),
        (&ENVIRONMENT, "inspectEnvironment"),
        (&VERIFICATION, "verify"),
    ];
    let (_, summary) = groups.into_iter().find(|(set, _)| all(set))?;
    let lead = &meaningful[0];
    if meaningful.iter().all(|i| i == lead) {
        return Some(lead.clone());
    }
    if meaningful.iter().all(|i| i.action == "search") {
        return Some(Intent::new("search"));
    }
    Some(Intent::new(summary))
}
