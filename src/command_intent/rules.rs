//! The per-tool rules of `commandIntent.ts`: curl, interpreters, jq, wc, date, lsof, sqlite.

use super::{
    Intent, base_or,
    shell::{positionals, subcommand},
};
use crate::theme_import::color::{js_space, js_trim};

/// `CURL_MUTATING_LONG_FLAGS`, matched as prefixes.
const CURL_LONG: [&str; 17] = [
    "--output",
    "--remote-name",
    "--request",
    "--data",
    "--form",
    "--upload-file",
    "--json",
    "--dump-header",
    "--cookie-jar",
    "--trace",
    "--stderr",
    "--config",
    "--etag-save",
    "--libcurl",
    "--alt-svc",
    "--hsts",
    "--write-out",
];

/// `CURL_MUTATING_SHORT_CHARS`.
const CURL_SHORT: &str = "oOXdFTDcKw";

const NODE_VALUE_FLAGS: [&str; 7] = [
    "-r",
    "--require",
    "--import",
    "--loader",
    "--experimental-loader",
    "--conditions",
    "-C",
];

const JQ_VALUE_FLAGS: [&str; 4] = ["-L", "--indent", "-f", "--from-file"];
const JQ_TWO_VALUE_FLAGS: [&str; 5] = [
    "--arg",
    "--argjson",
    "--slurpfile",
    "--rawfile",
    "--argfile",
];

const SQLITE_MUTATING: [&str; 13] = [
    "insert",
    "update",
    "delete",
    "replace",
    "create",
    "drop",
    "alter",
    "vacuum",
    "attach",
    "detach",
    "reindex",
    "load_extension",
    "writefile",
];

const SQLITE_READ_ONLY_DOTS: [&str; 9] = [
    ".tables",
    ".schema",
    ".indexes",
    ".databases",
    ".dbinfo",
    ".show",
    ".headers",
    ".mode",
    ".width",
];

fn has(rest: &[String], token: &str) -> bool {
    rest.iter().any(|t| t == token)
}

/// `isVersionRequest`.
pub fn version_request(rest: &[String]) -> bool {
    matches!(rest, [only] if matches!(only.as_str(), "-v" | "--version" | "-V"))
}

fn version(target: &str) -> Option<Intent> {
    Some(Intent::new("showVersion").target(Some(target)))
}

/// A script argument, `-` (stdin) refused.
fn script_arg(script: Option<&str>) -> Option<&str> {
    script.filter(|s| !s.is_empty() && *s != "-")
}

pub fn curl(rest: &[String]) -> Option<Intent> {
    let mutating = rest.iter().any(|token| {
        CURL_LONG.iter().any(|flag| token.starts_with(flag))
            || (token.starts_with('-')
                && !token.starts_with("--")
                && token[1..].chars().any(|c| CURL_SHORT.contains(c)))
    });
    if mutating {
        return None;
    }
    let url = rest
        .iter()
        .find(|t| t.starts_with("http://") || t.starts_with("https://"))?;
    Some(Intent::new("fetch").target(Some(url)))
}

pub fn node(rest: &[String]) -> Option<Intent> {
    if version_request(rest) {
        return version("Node.js");
    }
    if rest
        .iter()
        .any(|t| matches!(t.as_str(), "-e" | "--eval" | "-p" | "--print"))
    {
        return Some(Intent::new("runNodeScript"));
    }
    let check = rest.iter().any(|t| t == "-c" || t == "--check");
    let files = positionals(rest, &NODE_VALUE_FLAGS);
    let script = script_arg(files.first().copied())?;
    let action = if check { "checkSyntax" } else { "runScript" };
    Some(Intent::new(action).target(Some(base_or(script))))
}

pub fn python(rest: &[String]) -> Option<Intent> {
    if version_request(rest) {
        return version("Python");
    }
    if rest.iter().any(|t| t == "-c" || t == "-m") {
        return Some(Intent::new("runPythonScript"));
    }
    let script = script_arg(positionals(rest, &["-W", "-X"]).first().copied())?;
    Some(Intent::new("runScript").target(Some(base_or(script))))
}

/// `/^-[^\s]*[eE]/`.
fn perl_eval(token: &str) -> bool {
    token.strip_prefix('-').is_some_and(|flags| {
        flags
            .chars()
            .take_while(|&c| !js_space(c))
            .any(|c| c == 'e' || c == 'E')
    })
}

pub fn perl(rest: &[String]) -> Option<Intent> {
    if version_request(rest) {
        return version("Perl");
    }
    let flags = ["-I", "-M", "-m"];
    if has(rest, "-c") {
        let script = positionals(rest, &flags).first().copied();
        let script = script.filter(|s| !s.is_empty())?;
        return Some(Intent::new("checkSyntax").target(Some(base_or(script))));
    }
    if rest.iter().any(|t| perl_eval(t)) {
        return Some(Intent::new("runPerlScript"));
    }
    let script = script_arg(positionals(rest, &flags).first().copied())?;
    Some(Intent::new("runScript").target(Some(base_or(script))))
}

fn swift_source(rest: &[String]) -> Option<&str> {
    rest.iter()
        .find(|t| t.to_lowercase().ends_with(".swift"))
        .map(String::as_str)
        .filter(|s| !s.is_empty())
}

