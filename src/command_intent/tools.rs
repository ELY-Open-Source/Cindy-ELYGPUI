//! Search, package scripts, build tools and the gates on Codex's actions.

use super::{
    Intent, READ_BINS, analyze, rules,
    shell::{binary_name, positionals},
    single::{FIND_DESTRUCTIVE, tree_writes},
};
use crate::theme_import::color::js_space;

/// `GREP_VALUE_FLAGS` with `-e` and `--regexp`.
const GREP_VALUE_FLAGS: [&str; 18] = [
    "-A",
    "-B",
    "-C",
    "-m",
    "-d",
    "-g",
    "-t",
    "-T",
    "-f",
    "--type",
    "--glob",
    "--include",
    "--exclude",
    "--exclude-dir",
    "--max-count",
    "--context",
    "-e",
    "--regexp",
];

const PM_VALUE_FLAGS: [&str; 6] = ["--filter", "-F", "--dir", "-C", "--cwd", "--prefix"];

/// `PACKAGE_MANAGER_BUILTINS`.
const PM_BUILTINS: &[&str] = &[
    "access",
    "add",
    "allow-builds",
    "approve-builds",
    "audit",
    "bin",
    "bugs",
    "cache",
    "catalog",
    "ci",
    "completion",
    "config",
    "constraints",
    "create",
    "dedupe",
    "delete",
    "deploy",
    "deprecate",
    "diff",
    "dist-tag",
    "dlx",
    "doctor",
    "docs",
    "edit",
    "env",
    "exec",
    "explain",
    "explore",
    "fetch",
    "find-dupes",
    "fund",
    "get",
    "help",
    "help-search",
    "hook",
    "ignored-builds",
    "import",
    "info",
    "init",
    "install",
    "install-ci-test",
    "install-test",
    "i",
    "link",
    "list",
    "licenses",
    "login",
    "logout",
    "ls",
    "node",
    "npm",
    "org",
    "outdated",
    "owner",
    "pack",
    "patch",
    "patch-commit",
    "ping",
    "pkg",
    "plugin",
    "pm",
    "prefix",
    "profile",
    "prune",
    "publish",
    "query",
    "rebuild",
    "remove",
    "repo",
    "root",
    "run",
    "sbom",
    "search",
    "self-update",
    "server",
    "set",
    "setup",
    "shrinkwrap",
    "stage",
    "star",
    "stars",
    "store",
    "team",
    "token",
    "uninstall",
    "unlink",
    "unplug",
    "unpublish",
    "update",
    "up",
    "upgrade-interactive",
    "version",
    "view",
    "whoami",
    "why",
    "workspace",
    "workspaces",
    "x",
];

const KNOWN_TOOL_BINARIES: [&str; 7] = [
    "vitest", "jest", "pytest", "eslint", "tsc", "prettier", "node",
];

/// `/^-[a-zA-Z]/`.
fn short_flag(token: &str) -> bool {
    token
        .strip_prefix('-')
        .and_then(|r| r.chars().next())
        .is_some_and(|c| c.is_ascii_alphabetic())
}

/// `grepIntent`.
pub fn grep(rest: &[String]) -> Option<Intent> {
    if rest.iter().any(|t| t.starts_with("--pre")) {
        return None;
    }
    let pos = positionals(rest, &GREP_VALUE_FLAGS);
    if rest.iter().any(|t| t == "--files") {
        return Some(Intent::new("list").target(pos.first().copied()));
    }
    let from_file = rest.iter().any(|t| {
        t == "--file"
            || t.starts_with("--file=")
            || t.starts_with("-f")
            || (short_flag(t) && !t.starts_with("--") && !t.starts_with("-e") && t.contains('f'))
    });
    if from_file {
        return None;
    }
    let mut pattern: Option<&str> = None;
    for (index, token) in rest.iter().enumerate() {
        if token == "-e" || token == "--regexp" {
            pattern = rest.get(index + 1).map(String::as_str);
            break;
        }
        if let Some(value) = token.strip_prefix("--regexp=") {
            pattern = Some(value);
            break;
        }
        if let Some(value) = token.strip_prefix("-e").filter(|v| !v.is_empty()) {
            pattern = Some(value);
            break;
        }
    }
    let pattern = pattern.or(pos.first().copied()).filter(|p| !p.is_empty())?;
    let path = if pos.first() == Some(&pattern) {
        pos.get(1)
    } else {
        pos.first()
    };
    Some(
        Intent::new("search")
            .target(Some(pattern))
            .path(path.copied()),
    )
}

