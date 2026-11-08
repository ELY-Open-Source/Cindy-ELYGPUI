//! `clearInactiveRecord`: only under the transition gate, never by age.

use std::{fs, io::Write as _, path::Path};

use super::{Failure, Record, alive, create_private, read_record, sidecar, uuid};
use crate::localdb::boot;

#[derive(serde::Serialize, serde::Deserialize)]
struct Observation {
    boot: Option<String>,
    fingerprint: Option<String>,
}

/// An unknown owner may be a suspended old publisher: remember this exact file, and isolate
/// it only after a different kernel boot has seen the same file.
fn observe(path: &Path, boot: &str, fingerprint: &str) -> Failure {
    let candidate = format!("{}.{}.tmp", path.display(), uuid());
    let written = (|| {
        let mut file = create_private(Path::new(&candidate))?;
        let observation = Observation {
            boot: Some(boot.to_owned()),
            fingerprint: Some(fingerprint.to_owned()),
        };
        file.write_all(
            serde_json::to_string(&observation)
                .map_err(std::io::Error::other)?
                .as_bytes(),
        )?;
        file.sync_all()?;
        drop(file);
        fs::rename(&candidate, path)
    })();
    let _ = fs::remove_file(&candidate);
    if written.is_err() {
        Failure::LockUnreadable
    } else {
        Failure::LockCorrupt
    }
}

pub(super) fn clear_inactive(path: &Path) -> Option<Failure> {
    let (owner, fingerprint, identity, bytes) = match read_record(path) {
        Record::Missing => return None,
        Record::Unreadable => return Some(Failure::LockUnreadable),
        Record::Readable {
            owner,
            fingerprint,
            identity,
            bytes,
        } => (owner, fingerprint, identity, bytes),
    };
    let observation = sidecar(path, ".recovery-observation");
    match owner {
        Some(owner) if alive(owner.pid) => return Some(Failure::WriterActive),
        Some(_) => {}
        None => {
            let Some(boot) = boot::identity() else {
                return Some(Failure::LockCorrupt);
            };
            let previous = crate::bounded_read::read(&observation, 4096)
                .ok()
                .flatten()
                .and_then(|b| serde_json::from_slice::<Observation>(&b).ok());
            let proven = previous.is_some_and(|p| {
                p.boot.as_deref().is_some_and(|b| b != boot)
                    && p.fingerprint.as_deref() == Some(fingerprint.as_str())
            });
            if !proven {
                return Some(observe(&observation, boot, &fingerprint));
            }
        }
    }
    match read_record(path) {
        Record::Missing => return None,
        Record::Readable {
            fingerprint: now, ..
        } if now == fingerprint => {}
        _ => return Some(Failure::RecoveryBusy),
    }
    let backup = sidecar(path, &format!(".schema-lease-backup-{}", uuid()));
    if fs::rename(path, &backup).is_err() {
        return Some(Failure::LockUnreadable);
    }
    match read_record(&backup) {
        Record::Readable {
            identity: kept,
            bytes: same,
            ..
        } if kept == identity && same == bytes => {
            let _ = fs::remove_file(&observation);
            None
        }
        _ => {
            let _ = fs::hard_link(&backup, path);
            Some(Failure::RecoveryBusy)
        }
    }
}
