//! The advisory tier of Cindy's `device-link/crossProcessLock.ts`: a `{pid, startedAt, nonce}`
//! file beside the store, a heartbeat, nonce-bound release and stale takeover through a gate.

mod fsops;
mod gate;
mod identity;
mod record;
#[cfg(test)]
mod tests;

use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
    thread,
    time::{Duration, Instant},
};

pub use fsops::append as append_suffix;
use fsops::{Identity, append, now_ms, remove_with_retry, restore_moved, transient};
pub use identity::note_process_start;
use record::Advisory;

const STALE_MS: f64 = 10_000.0;
const RETRY: Duration = Duration::from_millis(40);
const MAX_TAKEOVERS: u32 = 3;
const HEARTBEAT: Duration = Duration::from_secs(2);
const RELEASE_GATE_WAIT: Duration = Duration::from_secs(3);

/// `LockStatus`: held, or why not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Held,
    Busy,
    Unavailable,
}

/// Owner records whose release failed, by lock path: the nonce to finish later.
static PENDING: LazyLock<Mutex<HashMap<PathBuf, String>>> = LazyLock::new(Mutex::default);

/// `withCrossProcessLock`: waits up to `wait`, then runs `task` with what it got.
pub fn with_lock<T>(lock: &Path, label: &str, wait: Duration, task: impl FnOnce(Status) -> T) -> T {
    let deadline = Instant::now() + wait;
    let mut status = Status::Unavailable;
    let mut takeovers = 0;
    let mut held = None;
    loop {
        recover_pending(lock);
        if gate::reclaim_in_progress(lock) {
            status = Status::Busy;
            if Instant::now() >= deadline {
                break;
            }
            thread::sleep(RETRY);
            continue;
        }
        match publish(lock) {
            Ok(nonce) => {
                if gate::reclaim_in_progress(lock) {
                    release(lock, label, &nonce);
                    status = Status::Busy;
                    if Instant::now() >= deadline {
                        break;
                    }
                    thread::sleep(RETRY);
                    continue;
                }
                held = Some(nonce);
                break;
            }
            Err(error) if !transient(&error) => {
                log::warn!(target: "cross-process-lock", "{label} lock unavailable: {error}");
                status = Status::Unavailable;
                break;
            }
            Err(_) => {
                let stale = (takeovers < MAX_TAKEOVERS)
                    .then(|| inspect_stale(lock))
                    .flatten();
                if let Some(stale) = stale {
                    takeovers += 1;
                    match quarantine(lock, &stale) {
                        Takeover::Taken | Takeover::Changed => continue,
                        Takeover::Busy | Takeover::Failed => {
                            status = Status::Busy;
                            break;
                        }
                    }
                }
                if Instant::now() >= deadline {
                    status = Status::Busy;
                    break;
                }
                thread::sleep(RETRY);
            }
        }
    }
    let Some(nonce) = held else {
        log::warn!(target: "cross-process-lock", "{label} lock not held: {status:?}");
        return task(status);
    };
    let _held = Held {
        lock,
        label,
        nonce,
        heartbeat: Some(Heartbeat::start(lock.to_owned())),
    };
    task(Status::Held)
}

/// The lock while its task runs; dropping it stops the heartbeat, then releases.
struct Held<'a> {
    lock: &'a Path,
    label: &'a str,
    nonce: String,
    heartbeat: Option<Heartbeat>,
}

impl Drop for Held<'_> {
    fn drop(&mut self) {
        drop(self.heartbeat.take());
        release(self.lock, self.label, &self.nonce);
    }
}

/// Touches a file every two seconds until dropped.
struct Heartbeat {
    stop: Option<std::sync::mpsc::Sender<()>>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Heartbeat {
    fn start(path: PathBuf) -> Self {
        let (stop, wait) = std::sync::mpsc::channel::<()>();
        let thread = thread::spawn(move || {
            while let Err(std::sync::mpsc::RecvTimeoutError::Timeout) = wait.recv_timeout(HEARTBEAT)
            {
                let now = std::time::SystemTime::now();
                let times = std::fs::FileTimes::new()
                    .set_accessed(now)
                    .set_modified(now);
                let touched = std::fs::File::options()
                    .write(true)
                    .open(&path)
                    .and_then(|f| f.set_times(times));
                if let Err(error) = touched {
                    log::debug!(target: "cross-process-lock", "heartbeat missed: {error}");
                }
            }
        });
        Self {
            stop: Some(stop),
            thread: Some(thread),
        }
    }
}

impl Drop for Heartbeat {
    fn drop(&mut self) {
        drop(self.stop.take());
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            log::error!(target: "cross-process-lock", "heartbeat thread panicked");
        }
    }
}