/// `hasMutatingForwardedArg`.
fn forwards_write(args: &[String]) -> bool {
    args.iter()
        .any(|t| t == "--fix" || t.starts_with("--fix=") || t == "-u" || t.starts_with("--update"))
}

/// `argsAfter`: the tokens after the first `anchor`.
pub fn args_after<'a>(rest: &'a [String], anchor: &str) -> &'a [String] {
    let at = rest.iter().position(|t| t == anchor).map_or(0, |i| i + 1);
    &rest[at..]
}

/// `packageManagerIntent`.
pub fn package_manager(manager: &str, rest: &[String]) -> Option<Intent> {
    if rules::version_request(rest) {
        return Some(Intent::new("showVersion").target(Some(manager)));
    }
    let pos = positionals(rest, &PM_VALUE_FLAGS);
    let sub = *pos.first().filter(|s| !s.is_empty())?;
    let packages = || pos[1..].join(" ");
    match sub {
        "install" | "i" | "ci" | "add" => Some(Intent::new("install").target(Some(&packages()))),
        "run" => {
            let script_name = pos.get(1).copied().filter(|s| !s.is_empty())?;
            if forwards_write(args_after(rest, script_name)) {
                return None;
            }
            script(script_name).or_else(|| Some(Intent::new("runScript").target(Some(script_name))))
        }
        "test" => (!forwards_write(args_after(rest, sub))).then(|| Intent::new("test")),
        "exec" | "dlx" => {
            let tool = pos.get(1).copied().filter(|s| !s.is_empty())?;
            tool_binary(Some(tool), args_after(rest, tool))
        }
        _ => {
            let forwarded = args_after(rest, sub);
            if let Some(script) = script(sub) {
                return (!forwards_write(forwarded)).then_some(script);
            }
            if let Some(tool) = tool_binary(Some(sub), forwarded) {
                return Some(tool);
            }
            let known = KNOWN_TOOL_BINARIES.contains(&binary_name(sub).as_str());
            (!known && !PM_BUILTINS.contains(&sub))
                .then(|| Intent::new("runScript").target(Some(sub)))
        }
    }
}

/// `scriptIntent`: the test, build, lint and typecheck script families.
pub fn script(name: &str) -> Option<Intent> {
    ["test", "build", "lint", "typecheck"]
        .into_iter()
        .find(|family| {
            name.strip_prefix(family)
                .is_some_and(|r| r.is_empty() || r.starts_with([':', '.', '-']))
        })
        .map(Intent::new)
}

/// `toolBinaryIntent`.
pub fn tool_binary(bin: Option<&str>, args: &[String]) -> Option<Intent> {
    let bin = bin.filter(|b| !b.is_empty())?;
    let has = |token: &str| args.iter().any(|t| t == token);
    match binary_name(bin).as_str() {
        "vitest" | "jest" | "pytest" => {
            let updates = args.iter().any(|t| t == "-u" || t.starts_with("--update"));
            (!updates).then(|| Intent::new("test"))
        }
        "eslint" => {
            let fixes = args.iter().any(|t| t == "--fix" || t.starts_with("--fix="));
            (!fixes).then(|| Intent::new("lint"))
        }
        "tsc" => {
            let at = args.iter().position(|t| t == "--noEmit")?;
            let value = args.get(at + 1).map_or(String::new(), |v| v.to_lowercase());
            (value != "false").then(|| Intent::new("typecheck"))
        }
        "prettier" => (has("--check") || has("-c")).then(|| Intent::new("checkFormatting")),
        "node" => rules::node(args),
        _ => None,
    }
}

