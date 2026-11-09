//! `gitIntent` and `githubCliIntent`: Git and GitHub CLI forms with no hidden effects.

use super::{
    Intent,
    shell::{positionals, subcommand},
};
use crate::theme_import::color::js_trim;

const GIT_GLOBAL_VALUE_FLAGS: [&str; 7] = [
    "-C",
    "-c",
    "--git-dir",
    "--work-tree",
    "--namespace",
    "--exec-path",
    "--config-env",
];

const BRANCH_MUTATING: [&str; 15] = [
    "-d",
    "-D",
    "-m",
    "-M",
    "-c",
    "-C",
    "-f",
    "-u",
    "--delete",
    "--move",
    "--copy",
    "--force",
    "--edit-description",
    "--set-upstream-to",
    "--unset-upstream",
];

const BRANCH_VALUE_FLAGS: [&str; 7] = [
    "--contains",
    "--no-contains",
    "--merged",
    "--no-merged",
    "--points-at",
    "--sort",
    "--format",
];

const GH_GLOBAL_VALUE_FLAGS: [&str; 3] = ["-R", "--repo", "--hostname"];

const GH_API_VALUE_FLAGS: [&str; 15] = [
    "-H",
    "--header",
    "--hostname",
    "--input",
    "-q",
    "--jq",
    "-X",
    "--method",
    "-f",
    "--raw-field",
    "-F",
    "--field",
    "-t",
    "--template",
    "--cache",
];

fn any(args: &[String], f: impl Fn(&str) -> bool) -> bool {
    args.iter().any(|t| f(t))
}

/// `token === flag || token.startsWith(`${flag}=`)`.
fn flag_or_value(token: &str, flag: &str) -> bool {
    token
        .strip_prefix(flag)
        .is_some_and(|r| r.is_empty() || r.starts_with('='))
}

fn branch_is_read_only(args: &[String]) -> bool {
    let mutating = any(args, |t| {
        BRANCH_MUTATING.iter().any(|f| flag_or_value(t, f))
            || (t.starts_with('-')
                && t.chars().nth(1).is_some_and(|c| c != '-')
                && t[1..].contains(['d', 'D', 'm', 'M', 'c', 'C', 'f', 'u']))
    });
    if mutating {
        return false;
    }
    args.iter().any(|t| t == "--list") || positionals(args, &BRANCH_VALUE_FLAGS).is_empty()
}

fn when(ok: bool, action: &'static str) -> Option<Intent> {
    ok.then(|| Intent::new(action))
}

pub fn git(rest: &[String]) -> Option<Intent> {
    let (name, args) = subcommand(rest, &GIT_GLOBAL_VALUE_FLAGS)?;
    let output = any(args, |t| flag_or_value(t, "--output"));
    let ok = |ok: bool, action| when(ok, action);
    match name.as_str() {
        "status" => ok(true, "gitStatus"),
        "diff" => ok(!output, "gitDiff"),
        "log" => ok(!output, "gitLog"),
        "show" => ok(!output, "gitShow"),
        "add" => ok(true, "gitAdd"),
        "commit" => ok(!any(args, |t| t == "--amend"), "gitCommit"),
        "fetch" => ok(true, "gitFetch"),
        "pull" => ok(true, "gitPull"),
        "push" => {
            let destructive = any(args, |t| {
                matches!(
                    t,
                    "-f" | "--force" | "-d" | "--delete" | "--mirror" | "--prune"
                ) || t.starts_with("--force=")
                    || t.starts_with("--force-with-lease")
            });
            ok(!destructive, "gitPush")
        }
        "rebase" => ok(!rebase_runs_shell(args), "gitRebase"),
        "merge" => ok(true, "gitMerge"),
        "cherry-pick" => ok(true, "gitCherryPick"),
        "stash" => ok(true, "gitStash"),
        "restore" => ok(true, "gitRestore"),
        "submodule" => {
            let foreach = subcommand(args, &[]).is_some_and(|(n, _)| n == "foreach");
            ok(!foreach, "gitSubmodule")
        }
        "remote" => {
            let op = subcommand(args, &[]);
            let read = op.is_none_or(|(n, _)| n == "show" || n == "get-url");
            ok(read, "gitRemote")
        }
        "rev-parse" => ok(true, "gitRevParse"),
        "branch" => ok(branch_is_read_only(args), "gitBranch"),
        "grep" => {
            let pager = any(args, |t| {
                t.starts_with("-O") || t.starts_with("--open-files-in-pager")
            });
            ok(!pager, "gitGrep")
        }
        "merge-base" => ok(true, "gitMergeBase"),
        "ls-files" => ok(true, "gitLsFiles"),
        "rev-list" => ok(true, "gitRevList"),
        "ls-remote" => {
            let upload = any(args, |t| t == "-u" || t.starts_with("--upload-pack"));
            ok(!upload, "gitLsRemote")
        }
        "worktree" => match subcommand(args, &[])?.0.as_str() {
            "list" => ok(true, "gitWorktreeList"),
            "add" => ok(true, "gitWorktreeAdd"),
            "remove" => ok(true, "gitWorktreeRemove"),
            "move" => ok(true, "gitWorktreeMove"),
            "prune" => ok(true, "gitWorktreePrune"),
            _ => None,
        },
        _ => None,
    }
}

/// `gitRebaseInvokesShell`: `--exec` or `-x` before any `--`.
fn rebase_runs_shell(args: &[String]) -> bool {
    for token in args {
        if token == "--" {
            return false;
        }
        if flag_or_value(token, "--exec") || token.starts_with("-x") {
            return true;
        }
    }
    false
}