/// `publishAdvisoryLock`: a file created here, holding this process and a fresh nonce.
fn publish(lock: &Path) -> io::Result<String> {
    use std::io::Write as _;
    let nonce = uuid::Uuid::new_v4().to_string();
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(lock)?;
    let text = format!(
        r#"{{"pid":{},"startedAt":{},"nonce":"{nonce}"}}"#,
        std::process::id(),
        now_ms() as i64
    );
    if let Err(error) = file.write_all(text.as_bytes()) {
        drop(file);
        if let Err(removed) = std::fs::remove_file(lock) {
            log::warn!(target: "cross-process-lock", "a half-written lock stayed: {removed}");
        }
        return Err(error);
    }
    Ok(nonce)
}

/// `AdvisoryStaleCandidate`: the file as seen, and its record if it had one.
#[derive(Clone, Debug, PartialEq)]
struct Candidate {
    identity: Identity,
    record: Option<Advisory>,
}

/// `inspectStaleAdvisoryLock`: only a stale record whose owner is gone.
fn inspect_stale(lock: &Path) -> Option<Candidate> {
    let identity = Identity::lstat(lock).ok()?;
    if now_ms() - identity.mtime_ms <= STALE_MS {
        return None;
    }
    let record = record::read_advisory(lock).ok()?;
    if let Some(r) = &record
        && identity::kill0(r.pid) != identity::Kill::Missing
    {
        return None;
    }
    Some(Candidate { identity, record })
}

/// `inspectAdvisoryCandidate`.
fn inspect(lock: &Path) -> Option<Candidate> {
    let identity = Identity::lstat(lock).ok()?;
    let record = record::read_advisory(lock).ok()?;
    Some(Candidate { identity, record })
}

enum Takeover {
    Taken,
    Changed,
    Busy,
    Failed,
}

/// `quarantineAdvisoryLock`: under the gate, the same stale file moves aside and goes.
fn quarantine(lock: &Path, expected: &Candidate) -> Takeover {
    let Some(gate) = gate::acquire(lock, Duration::ZERO) else {
        return Takeover::Busy;
    };
    let moved = append(
        lock,
        &format!(".stale-{}-{}", std::process::id(), uuid::Uuid::new_v4()),
    );
    let result = (|| {
        if inspect_stale(lock).as_ref() != Some(expected) {
            return Takeover::Changed;
        }
        if let Err(error) = std::fs::rename(lock, &moved) {
            return if fsops::missing(&error) {
                Takeover::Changed
            } else {
                Takeover::Failed
            };
        }
        if inspect(&moved).as_ref() != Some(expected) {
            restore_moved(&moved, lock);
            return Takeover::Failed;
        }
        remove_with_retry(&moved);
        if moved.exists() {
            Takeover::Failed
        } else {
            log::info!(target: "cross-process-lock", "took over a stale lock at {}", lock.display());
            Takeover::Taken
        }
    })();
    gate::release(gate);
    result
}

/// `releaseAdvisoryLock`: the nonce, not the pid, proves the file is still ours.
fn release(lock: &Path, label: &str, nonce: &str) {
    let Some(gate) = gate::acquire(lock, RELEASE_GATE_WAIT) else {
        pending().insert(lock.to_owned(), nonce.to_owned());
        log::warn!(target: "cross-process-lock", "{label} lock release gate is busy; leaving a recoverable owner record");
        return;
    };
    let releasing = append(
        lock,
        &format!(".release-{}-{}", std::process::id(), uuid::Uuid::new_v4()),
    );
    let ours = |path: &Path| matches!(record::read_advisory(path), Ok(Some(r)) if r.nonce.as_deref() == Some(nonce));
    if !ours(lock) {
        log::warn!(target: "cross-process-lock", "{label} lock was taken over; leaving it to its new owner");
    } else if std::fs::rename(lock, &releasing).is_err() {
        pending().insert(lock.to_owned(), nonce.to_owned());
    } else if ours(&releasing) {
        pending().remove(lock);
        remove_with_retry(&releasing);
    } else {
        restore_moved(&releasing, lock);
        log::warn!(target: "cross-process-lock", "{label} lock identity changed during release; preserving the isolated record");
    }
    gate::release(gate);
}

/// `recoverPendingAdvisoryLock`.
fn recover_pending(lock: &Path) {
    let Some(nonce) = pending().get(lock).cloned() else {
        return;
    };
    match record::read_advisory(lock) {
        Err(_) => {}
        Ok(Some(r)) if r.nonce.as_deref() == Some(nonce.as_str()) => {
            release(lock, "advisory recovery", &nonce);
        }
        Ok(_) => {
            pending().remove(lock);
        }
    }
}

fn pending() -> std::sync::MutexGuard<'static, HashMap<PathBuf, String>> {
    PENDING.lock().expect("the pending map never poisons")
}
