//! `lib/fileLocation.ts` over `shared/workdirPath.ts`: a file's place as the task's folder names it.

#[cfg(test)]
mod tests;

use crate::{local_path::windows_like, markdown_target::Place};

/// `dropDotSegments`.
fn drop_dots(path: &str) -> String {
    let root = if path.starts_with('/') { "/" } else { "" };
    let kept: Vec<&str> = path
        .split('/')
        .filter(|s| !matches!(*s, "." | ""))
        .collect();
    format!("{root}{}", kept.join("/"))
}

/// `toWorkdirRelPosix`.
fn rel_posix(workdir: &str, path: &str) -> Option<String> {
    if !workdir.starts_with('/') || !path.starts_with('/') || path.split('/').any(|s| s == "..") {
        return None;
    }
    let rest = path.strip_prefix(workdir.trim_end_matches('/'))?;
    let rel = rest.strip_prefix('/')?;
    (!rel.is_empty()).then(|| rel.to_owned())
}

/// `stripWindowsLongPathPrefix`.
fn strip_long(path: &str) -> String {
    match (
        path.strip_prefix("\\\\?\\UNC\\"),
        path.strip_prefix("\\\\?\\"),
    ) {
        (Some(rest), _) => format!("\\\\{rest}"),
        (None, Some(rest)) => rest.to_owned(),
        _ => path.to_owned(),
    }
}

/// `normalizeWindowsPath`: forward slashes, no `.` segment, a UNC path's two slashes kept.
fn fold_windows(path: &str) -> String {
    let slashed = strip_long(path).replace('\\', "/");
    let root = if slashed.starts_with("//") { "//" } else { "" };
    let kept: Vec<&str> = slashed
        .split('/')
        .filter(|s| !matches!(*s, "." | ""))
        .collect();
    format!("{root}{}", kept.join("/"))
}

/// `toWorkdirRel`: the path under the folder, with POSIX separators.
pub fn rel(workdir: &str, path: &str) -> Option<String> {
    if workdir.is_empty() || path.is_empty() {
        return None;
    }
    if !windows_like(workdir) {
        return rel_posix(workdir, &drop_dots(path));
    }
    if !windows_like(path) {
        return None;
    }
    let (folder, file) = (fold_windows(workdir), fold_windows(path));
    let under = format!("{}/", folder.to_lowercase());
    if file.split('/').any(|s| s == "..") || !file.to_lowercase().starts_with(&under) {
        return None;
    }
    // `slice` counts the folder as written, in UTF-16 units.
    let units: Vec<u16> = file.encode_utf16().collect();
    let rel = String::from_utf16_lossy(units.get(folder.encode_utf16().count() + 1..)?);
    (!rel.is_empty()).then_some(rel)
}

/// `formatFileLocation`: `rel:line[:column]`; a `Place` holds safe positive integers alone.
pub fn format(workdir: &str, path: &str, at: Place) -> Option<String> {
    let line = at.line?;
    let rel = rel(workdir, path)?;
    let column = at.column.map(|column| format!(":{column}"));
    Some(format!("{rel}:{line}{}", column.unwrap_or_default()))
}
