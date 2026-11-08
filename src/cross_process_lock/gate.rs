//! The reclaim gate: `{lock}.reclaim.d/gate-{pid}-{nonce}.json`, which a takeover or a release
//! holds while it moves the lock file, and the legacy `{lock}.reclaim`.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
    thread,
    time::{Duration, Instant},
};

use regex::Regex;

use super::{
    Heartbeat, RETRY, STALE_MS,
    fsops::{
        Identity, append, exists, missing, now_ms, remove_empty_dir_with_retry, remove_with_retry,
        restore_moved,
    },
    identity,
    record::{self, Full, Read},
};

/// Own gate files that outlived their release, to delete on the next acquisition.
static PENDING_CLEANUP: LazyLock<Mutex<HashSet<PathBuf>>> = LazyLock::new(Mutex::default);

static GATE_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^gate-(\d+)-[0-9a-f-]+\.json$").expect("valid pattern"));

/// `AdvisoryReclaimGate`.
pub struct Gate {
    dir: PathBuf,
    file: PathBuf,
    nonce: String,
    heartbeat: Option<Heartbeat>,
}

fn cleanup() -> std::sync::MutexGuard<'static, HashSet<PathBuf>> {
    PENDING_CLEANUP
        .lock()
        .expect("the cleanup set never poisons")
}

fn dir_of(lock: &Path) -> PathBuf {
    append(lock, ".reclaim.d")
}

/// `cleanupPendingOwnGates`.
fn cleanup_pending_own_gates() {
    let files: Vec<PathBuf> = cleanup().iter().cloned().collect();
    for file in files {
        remove_with_retry(&file);
        if !exists(&file) {
            remove_with_retry(&record::marker_path(&file));
            cleanup().remove(&file);
        }
    }
}

/// `ordinaryReclaimInProgress`: a legacy gate or a live gate file.
pub fn reclaim_in_progress(lock: &Path) -> bool {
    cleanup_pending_own_gates();
    legacy_in_progress(lock) || find_active(&dir_of(lock)).is_some()
}

/// `legacyReclaimInProgress`: a stale `{lock}.reclaim` is moved aside and dropped.
fn legacy_in_progress(lock: &Path) -> bool {
    let gate = append(lock, ".reclaim");
    match std::fs::metadata(&gate) {
        Err(error) if missing(&error) => return false,
        _ => {}
    }
    let Some(expected) = inspect_stale(&gate) else {
        return true;
    };
    let moved = append(
        &gate,
        &format!("-stale-{}-{}", std::process::id(), uuid::Uuid::new_v4()),
    );
    if std::fs::rename(&gate, &moved).is_err() {
        return true;
    }
    let matches =
        |path: &Path| matches!(record::read_full(path, false), Read::Record(r) if r == expected);
    let result = if matches(&moved) {
        log::info!(target: "cross-process-lock", "dropped a stale legacy reclaim gate at {}", gate.display());
        false
    } else {
        restore_moved(&moved, &gate);
        true
    };
    if exists(&moved) && matches(&moved) {
        remove_with_retry(&moved);
    }
    result
}

/// `inspectStaleLock` for the legacy gate: a released record, or a stale one whose owner is gone.
fn inspect_stale(path: &Path) -> Option<Full> {
    let identity = Identity::stat(path).ok()?;
    let Read::Record(record) = record::read_full(path, false) else {
        return None;
    };
    if record.released {
        return Some(record);
    }
    if now_ms() - identity.mtime_ms <= STALE_MS {
        return None;
    }
    (!identity::owner_active(&record)).then_some(record)
}

