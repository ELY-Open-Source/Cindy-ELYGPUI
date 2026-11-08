//! `schemaMigrationLease.ts`, the reader side: the port only reads Cindy's schema.

mod stat;

use std::{
    fs,
    io::{self, Write as _},
    path::{Path, PathBuf},
};

use sha2::{Digest as _, Sha256};

/// `SchemaLeaseFailureReason`, less `readers-active`, which only a writer meets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    WriterActive,
    LockCorrupt,
    LockUnreadable,
    RecoveryBusy,
}

impl Failure {
    /// The `localDbFatal.schemaLease.<reason>` key part.
    pub fn reason(self) -> &'static str {
        match self {
            Self::WriterActive => "writer-active",
            Self::LockCorrupt => "lock-corrupt",
            Self::LockUnreadable => "lock-unreadable",
            Self::RecoveryBusy => "recovery-busy",
        }
    }
}

#[derive(serde::Serialize)]
struct Owner {
    pid: u64,
    token: String,
    #[serde(rename = "createdAt")]
    created_at: String,
}

fn hex(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// `createOwner`: this process, a fresh token, `toISOString()`.
fn new_owner() -> Owner {
    let now = jiff::Timestamp::now();
    Owner {
        pid: u64::from(std::process::id()),
        token: uuid(),
        created_at: now.strftime("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
    }
}

/// `parseOwner` over `JSON.parse`: a positive safe-integer pid, a token and a dated `createdAt`.
fn parse_owner(bytes: &[u8]) -> Option<Owner> {
    let value = crate::cindy_json::roundtrip(std::str::from_utf8(bytes).ok()?).ok()?;
    let pid = value.get("pid")?.as_f64()?;
    let safe = pid.fract() == 0.0 && pid > 0.0 && pid <= 9_007_199_254_740_991.0;
    let token = value.get("token")?.as_str().filter(|t| !t.is_empty())?;
    let created_at = value.get("createdAt")?.as_str()?;
    created_at.parse::<jiff::Timestamp>().ok()?;
    safe.then(|| Owner {
        pid: pid as u64,
        token: token.to_owned(),
        created_at: created_at.to_owned(),
    })
}

/// `isProcessAlive`: unknown means alive.
#[cfg(unix)]
fn alive(pid: u64) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return true;
    };
    // SAFETY: signal 0 only checks that the process exists.
    if unsafe { libc::kill(pid, 0) } == 0 {
        return true;
    }
    io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
}

/// libuv's `uv_kill(pid, 0)`: gone when the pid is invalid or the process has exited.
#[cfg(windows)]
fn alive(pid: u64) -> bool {
    let Ok(pid) = u32::try_from(pid) else {
        return true;
    };
    use windows::Win32::{
        Foundation::{CloseHandle, ERROR_INVALID_PARAMETER, STILL_ACTIVE},
        System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    };
    // SAFETY: a handle opened here and closed here.
    unsafe {
        let process = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(process) => process,
            Err(error) => return error.code() != ERROR_INVALID_PARAMETER.to_hresult(),
        };
        let mut code = 0u32;
        let running =
            GetExitCodeProcess(process, &mut code).is_err() || code == STILL_ACTIVE.0 as u32;
        let _ = CloseHandle(process);
        running
    }
}

enum Record {
    Missing,
    Unreadable,
    Readable {
        owner: Option<Owner>,
        fingerprint: String,
        identity: String,
        bytes: Vec<u8>,
    },
}

fn missing(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::NotFound
}

/// `readRecord`: one stable read of a lease file, fingerprinted as Cindy does.
fn read_record(path: &Path) -> Record {
    let before = match stat::lstat(path) {
        Ok(before) => before,
        Err(error) if missing(&error) => return Record::Missing,
        Err(_) => return Record::Unreadable,
    };
    let bytes = match crate::bounded_read::read(path, 16 * 1024) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return Record::Unreadable,
        Err(error) => {
            let gone = error.downcast_ref::<io::Error>().is_some_and(missing);
            return if gone {
                Record::Missing
            } else {
                Record::Unreadable
            };
        }
    };
    let after = match stat::lstat(path) {
        Ok(after) => after,
        Err(error) if missing(&error) => return Record::Missing,
        Err(_) => return Record::Unreadable,
    };
    if !before.file || before != after {
        return Record::Unreadable;
    }
    let s = after;
    let full = format!(
        "{}:{}:{}:{}:{}:{}",
        s.dev, s.ino, s.size, s.mtime, s.ctime, s.birth
    );
    let mut hasher = Sha256::new();
    hasher.update(full.as_bytes());
    hasher.update(&bytes);
    Record::Readable {
        owner: parse_owner(&bytes),
        fingerprint: format!("{:x}", hasher.finalize()),
        identity: format!("{}:{}:{}:{}:{}", s.dev, s.ino, s.size, s.mtime, s.birth),
        bytes,
    }
}

/// `sidecarPath`: siblings of reader records stay outside the readers directory.
fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let parent = path.parent().expect("a lease file has a folder");
    let parent_name = parent.file_name().map(|n| n.to_string_lossy().into_owned());
    match parent_name.filter(|n| n.ends_with(".schema-readers")) {
        Some(readers) => {
            let file = path
                .file_name()
                .expect("a lease file has a name")
                .to_string_lossy();
            let dir = parent.parent().expect("the readers folder has a parent");
            dir.join(format!("{readers}.{file}{suffix}"))
        }
        None => PathBuf::from(format!("{}{suffix}", path.display())),
    }
}

mod recovery;
mod transition;

/// `writeOwnerExclusive`: flush a private candidate, then link it in without replacing anyone.
fn write_exclusive(path: &Path, owner: &Owner) -> io::Result<bool> {
    let staging = sidecar(path, &format!(".candidate-{}-{}", owner.pid, owner.token));
    let result = (|| {
        let mut file = create_private(&staging)?;
        let mut text = serde_json::to_string(owner).map_err(io::Error::other)?;
        text.push('\n');
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        drop(file);
        match fs::hard_link(&staging, path) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(false),
            Err(error) => Err(error),
        }
    })();
    let _ = fs::remove_file(&staging);
    result
}

/// `openSync(path, 'wx', 0o600)`.
fn create_private(path: &Path) -> io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)
}

/// A held reader record; dropping it releases it, as `releaseOwnedFile` does.
pub struct Lease {
    path: PathBuf,
    owner: Owner,
}

impl Drop for Lease {
    fn drop(&mut self) {
        if let Record::Readable {
            owner: Some(current),
            ..
        } = read_record(&self.path)
            && current.token == self.owner.token
            && current.pid == self.owner.pid
        {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// `acquireSchemaMigrationReaderLease`: held until the database closes.
pub fn acquire_reader(db: &Path) -> Result<Lease, Failure> {
    transition::with_gate(db, || {
        let writer = PathBuf::from(format!("{}.schema-writer.lock", db.display()));
        if let Some(failure) = recovery::clear_inactive(&writer) {
            return Ok(Err(failure));
        }
        let readers = PathBuf::from(format!("{}.schema-readers", db.display()));
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        builder.create(&readers)?;
        let owner = new_owner();
        let path = readers.join(format!("{}-{}.json", owner.pid, owner.token));
        if !write_exclusive(&path, &owner)? {
            return Ok(Err(Failure::WriterActive));
        }
        let lease = Lease { path, owner };
        if writer.exists() {
            drop(lease);
            return Ok(Err(Failure::WriterActive));
        }
        Ok(Ok(lease))
    })
}

#[cfg(test)]
mod tests;