fn gh_action(group: &str, operation: &str) -> Option<&'static str> {
    let action = match (group, operation) {
        ("pr", "list") => "ghPrList",
        ("pr", "view") => "ghPrView",
        ("pr", "checks") => "ghPrChecks",
        ("pr", "status") => "ghPrStatus",
        ("pr", "diff") => "ghPrDiff",
        ("pr", "create") => "ghPrCreate",
        ("pr", "edit") => "ghPrEdit",
        ("pr", "comment") => "ghPrComment",
        ("pr", "review") => "ghPrReview",
        ("pr", "merge") => "ghPrMerge",
        ("pr", "close") => "ghPrClose",
        ("pr", "reopen") => "ghPrReopen",
        ("pr", "checkout") => "ghPrCheckout",
        ("issue", "list") => "ghIssueList",
        ("issue", "view") => "ghIssueView",
        ("issue", "status") => "ghIssueStatus",
        ("issue", "create") => "ghIssueCreate",
        ("issue", "edit") => "ghIssueEdit",
        ("issue", "comment") => "ghIssueComment",
        ("issue", "close") => "ghIssueClose",
        ("issue", "reopen") => "ghIssueReopen",
        ("auth", "status") => "ghAuthStatus",
        ("auth", "login") => "ghAuthLogin",
        ("auth", "logout") => "ghAuthLogout",
        ("auth", "refresh") => "ghAuthRefresh",
        ("auth", "switch") => "ghAuthSwitch",
        ("run", "list") => "ghRunList",
        ("run", "view") => "ghRunView",
        ("run", "watch") => "ghRunWatch",
        ("search", "code" | "commits" | "issues" | "prs" | "repos") => "ghSearch",
        ("repo", "list") => "ghRepoList",
        _ => return None,
    };
    Some(action)
}

pub fn github(rest: &[String]) -> Option<Intent> {
    let (group, args) = subcommand(rest, &GH_GLOBAL_VALUE_FLAGS)?;
    if group == "api" {
        return Some(api(args));
    }
    let (operation, op_args) = subcommand(args, &GH_GLOBAL_VALUE_FLAGS)?;
    if operation.is_empty() {
        return None;
    }
    if group == "repo" && operation == "view" {
        let target = positionals(op_args, &GH_GLOBAL_VALUE_FLAGS)
            .first()
            .copied();
        return Some(Intent::new("ghRepoView").target(target));
    }
    gh_action(&group, &operation).map(Intent::new)
}

/// `readCliOptionValue`.
fn option_value<'a>(tokens: &'a [String], short: &str, long: &str) -> Option<&'a str> {
    for (index, token) in tokens.iter().enumerate() {
        if token == short || token == long {
            return tokens.get(index + 1).map(String::as_str);
        }
        if let Some(value) = token.strip_prefix(long).and_then(|r| r.strip_prefix('=')) {
            return Some(value);
        }
        if let Some(value) = token.strip_prefix(short).filter(|v| !v.is_empty()) {
            return Some(value);
        }
    }
    None
}

/// `collectGithubApiFields`.
fn api_fields(tokens: &[String]) -> Vec<&str> {
    let mut fields = Vec::new();
    let mut index = 0;
    while let Some(token) = tokens.get(index) {
        index += 1;
        if matches!(token.as_str(), "-f" | "-F" | "--raw-field" | "--field") {
            fields.extend(tokens.get(index).map(String::as_str));
            index += 1;
            continue;
        }
        let inline = ["--raw-field=", "--field=", "-f", "-F"]
            .iter()
            .find_map(|flag| token.strip_prefix(flag).filter(|v| !v.is_empty()));
        fields.extend(inline);
    }
    fields
}

/// `/^word\b/i` with ASCII word characters.
fn starts_word(text: &str, word: &str) -> bool {
    text.get(..word.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(word))
        && !text[word.len()..]
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// `githubApiIntent`.
fn api(args: &[String]) -> Intent {
    let method = option_value(args, "-X", "--method").map(str::to_uppercase);
    let input = any(args, |t| flag_or_value(t, "--input"));
    let fields = api_fields(args);
    let endpoint = positionals(args, &GH_API_VALUE_FLAGS)
        .first()
        .map(|e| e.to_lowercase());
    let graphql = endpoint.as_deref() == Some("graphql");
    let call = Intent::new("ghApiCall");
    if input && (method.is_none() || graphql) {
        return call;
    }
    if graphql {
        let query = fields.iter().find_map(|f| f.strip_prefix("query="));
        let operation = query.map(js_trim).filter(|o| !o.is_empty());
        if let Some(op) = operation {
            if starts_word(op, "mutation") {
                return Intent::new("ghApiMutation");
            }
            if starts_word(op, "query") || op.starts_with('{') {
                return Intent::new("ghApiQuery");
            }
        }
        if method.is_none() {
            return call;
        }
    }
    match method.as_deref() {
        Some("GET" | "HEAD") => Intent::new("ghApiQuery"),
        Some("POST" | "PUT" | "PATCH" | "DELETE") => Intent::new("ghApiMutation"),
        Some(_) => call,
        None if endpoint.is_none_or(|e| e.is_empty()) => call,
        None if !fields.is_empty() => Intent::new("ghApiMutation"),
        None => Intent::new("ghApiQuery"),
    }
}
