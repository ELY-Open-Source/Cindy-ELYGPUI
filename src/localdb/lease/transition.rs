//! `withLeaseTransition`: one lease change at a time per database, through a renamed folder.

use std::{fs, io, path::Path};

use super::{Failure, alive, hex, uuid};
use crate::localdb::boot;

/// `owner-<pid>-<uuid>[-boot-<sha256>]`: the pid and the boot tag, when it has one.
fn parse_marker(name: &str) -> Option<(u64, Option<&str>)> {
    let rest = name.strip_prefix("owner-")?;
    let (pid, rest) = rest.split_once('-')?;
    let valid_pid = !pid.starts_with('0') && pid.bytes().all(|b| b.is_ascii_digit());
    if !valid_pid || pid.is_empty() || rest.len() < 36 {
        return None;
    }
    let (id, tail) = rest.split_at(36);
    let hex_or_dash = |s: &str| {
        s.bytes()
            .all(|b| b == b'-' || b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    if !hex_or_dash(id) {
        return None;
    }
    let tag = match tail.strip_prefix("-boot-") {
        None if tail.is_empty() => None,
        Some(tag) if tag.len() == 64 && hex_or_dash(tag) && !tag.contains('-') => Some(tag),
        _ => return None,
    };
    Some((pid.parse().ok()?, tag))
}

/// Clears a gate left by a dead owner; `Some` when it cannot or must not.
fn clear_stale(gate: &Path, boot_tag: Option<&str>) -> io::Result<Option<Failure>> {
    match fs::symlink_metadata(gate) {
        Ok(meta) if !meta.is_dir() => return Ok(Some(Failure::LockUnreadable)),
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    }
    let entries = match fs::read_dir(gate) {
        Ok(entries) => entries
            .map(|e| e.map(|e| e.file_name()))
            .collect::<io::Result<Vec<_>>>()?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if let Some(entry) = entries.first() {
        let name = entry.to_string_lossy();
        let owner = (entries.len() == 1).then(|| parse_marker(&name)).flatten();
        let Some((pid, tag)) = owner else {
            return Ok(Some(Failure::LockUnreadable));
        };
        let previous_boot = boot_tag.is_some_and(|now| tag.is_some_and(|t| t != now));
        if !previous_boot && alive(pid) {
            return Ok(Some(Failure::RecoveryBusy));
        }
        match fs::remove_file(gate.join(entry)) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
    }
    match fs::remove_dir(gate) {
        Err(error)
            if !matches!(
                error.kind(),
                io::ErrorKind::NotFound
                    | io::ErrorKind::DirectoryNotEmpty
                    | io::ErrorKind::AlreadyExists
            ) =>
        {
            Err(error)
        }
        _ => Ok(None),
    }
}

/// Runs `task` holding `<db>.schema-transition`; any IO error is `lock-unreadable`.
pub(super) fn with_gate<T>(
    db: &Path,
    task: impl FnOnce() -> io::Result<Result<T, Failure>>,
) -> Result<T, Failure> {
    let gate = std::path::PathBuf::from(format!("{}.schema-transition", db.display()));
    let boot_tag = boot::identity().map(hex);
    let tag = boot_tag
        .as_deref()
        .map_or(String::new(), |t| format!("-boot-{t}"));
    let marker = format!("owner-{}-{}{tag}", std::process::id(), uuid());
    let candidate = std::path::PathBuf::from(format!("{}.{marker}.candidate", gate.display()));
    let mut held = false;
    let mut task = Some(task);
    let outcome = (|| -> io::Result<Result<T, Failure>> {
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        builder.create(&candidate)?;
        super::create_private(&candidate.join(&marker))?;
        for _ in 0..3 {
            if fs::rename(&candidate, &gate).is_ok() {
                held = true;
                return task.take().expect("runs once")();
            }
            if let Some(failure) = clear_stale(&gate, boot_tag.as_deref())? {
                return Ok(Err(failure));
            }
        }
        Ok(Err(Failure::RecoveryBusy))
    })();
    let directory = if held { &gate } else { &candidate };
    let _ = fs::remove_file(directory.join(&marker));
    let _ = fs::remove_dir(directory);
    outcome.unwrap_or(Err(Failure::LockUnreadable))
}
