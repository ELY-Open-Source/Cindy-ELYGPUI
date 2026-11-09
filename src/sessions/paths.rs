//! `workingDir.ts`, `worktreePaths.ts` and `projectKeys.ts`: one spelling per project folder.

/// `process.platform` where it matters: Windows compares drive and UNC folders without case.
pub const PLATFORM: &str = if cfg!(windows) {
    "win32"
} else {
    std::env::consts::OS
};

/// `isWindowsPathLike`.
fn windows_like(p: &str) -> bool {
    drive_slash(p) || p.starts_with("\\\\") || p.starts_with("//")
}

/// `^[A-Za-z]:[\\/]`.
fn drive_slash(p: &str) -> bool {
    let b = p.as_bytes();
    b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'/' || b[2] == b'\\')
}

/// `stripWindowsLongPathPrefix`.
fn strip_long_prefix(p: &str) -> String {
    if let Some(rest) = p.strip_prefix("\\\\?\\UNC\\") {
        return format!("\\\\{rest}");
    }
    p.strip_prefix("\\\\?\\").unwrap_or(p).to_owned()
}

/// `normalizeWorkingDirForStorage`: no long-path prefix, `/` on Windows paths, no trailing `/`.
pub fn storage(raw: Option<&str>) -> Option<String> {
    let path = raw?;
    if path.trim().is_empty() {
        return None;
    }
    let stripped = strip_long_prefix(path);
    let mut out = if windows_like(path) || windows_like(&stripped) {
        stripped.replace('\\', "/")
    } else {
        stripped
    };
    while out.len() > 1 && out.ends_with('/') {
        let b = out.as_bytes();
        if b.len() == 3 && b[0].is_ascii_alphabetic() && b[1] == b':' {
            break;
        }
        out.pop();
    }
    Some(out)
}

const MANAGED: [&[&str]; 2] = [&[".cindy-worktrees"], &[".xdt-worktrees"]];
const GROUPING: [&[&str]; 4] = [
    &[".cindy-worktrees"],
    &[".xdt-worktrees"],
    &[".worktrees"],
    &[".claude", "worktrees"],
];

/// `resolveWorktreeBase`: the folder above a worktree container that has one more segment.
fn worktree_base(value: &str, containers: &[&[&str]], insensitive: bool) -> Option<String> {
    let backslash = drive_slash(value) || value.starts_with("\\\\");
    let ignore_case = insensitive && (backslash || value.starts_with("//"));
    let separator = |c: char| c == '/' || (backslash && c == '\\');
    let mut segments = Vec::new();
    let mut start = None;
    for (i, c) in value.char_indices().chain([(value.len(), '/')]) {
        if !separator(c) {
            start.get_or_insert(i);
        } else if let Some(s) = start.take() {
            segments.push((s, &value[s..i]));
        }
    }
    for i in 0..segments.len() {
        for container in containers {
            if i + container.len() >= segments.len() {
                continue;
            }
            let matched = container.iter().enumerate().all(|(k, want)| {
                let text = segments[i + k].1;
                if ignore_case {
                    text.to_lowercase() == *want
                } else {
                    text == *want
                }
            });
            let marker = segments[i].0;
            if !matched || marker == 0 {
                continue;
            }
            let base = value[..marker].trim_end_matches(&separator);
            if base.is_empty() {
                return Some(value[..1].to_owned());
            }
            let b = base.as_bytes();
            if b.len() == 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
                return Some(format!("{base}{}", &value[marker - 1..marker]));
            }
            return Some(base.to_owned());
        }
    }
    None
}

/// `normalizeWorkingDirForGrouping`: storage form, managed worktrees, then any worktree folder.
pub fn grouping(raw: Option<&str>) -> Option<String> {
    let out = storage(raw)?;
    let out = worktree_base(&out, &MANAGED, false).unwrap_or(out);
    Some(worktree_base(&out, &GROUPING, true).unwrap_or(out))
}

/// `normalizeWorkingDirForComparison`: the grouping form, Windows folders lowered.
pub fn comparison(raw: Option<&str>) -> Option<String> {
    let out = grouping(raw)?;
    Some(if windows_like(&out) {
        out.to_lowercase()
    } else {
        out
    })
}

/// `encodeURIComponent`.
pub fn encode_component(text: &str) -> String {
    let keep = |b: u8| b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b);
    text.bytes()
        .map(|b| {
            if keep(b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// `projectIdentityKey`.
pub fn identity_key(dir: &str, remote_host: Option<&str>) -> String {
    let normalized = grouping(Some(dir));
    let Some(dir) = normalized.as_deref() else {
        return format!("local:{dir}");
    };
    match remote_host.filter(|h| !h.is_empty()) {
        Some(host) => format!("remote:{}:{dir}", encode_component(host)),
        None => format!("local:{dir}"),
    }
}

/// `projectKeyComparisonKey` for an already formed key: Windows drive and UNC folders compare
/// without case on Windows.
pub fn comparison_key(key: &str, platform: &str) -> String {
    let Some(dir) = key.strip_prefix("local:") else {
        return key.to_owned();
    };
    let b = dir.as_bytes();
    let drive = b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'/';
    if platform == "win32" && (drive || dir.starts_with("//")) {
        format!("local:{}", dir.to_lowercase())
    } else {
        key.to_owned()
    }
}

/// `normalizeProjectKey` for the stored keys this chapter reads (aliases, hidden projects).
pub fn normalize_key(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(dir) = trimmed.strip_prefix("local:") {
        return grouping(Some(dir)).map(|d| format!("local:{d}"));
    }
    if let Some(rest) = trimmed.strip_prefix("remote:") {
        let (host, dir) = rest.split_once(':')?;
        let host = percent_decode(host)?;
        let dir = grouping(Some(dir))?;
        return (!host.is_empty()).then(|| identity_key(&dir, Some(&host)));
    }
    if let Some(rest) = trimmed.strip_prefix("device:") {
        let (device, dir) = rest.split_once(':')?;
        let device = percent_decode(device)?;
        let dir = grouping(Some(dir))?;
        return (!device.is_empty()).then(|| format!("device:{}:{dir}", encode_component(&device)));
    }
    grouping(Some(trimmed)).map(|d| format!("local:{d}"))
}

/// `decodeURIComponent`, failing as it does on a broken escape.
fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = text.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// `projectKeyComparisonKey` of a stored key: normalized first.
pub fn compare(raw: &str, platform: &str) -> Option<String> {
    normalize_key(raw).map(|key| comparison_key(&key, platform))
}

/// `projectIdentityKeyForSession`, for a task's folder and host.
pub fn session_key(dir: Option<&str>, remote_host: Option<&str>) -> Option<String> {
    grouping(dir).map(|dir| identity_key(&dir, remote_host))
}