/// `cargoIntent`.
pub fn cargo(sub: Option<&str>, args: &[String]) -> Option<Intent> {
    let has = |token: &str| args.iter().any(|t| t == token);
    match sub? {
        "test" => (!has("--no-run")).then(|| Intent::new("test")),
        "build" => Some(Intent::new("build")),
        "check" => Some(Intent::new("typecheck")),
        "clippy" => (!has("--fix")).then(|| Intent::new("lint")),
        "add" => Some(Intent::new("install")),
        _ => None,
    }
}

/// `SED_PRINT_ONLY_SCRIPT`: `/^[0-9,$;~\s]*p$/`.
fn print_only(script: &str) -> bool {
    script.strip_suffix('p').is_some_and(|head| {
        head.chars()
            .all(|c| c.is_ascii_digit() || ",$;~".contains(c) || js_space(c))
    })
}

/// `sedTokensAreReadOnly`: `-n`, no in-place edit or script file, one print-only script.
pub fn sed_is_read_only(rest: &[String]) -> bool {
    let has = |token: &str| rest.iter().any(|t| t == token);
    if !has("-n") {
        return false;
    }
    let edits = rest.iter().any(|t| {
        t.starts_with("-i")
            || t == "--in-place"
            || t.starts_with("--in-place=")
            || t.starts_with("-f")
            || t == "--file"
            || t.starts_with("--file=")
    });
    let expression = |t: &&String| *t == "-e" || *t == "--expression";
    if edits || rest.iter().filter(expression).count() > 1 {
        return false;
    }
    let script = match rest.iter().position(|t| expression(&t)) {
        Some(at) => rest.get(at + 1).map(String::as_str),
        None => positionals(rest, &[]).first().copied(),
    };
    script.is_some_and(print_only)
}

/// `sedPipelineFilterIsReadOnly`: a read-only sed that reads only its stdin.
pub fn sed_pipeline_filter_is_read_only(rest: &[String]) -> bool {
    if !sed_is_read_only(rest) {
        return false;
    }
    let flagged = rest.iter().any(|t| t == "-e" || t == "--expression");
    let count = positionals(rest, &["-e", "--expression"]).len();
    count == if flagged { 0 } else { 1 }
}

/// `isKnownReadCommand`: a read action's command must be a read-only reader.
pub fn is_known_read_command(command: Option<&str>) -> bool {
    let Some(argv) = command.and_then(analyze) else {
        return false;
    };
    let bin = binary_name(&argv[0]);
    let rest = &argv[1..];
    match bin.as_str() {
        _ if !READ_BINS.contains(&bin.as_str()) => false,
        "sed" => sed_is_read_only(rest),
        "less" | "more" => !rest.iter().any(|t| t.starts_with('+')),
        _ => true,
    }
}

/// `searchCommandHasSideEffects`: a search or list action's command that runs or writes.
pub fn search_has_side_effects(command: Option<&str>) -> bool {
    let Some(command) = command else {
        return false;
    };
    let Some(argv) = analyze(command) else {
        return true;
    };
    let rest = &argv[1..];
    match binary_name(&argv[0]).as_str() {
        "find" => rest.iter().any(|t| FIND_DESTRUCTIVE.contains(&t.as_str())),
        "fd" => rest.iter().any(|t| fd_exec_flag(t)),
        "rg" | "grep" | "egrep" | "fgrep" | "ag" => rest.iter().any(|t| t.starts_with("--pre")),
        "tree" => tree_writes(rest),
        _ => false,
    }
}

/// `hasFdExecFlag`: `--exec*`, or a short token with `x` or `X`.
pub fn fd_exec_flag(token: &str) -> bool {
    token.starts_with("--exec")
        || (short_flag(token) && !token.starts_with("--") && token.contains(['x', 'X']))
}
