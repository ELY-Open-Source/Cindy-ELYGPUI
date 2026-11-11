//! `localPathResolver.ts`'s joins: a tool's path made absolute against the task's folder.

pub mod shape;

use std::borrow::Cow;

use crate::cindy_json::decode_uri_component;

/// `WIN_ABS_RE`: a drive letter, a colon and a separator.
pub fn drive(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'/' | b'\\')
}

/// `isWindowsPathLike`.
fn windows_like(path: &str) -> bool {
    drive(path) || path.starts_with("\\\\") || path.starts_with("//")
}

/// `stripWindowsDriveUriSlash`: `/D:/x` is `D:/x`, a bare `/D:` the drive's root.
fn strip_drive_slash(value: &str) -> Cow<'_, str> {
    let Some(rest) = value.strip_prefix('/') else {
        return value.into();
    };
    let b = rest.as_bytes();
    match b.len() == 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        true => format!("{rest}/").into(),
        false if drive(rest) => rest.into(),
        false => value.into(),
    }
}

/// `basename` of `utils.ts`: the last segment, or the path when that is empty.
pub fn basename(path: &str) -> &str {
    match path.rsplit(['/', '\\']).next() {
        Some(last) if !last.is_empty() => last,
        _ => path,
    }
}

/// `resolveLocalPath`.
pub fn resolve(href: &str, cwd: &str) -> String {
    if let Some(rest) = href.strip_prefix("file://") {
        let decoded = decode_uri_component(rest).unwrap_or_else(|| rest.to_owned());
        return strip_drive_slash(&decoded).into_owned();
    }
    let href = match windows_like(cwd) {
        true => strip_drive_slash(href),
        false => href.into(),
    };
    if href.starts_with('/') || drive(&href) || href.starts_with("\\\\") {
        return href.into_owned();
    }
    let cwd_trimmed = cwd.trim_end_matches(['/', '\\']);
    match cwd.contains('\\') {
        true => format!("{cwd_trimmed}\\{}", href.replace('/', "\\")),
        false => format!("{cwd_trimmed}/{href}"),
    }
}

/// `resolveToolFilePath`: a relative path joins the folder; with no folder it stays.
pub fn resolve_tool(raw: &str, working_dir: &str) -> String {
    if raw.is_empty() || working_dir.is_empty() || raw.starts_with('/') {
        return raw.to_owned();
    }
    resolve(raw, working_dir)
}

#[cfg(test)]
mod tests {
    use super::{resolve, resolve_tool};
    use crate::{
        generated_files::tests::{text, yields},
        rich_answer::js::Js,
    };

    fn both(args: &[Js]) -> (&str, &str) {
        (args[0].str().unwrap(), args[1].str().unwrap())
    }

    #[test]
    fn joins_match_cindys() {
        yields("resolveLocalPath", |args| {
            let (href, cwd) = both(args);
            text(&resolve(href, cwd))
        });
        yields("resolveToolFilePath", |args| {
            let (raw, dir) = both(args);
            text(&resolve_tool(raw, dir))
        });
    }
}