/// `findActiveReclaimGate`: the oldest live gate file, or the directory when it cannot be read.
fn find_active(dir: &Path) -> Option<PathBuf> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if missing(&error) => return None,
        Err(error) => {
            log::warn!(target: "cross-process-lock", "reclaim gates unreadable: {error}");
            return Some(dir.to_owned());
        }
    };
    let mut active: Vec<(PathBuf, Full)> = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                log::warn!(target: "cross-process-lock", "reclaim gates unreadable: {error}");
                return Some(dir.to_owned());
            }
        };
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.starts_with("gate-") || !name.ends_with(".json") {
            continue;
        }
        let file = dir.join(name);
        let Ok(identity) = Identity::stat(&file) else {
            continue;
        };
        let read = record::read_full(&file, true);
        let expected = match &read {
            Read::Missing => continue,
            Read::Record(r) => r.nonce.as_deref(),
            _ => None,
        };
        if record::valid_marker(&file, expected) {
            remove_with_retry(&file);
            if !exists(&file) {
                remove_with_retry(&record::marker_path(&file));
            }
            continue;
        }
        let fresh = now_ms() - identity.mtime_ms <= STALE_MS;
        let record = match read {
            Read::Record(record) => record,
            _ => {
                if fresh || malformed_owner_active(name) {
                    return Some(file);
                }
                remove_with_retry(&file);
                if exists(&file) {
                    return Some(file);
                }
                continue;
            }
        };
        if record.released {
            remove_with_retry(&file);
            continue;
        }
        if fresh || identity::owner_active(&record) {
            active.push((file, record));
        } else {
            remove_with_retry(&file);
        }
    }
    active.sort_by(|(lp, l), (rp, r)| {
        l.started_at
            .total_cmp(&r.started_at)
            .then_with(|| l.nonce.cmp(&r.nonce))
            .then_with(|| lp.cmp(rp))
    });
    active.into_iter().next().map(|(file, _)| file)
}

/// `isMalformedGateOwnerActive`: the pid in the file name decides.
fn malformed_owner_active(name: &str) -> bool {
    let Some(pid) = GATE_NAME
        .captures(name)
        .and_then(|c| c[1].parse::<i64>().ok())
        .filter(|pid| *pid > 0)
    else {
        return true;
    };
    identity::owner_active(&Full {
        pid,
        started_at: 0.0,
        nonce: None,
        identity: None,
        released: false,
    })
}

/// `acquireAdvisoryReclaimGate`: a gate file of ours that wins `find_active`, within `wait`.
pub fn acquire(lock: &Path, wait: Duration) -> Option<Gate> {
    cleanup_pending_own_gates();
    let deadline = Instant::now() + wait;
    let dir = dir_of(lock);
    loop {
        if legacy_in_progress(lock) {
            if Instant::now() >= deadline {
                return None;
            }
            thread::sleep(RETRY);
            continue;
        }
        if let Err(error) = std::fs::create_dir_all(&dir) {
            log::warn!(target: "cross-process-lock", "reclaim gate directory failed: {error}");
            return None;
        }
        let nonce = uuid::Uuid::new_v4().to_string();
        let file = dir.join(format!("gate-{}-{nonce}.json", std::process::id()));
        if let Err(error) = write_gate(&file, &nonce) {
            log::warn!(target: "cross-process-lock", "reclaim gate publication failed: {error}");
            remove_with_retry(&file);
            return None;
        }
        let gate = Gate {
            dir: dir.clone(),
            heartbeat: Some(Heartbeat::start(file.clone())),
            file,
            nonce,
        };
        loop {
            let winner = find_active(&dir);
            if winner.as_deref() == Some(gate.file.as_path()) {
                return Some(gate);
            }
            if winner.is_none() || Instant::now() >= deadline {
                release(gate);
                break;
            }
            thread::sleep(RETRY);
        }
        if Instant::now() >= deadline {
            return None;
        }
    }
}

/// The gate's full `LockRecord`, so strict contenders read it too.
fn write_gate(file: &Path, nonce: &str) -> std::io::Result<()> {
    use std::io::Write as _;
    let mut out = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(file)?;
    let text = format!(
        r#"{{"pid":{},"startedAt":{},"nonce":"{nonce}","processStartIdentity":"{}","state":"held"}}"#,
        std::process::id(),
        now_ms() as i64,
        identity::estimate_self()
    );
    out.write_all(text.as_bytes())
}

/// `releaseAdvisoryReclaimGate`: our file goes; one that stays gets a release marker.
pub fn release(mut gate: Gate) {
    drop(gate.heartbeat.take());
    let ours = matches!(
        record::read_full(&gate.file, true),
        Read::Record(r) if r.nonce.as_deref() == Some(gate.nonce.as_str())
    );
    if ours {
        remove_with_retry(&gate.file);
        if exists(&gate.file)
            && let Err(error) = record::publish_marker(&gate.file, &gate.nonce)
        {
            log::warn!(target: "cross-process-lock", "advisory reclaim gate release marker could not be published: {error}");
        }
    }
    if exists(&gate.file) {
        cleanup().insert(gate.file.clone());
    } else {
        remove_with_retry(&record::marker_path(&gate.file));
    }
    remove_empty_dir_with_retry(&gate.dir);
}
