//! `defaultCanonicalizePath`: one spelling per directory, as the file system sees it.

use std::path::{Path, PathBuf};

/// The volume a directory lives on.
#[cfg(unix)]
fn volume(dir: &Path) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(dir).ok().map(|m| m.dev())
}

/// The volume serial number, through a handle on the directory itself.
#[cfg(windows)]
fn volume(dir: &Path) -> Option<u64> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS;
    let mut options = std::fs::OpenOptions::new();
    options
        .read(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0);
    let file = options.open(dir).ok()?;
    crate::bounded_read::identity(&file)
        .ok()
        .map(|[serial, _]| serial as u64)
}

fn flip(name: &str) -> String {
    let flip = |c: char| {
        if c.is_ascii_lowercase() {
            c.to_ascii_uppercase()
        } else {
            c.to_ascii_lowercase()
        }
    };
    name.chars().map(flip).collect()
}

/// `volumeIsCaseInsensitive`: the first name with letters, flipped, still names this directory.
fn case_insensitive(existing: &Path) -> bool {
    let mut dir = existing.to_owned();
    loop {
        let parent = dir.parent().map(Path::to_owned);
        if let Some(parent) = &parent
            && volume(parent) != volume(&dir)
        {
            return false;
        }
        let name = dir.file_name().map(|n| n.to_string_lossy().into_owned());
        if let (Some(parent), Some(name)) = (&parent, name) {
            let flipped = flip(&name);
            if flipped != name {
                let real = std::fs::canonicalize(parent.join(flipped));
                return real.ok().as_deref() == std::fs::canonicalize(&dir).ok().as_deref();
            }
        }
        match parent {
            Some(parent) => dir = parent,
            None => return false,
        }
    }
}

fn spelled(real: PathBuf) -> PathBuf {
    if case_insensitive(&real) {
        PathBuf::from(real.to_string_lossy().to_lowercase())
    } else {
        real
    }
}

/// Node's `path.resolve`: absolute, with `.` and `..` folded away by name.
pub fn resolve(path: &Path) -> PathBuf {
    use std::path::Component;
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_owned());
    let mut out = PathBuf::new();
    for part in absolute.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// The real path, or, for one not yet created, its nearest real ancestor plus the rest.
pub fn path(path: &Path) -> PathBuf {
    let resolved = resolve(path);
    if let Ok(real) = std::fs::canonicalize(&resolved) {
        return spelled(real);
    }
    let mut rest = Vec::new();
    let mut dir = resolved.as_path();
    while let Some(parent) = dir.parent() {
        rest.push(dir.file_name().expect("a child has a name").to_owned());
        dir = parent;
        if let Ok(real) = std::fs::canonicalize(dir) {
            let joined = rest.iter().rev().fold(real.clone(), |p, n| p.join(n));
            return if case_insensitive(&real) {
                PathBuf::from(joined.to_string_lossy().to_lowercase())
            } else {
                joined
            };
        }
    }
    resolved
}