pub fn swift_compiler(rest: &[String]) -> Option<Intent> {
    if version_request(rest) {
        return version("Swift");
    }
    let target = base_or(swift_source(rest)?);
    let action = if has(rest, "-typecheck") {
        "typecheck"
    } else if has(rest, "-parse") {
        "checkSyntax"
    } else {
        "build"
    };
    Some(Intent::new(action).target(Some(target)))
}

pub fn swift(rest: &[String]) -> Option<Intent> {
    if version_request(rest) {
        return version("Swift");
    }
    if has(rest, "-e") {
        return Some(Intent::new("runSwiftScript"));
    }
    let script = swift_source(rest)?;
    Some(Intent::new("runScript").target(Some(base_or(script))))
}

pub fn xcrun(rest: &[String]) -> Option<Intent> {
    let (name, args) = subcommand(rest, &["--sdk", "--toolchain"])?;
    match name.as_str() {
        "swiftc" => swift_compiler(args),
        "swift" => swift(args),
        "simctl" if args.first().map(String::as_str) == Some("list") => {
            Some(Intent::new("inspectEnvironment"))
        }
        _ => None,
    }
}

pub fn plutil(rest: &[String]) -> Option<Intent> {
    if has(rest, "-p") {
        return Some(Intent::new("inspect"));
    }
    has(rest, "-lint").then(|| Intent::new("checkSyntax"))
}

pub fn jq(rest: &[String]) -> Option<Intent> {
    if version_request(rest) {
        return version("jq");
    }
    if has(rest, "-h") || has(rest, "--help") {
        return None;
    }
    if rest
        .iter()
        .any(|t| JQ_TWO_VALUE_FLAGS.contains(&t.as_str()))
    {
        return Some(Intent::new("parseJson"));
    }
    let files = positionals(rest, &JQ_VALUE_FLAGS);
    let from_file = has(rest, "-f") || has(rest, "--from-file");
    let input = files.get(if from_file { 0 } else { 1 }).copied();
    Some(Intent::new("parseJson").target(input.map(base_or)))
}

pub fn count(rest: &[String]) -> Option<Intent> {
    if has(rest, "--help") {
        return None;
    }
    if version_request(rest) {
        return version("wc");
    }
    let target = positionals(rest, &["--files0-from"]).first().copied();
    Some(Intent::new("count").target(target.map(base_or)))
}

pub fn date(rest: &[String]) -> Option<Intent> {
    let display = ["-u", "--utc", "--universal", "-R", "--rfc-email"];
    rest.iter()
        .all(|t| display.contains(&t.as_str()) || t.starts_with('+'))
        .then(|| Intent::new("showDateTime"))
}

pub fn locate(rest: &[String]) -> Option<Intent> {
    let [flag, names @ ..] = rest else {
        return None;
    };
    if names.is_empty() || (flag != "-v" && flag != "-V") {
        return None;
    }
    let targets: Vec<&str> = names
        .iter()
        .map(String::as_str)
        .filter(|t| !t.is_empty() && !t.starts_with('-'))
        .collect();
    targets.first()?;
    Some(Intent::new("locateCommand").target(Some(&targets.join(" "))))
}

pub fn lsof(rest: &[String]) -> Intent {
    for (index, token) in rest.iter().enumerate() {
        if token == "-i" {
            let target = rest.get(index + 1).map(String::as_str);
            return Intent::new("inspectPorts").target(target.filter(|t| !t.starts_with('-')));
        }
        if let Some(target) = token.strip_prefix("-i").filter(|t| !t.is_empty()) {
            return Intent::new("inspectPorts").target(Some(target));
        }
    }
    Intent::new("inspectProcesses")
}

/// `SQLITE_MUTATING_SQL`: one of the words between ASCII word boundaries, any case.
fn mutating_sql(sql: &str) -> bool {
    let lower = sql.to_ascii_lowercase();
    let word = |b: Option<&u8>| b.is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_');
    let bytes = lower.as_bytes();
    SQLITE_MUTATING.iter().any(|needle| {
        lower.match_indices(needle).any(|(at, _)| {
            let end = at + needle.len();
            !word(at.checked_sub(1).and_then(|i| bytes.get(i))) && !word(bytes.get(end))
        })
    })
}

/// `/[?&]mode=ro(?:&|$)/i`.
fn read_only_uri(database: &str) -> bool {
    let lower = database.to_ascii_lowercase();
    ["?mode=ro", "&mode=ro"].iter().any(|needle| {
        lower
            .match_indices(needle)
            .any(|(at, _)| matches!(lower[at + needle.len()..].chars().next(), None | Some('&')))
    })
}

pub fn sqlite(rest: &[String]) -> Option<Intent> {
    let unsafe_option = |t: &String| {
        ["-cmd", "-init"]
            .iter()
            .any(|f| t == f || t.strip_prefix(f).is_some_and(|r| r.starts_with('=')))
    };
    if rest.iter().any(unsafe_option) {
        return None;
    }
    let pos = positionals(rest, &["-separator", "-newline", "-vfs"]);
    let (database, statements) = pos.split_first()?;
    if database.is_empty() || (!has(rest, "-readonly") && !read_only_uri(database)) {
        return None;
    }
    let dot_write = statements.iter().any(|statement| {
        let trimmed = js_trim(statement);
        let head = trimmed.split(js_space).next().unwrap_or("");
        trimmed.starts_with('.') && !SQLITE_READ_ONLY_DOTS.contains(&head)
    });
    if dot_write || mutating_sql(&statements.join(" ")) {
        return None;
    }
    Some(Intent::new("queryDatabase").target(Some(base_or(database))))
}
