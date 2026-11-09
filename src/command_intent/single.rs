//! `commandIntentFromSingleCommand`: the rule table by the first segment's binary.

use super::{
    Intent, analyze, base_or, git, rules,
    shell::{binary_name, positionals},
    tools,
};

/// `FIND_DESTRUCTIVE_FLAGS`: find actions that run, delete or write.
pub const FIND_DESTRUCTIVE: [&str; 9] = [
    "-exec", "-execdir", "-ok", "-okdir", "-delete", "-fprint", "-fprint0", "-fprintf", "-fls",
];

/// `NL_VALUE_FLAGS`.
const NL_VALUE_FLAGS: [&str; 20] = [
    "-b",
    "-d",
    "-f",
    "-h",
    "-i",
    "-l",
    "-n",
    "-s",
    "-v",
    "-w",
    "--body-numbering",
    "--section-delimiter",
    "--footer-numbering",
    "--header-numbering",
    "--line-increment",
    "--join-blank-lines",
    "--number-format",
    "--number-separator",
    "--starting-line-number",
    "--number-width",
];

/// `/^python(?:\d+(?:\.\d+)?)?$/`.
fn is_python(bin: &str) -> bool {
    let Some(version) = bin.strip_prefix("python") else {
        return false;
    };
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    match version.split_once('.') {
        _ if version.is_empty() => true,
        Some((major, minor)) => digits(major) && digits(minor),
        None => digits(version),
    }
}

/// A read of `file`, named by its basename.
fn read(file: Option<&str>) -> Option<Intent> {
    let file = file.filter(|f| !f.is_empty())?;
    Some(
        Intent::new("read")
            .target(Some(base_or(file)))
            .path(Some(file)),
    )
}

/// tree's `-o FILE` and `-R` write; bundled short flags are refused too.
pub fn tree_writes(rest: &[String]) -> bool {
    rest.iter()
        .any(|t| t.starts_with('-') && (t.contains('o') || t.contains('R')))
}

pub fn intent(command: &str) -> Option<Intent> {
    let argv = analyze(command)?;
    let bin = binary_name(&argv[0]);
    let rest = &argv[1..];
    if is_python(&bin) {
        return rules::python(rest);
    }
    match bin.as_str() {
        "cat" => read(positionals(rest, &[]).first().copied()),
        "less" | "more" if rest.iter().any(|t| t.starts_with('+')) => None,
        "less" | "more" => read(positionals(rest, &[]).first().copied()),
        "head" | "tail" => read(positionals(rest, &["-n", "-c"]).first().copied()),
        "nl" => read(positionals(rest, &NL_VALUE_FLAGS).first().copied()),
        "sed" if !tools::sed_is_read_only(rest) => None,
        "sed" => read(positionals(rest, &["-e"]).get(1).copied()),
        "tree" if tree_writes(rest) => None,
        "ls" | "tree" => {
            let flags: &[&str] = if bin == "tree" {
                &["-L", "--filelimit"]
            } else {
                &[]
            };
            Some(Intent::new("list").target(positionals(rest, flags).first().copied()))
        }
        "grep" | "egrep" | "fgrep" | "rg" | "ag" => tools::grep(rest),
        "find" => find(rest),
        "fd" => {
            if rest.iter().any(|t| tools::fd_exec_flag(t)) {
                return None;
            }
            let flags = ["-e", "-t", "-E", "--extension", "--type", "--exclude"];
            let pos = positionals(rest, &flags);
            let pattern = pos.first().filter(|p| !p.is_empty())?;
            Some(
                Intent::new("search")
                    .target(Some(pattern))
                    .path(pos.get(1).copied()),
            )
        }
        "curl" => rules::curl(rest),
        "node" => rules::node(rest),
        "perl" => rules::perl(rest),
        "swiftc" => rules::swift_compiler(rest),
        "swift" => rules::swift(rest),
        "xcrun" => rules::xcrun(rest),
        "jq" => rules::jq(rest),
        "wc" => rules::count(rest),
        "pwd" => rest
            .iter()
            .all(|t| ["-L", "-P", "--logical", "--physical"].contains(&t.as_str()))
            .then(|| Intent::new("showCurrentDirectory")),
        "date" => rules::date(rest),
        "command" => rules::locate(rest),
        "which" => {
            let targets = positionals(rest, &[]);
            targets.first().filter(|t| !t.is_empty())?;
            Some(Intent::new("locateCommand").target(Some(&targets.join(" "))))
        }
        "ps" | "pgrep" => Some(Intent::new("inspectProcesses")),
        "lsof" => Some(rules::lsof(rest)),
        "sqlite3" => rules::sqlite(rest),
        "test" => Some(Intent::new("inspectEnvironment")),
        "[" => (rest.last().map(String::as_str) == Some("]"))
            .then(|| Intent::new("inspectEnvironment")),
        "stat" | "du" => Some(Intent::new("inspect")),
        "file" => {
            (!rest.iter().any(|t| t == "-C" || t == "--compile")).then(|| Intent::new("inspect"))
        }
        "plutil" => rules::plutil(rest),
        "defaults" | "tailscale" => {
            let wanted = if bin == "defaults" { "read" } else { "status" };
            (rest.first().map(String::as_str) == Some(wanted))
                .then(|| Intent::new("inspectEnvironment"))
        }
        "git" if rules::version_request(rest) => {
            Some(Intent::new("showVersion").target(Some("Git")))
        }
        "git" => git::git(rest),
        "gh" if rules::version_request(rest) => {
            Some(Intent::new("showVersion").target(Some("GitHub CLI")))
        }
        "gh" => git::github(rest),
        "pnpm" | "npm" | "yarn" | "bun" => tools::package_manager(&bin, rest),
        "npx" | "pnpx" | "bunx" => {
            let tool = positionals(rest, &[]).first().copied()?;
            tools::tool_binary(Some(tool), tools::args_after(rest, tool))
        }
        "make" => {
            let goals = positionals(rest, &["-C", "-j", "-f"]);
            match goals.as_slice() {
                [] => Some(Intent::new("build")),
                [goal] => tools::script(goal),
                _ => None,
            }
        }
        "cargo" => tools::cargo(
            rest.first().map(String::as_str),
            rest.get(1..).unwrap_or(&[]),
        ),
        "go" => go(rest),
        _ => tools::tool_binary(Some(&bin), rest),
    }
}

fn find(rest: &[String]) -> Option<Intent> {
    if rest.iter().any(|t| FIND_DESTRUCTIVE.contains(&t.as_str())) {
        return None;
    }
    let name = rest.iter().position(|t| t == "-name" || t == "-iname");
    let pattern = name.and_then(|i| rest.get(i + 1)).map(String::as_str);
    let path = rest
        .first()
        .filter(|p| !p.is_empty() && !p.starts_with('-'))
        .map(String::as_str);
    match pattern.filter(|p| !p.is_empty()) {
        None => Some(Intent::new("list").target(path)),
        Some(pattern) => Some(Intent::new("search").target(Some(pattern)).path(path)),
    }
}

fn go(rest: &[String]) -> Option<Intent> {
    match rest.first().map(String::as_str) {
        Some("test") => {
            let builds = rest
                .iter()
                .any(|t| t == "-c" || t == "-o" || t.starts_with("-o="));
            (!builds).then(|| Intent::new("test"))
        }
        Some("build") => Some(Intent::new("build")),
        Some("vet") => Some(Intent::new("lint")),
        _ => None,
    }
}
