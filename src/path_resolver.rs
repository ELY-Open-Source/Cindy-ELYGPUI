//! `main/pathResolver.ts`: a path a reply names, found in or about the task's folder.

pub mod cache;
#[cfg(test)]
mod tests;
mod walk;

use std::collections::{HashMap, HashSet};

use walk::{find, find_many};

use crate::{
    cindy_json::decode_uri_component,
    file_path_policy::is_absolute,
    local_path::{shape::scheme_slashes, windows_like},
    node_path::{join, win32},
    open_path::strip_drive_uri_slash,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    File,
    Directory,
}

/// `ResolvePathResult`: `multiple` holds files alone.
#[derive(Clone, Debug, PartialEq)]
pub enum Resolved {
    None,
    Unique(String, Kind),
    Multiple(Vec<String>),
}

/// `ResolveOptions`, less the ignored folders no caller sets.
pub struct Options<'a> {
    pub allowed: &'a dyn Fn(&str) -> bool,
    pub max_candidates: usize,
    pub max_depth: usize,
    pub home: &'a str,
}

/// `safeDecodeHrefForLookup`: the escapes read once, unless they add path or URL syntax.
pub fn decode(value: &str) -> Option<String> {
    let decoded = decode_uri_component(value).unwrap_or_else(|| value.to_owned());
    let count = |text: &str, c: char| text.matches(c).count();
    let moved = ['/', '\\']
        .iter()
        .any(|c| count(&decoded, *c) != count(value, *c));
    if (moved && !win32::is_absolute(&decoded)) || scheme_slashes(&decoded) {
        return None;
    }
    let (before, after) = (segments(value), segments(&decoded));
    let climbs =
        |(at, segment): (usize, &&str)| **segment == *".." && before.get(at) != Some(&"..");
    (!after.iter().enumerate().any(climbs)).then_some(decoded)
}

/// `split(/[\\/]+/)`.
fn segments(value: &str) -> Vec<&str> {
    let separator = |c: char| c == '/' || c == '\\';
    let (mut out, mut rest) = (Vec::new(), value);
    while let Some(at) = rest.find(separator) {
        out.push(&rest[..at]);
        rest = rest[at..].trim_start_matches(separator);
    }
    out.push(rest);
    out
}

/// `expandTilde`: `~`, `~/x` and `~\x` alone.
fn expand(path: &str, home: &str) -> String {
    let rest = path.strip_prefix("~/").or(path.strip_prefix("~\\"));
    match (path, rest) {
        ("~", _) => home.to_owned(),
        (_, Some(rest)) => join(&[home, rest]),
        _ => path.to_owned(),
    }
}

/// A file or folder at `path` that the caller may read.
fn stat(path: &str, options: &Options) -> Option<Resolved> {
    let found = std::fs::metadata(path).ok()?;
    let kind = match (found.is_file(), found.is_dir()) {
        (true, _) => Kind::File,
        (_, true) => Kind::Directory,
        _ => return None,
    };
    (options.allowed)(path).then(|| Resolved::Unique(path.to_owned(), kind))
}

/// What the cheap paths settle: `Err` holds the decoded href the walk still owes.
fn direct(href: &str, dir: &str, options: &Options) -> Result<Resolved, String> {
    if href.is_empty() || href.contains('\n') || scheme_slashes(href) {
        return Ok(Resolved::None);
    }
    let Some(decoded) = decode(href) else {
        return Ok(Resolved::None);
    };
    let mut probe = expand(&decoded, options.home);
    if windows_like(dir) {
        probe = strip_drive_uri_slash(&probe);
    }
    if is_absolute(&probe) {
        return Ok(stat(&probe, options).unwrap_or(Resolved::None));
    }
    stat(&join(&[dir, &decoded]), options).ok_or(decoded)
}

fn found(mut candidates: Vec<String>) -> Resolved {
    match candidates.len() {
        0 => Resolved::None,
        1 => Resolved::Unique(candidates.remove(0), Kind::File),
        _ => Resolved::Multiple(candidates),
    }
}

/// `resolveWorkspacePath`.
pub fn resolve(href: &str, dir: &str, options: &Options) -> Resolved {
    if dir.is_empty() || !is_absolute(dir) {
        return Resolved::None;
    }
    match direct(href, dir, options) {
        Ok(settled) => settled,
        Err(decoded) => found(find(dir, &decoded, options)),
    }
}

/// `resolveWorkspacePathBatch`: one walk for every href the cheap paths leave.
pub fn resolve_all(hrefs: &[String], dir: &str, options: &Options) -> HashMap<String, Resolved> {
    let mut out = HashMap::new();
    if dir.is_empty() || !is_absolute(dir) {
        out.extend(hrefs.iter().map(|href| (href.clone(), Resolved::None)));
        return out;
    }
    let mut owed = Vec::new();
    for href in hrefs {
        if out.contains_key(href) || owed.iter().any(|(seen, _)| *seen == href) {
            continue;
        }
        match direct(href, dir, options) {
            Ok(settled) => _ = out.insert(href.clone(), settled),
            Err(decoded) => owed.push((href, decoded)),
        }
    }
    if owed.is_empty() {
        return out;
    }
    let mut seen = HashSet::new();
    let wanted = owed.iter().map(|(_, decoded)| decoded.as_str());
    let wanted: Vec<&str> = wanted.filter(|decoded| seen.insert(*decoded)).collect();
    let walked = find_many(dir, &wanted, options);
    log::debug!(target: "fs:resolve-path-batch", "walked for {} of {} paths", wanted.len(), hrefs.len());
    for (href, decoded) in &owed {
        out.insert((*href).clone(), found(walked[decoded.as_str()].clone()));
    }
    out
}
