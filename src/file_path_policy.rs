//! `filePathPolicy.ts`'s system blocklist and `isPathAllowedAgainst`, on this host's paths.

use std::{path::Path, sync::LazyLock};

use crate::dev_profile::canonical::resolve;

/// `buildWin32SystemBlocklist`: the env's roots, C-drive defaults, one per spelling.
pub fn win32_blocklist(env: impl Fn(&str) -> Option<String>) -> Vec<String> {
    let candidates = [
        ("SystemRoot", "C:\\Windows"),
        ("ProgramFiles", "C:\\Program Files"),
        ("ProgramFiles(x86)", "C:\\Program Files (x86)"),
        ("ProgramData", "C:\\ProgramData"),
    ]
    .map(|(key, default)| env(key).unwrap_or_else(|| default.to_owned()));
    let mut out: Vec<String> = Vec::new();
    for c in candidates {
        let seen = out.iter().any(|o| o.to_lowercase() == c.to_lowercase());
        if !c.is_empty() && !seen {
            out.push(c);
        }
    }
    out
}

/// `buildSystemPathBlocklist`.
pub fn system_blocklist(windows: bool, env: impl Fn(&str) -> Option<String>) -> Vec<String> {
    match windows {
        true => win32_blocklist(env),
        false => ["/etc", "/proc", "/sys", "/dev", "/root", "/var/log"]
            .map(str::to_owned)
            .into(),
    }
}

/// `SYSTEM_PATH_BLOCKLIST`, built once as the main process builds it.
static SYSTEM: LazyLock<Vec<String>> = LazyLock::new(|| {
    let list = system_blocklist(cfg!(windows), |key| std::env::var(key).ok());
    log::info!(target: "file-path-policy", "system blocklist: {list:?}");
    list
});

/// `stripWinNamespace`: `\\?\` and `\\.\` prefixes, and `\\?\UNC\` back to `\\`.
pub fn strip_namespace(input: &str) -> String {
    let Some(rest) = input
        .strip_prefix("\\\\?\\")
        .or_else(|| input.strip_prefix("\\\\.\\"))
    else {
        return input.to_owned();
    };
    match rest.get(..4) {
        Some(head) if head.eq_ignore_ascii_case("UNC\\") => format!("\\\\{}", &rest[4..]),
        _ => rest.to_owned(),
    }
}

/// Node's `isAbsolute` for this host.
pub fn is_absolute(path: &str) -> bool {
    match cfg!(windows) {
        true => crate::node_path::win32::is_absolute(path),
        false => path.starts_with('/'),
    }
}

/// The resolved key: lowercased on Windows, without a trailing separator.
fn key(path: &str) -> String {
    let resolved = resolve(Path::new(path)).to_string_lossy().into_owned();
    let folded = match cfg!(windows) {
        true => resolved.to_lowercase(),
        false => resolved,
    };
    let sep = std::path::MAIN_SEPARATOR;
    match folded.len() > 1 && folded.ends_with(sep) {
        true => folded[..folded.len() - 1].to_owned(),
        false => folded,
    }
}

/// `isPathAllowedAgainst`: absolute, and neither a root on the list nor inside one.
pub fn allowed_against(path: &str, blocklist: &[String]) -> bool {
    let input = match cfg!(windows) {
        true => strip_namespace(path),
        false => path.to_owned(),
    };
    if input.is_empty() || !is_absolute(&input) {
        return false;
    }
    let target = key(&input);
    let sep = std::path::MAIN_SEPARATOR;
    blocklist.iter().all(|blocked| {
        let blocked = match cfg!(windows) {
            true => strip_namespace(blocked),
            false => blocked.clone(),
        };
        let root = key(&blocked);
        target != root && !target.starts_with(&format!("{root}{sep}"))
    })
}

/// `isPathAllowed`.
pub fn allowed(path: &str) -> bool {
    allowed_against(path, &SYSTEM)
}

#[cfg(test)]
mod tests;
