//! The file steps the lock shares: errno classes, stat identity, bounded retries.

use std::{
    ffi::OsString,
    io::{self, ErrorKind},
    path::{Path, PathBuf},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use super::RETRY;

const REMOVE_ATTEMPTS: u32 = 3;
const EMPTY_DIR_REMOVE_ATTEMPTS: u32 = 5;

/// `Date.now()`.
pub fn now_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock is after 1970")
        .as_secs_f64()
        * 1000.0
}

/// `path` with `suffix` added to its last part.
pub fn append(path: &Path, suffix: &str) -> PathBuf {
    let mut name = OsString::from(path.as_os_str());
    name.push(suffix);
    PathBuf::from(name)
}

/// `ENOENT` or `ENOTDIR`.
pub fn missing(error: &io::Error) -> bool {
    matches!(error.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory)
}

/// `EPERM`, `EBUSY` or `EACCES`; Windows sharing and lock violations are `EBUSY` to libuv.
fn busy(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        ErrorKind::PermissionDenied | ErrorKind::ResourceBusy
    ) || (cfg!(windows) && matches!(error.raw_os_error(), Some(32 | 33)))
}

/// A publish error that the wait loop retries: `EEXIST` or a busy path.
pub fn transient(error: &io::Error) -> bool {
    error.kind() == ErrorKind::AlreadyExists || busy(error)
}

/// `MalformedLockIdentity`: what `fs.stat` says the file is.
#[derive(Clone, Debug, PartialEq)]
pub struct Identity {
    pub dev: u64,
    pub ino: u64,
    pub size: u64,
    pub mtime_ms: f64,
    /// `birthtimeMs`, or 0 where the filesystem has none.
    pub birth_ms: f64,
}

impl Identity {
    /// `fs.lstat`.
    pub fn lstat(path: &Path) -> io::Result<Self> {
        Self::of(path, false)
    }

    /// `fs.stat`.
    pub fn stat(path: &Path) -> io::Result<Self> {
        Self::of(path, true)
    }

    #[cfg(unix)]
    fn of(path: &Path, follow: bool) -> io::Result<Self> {
        use std::os::unix::fs::MetadataExt;
        let m = if follow {
            std::fs::metadata(path)?
        } else {
            std::fs::symlink_metadata(path)?
        };
        let ms = |secs: i64, nanos: i64| secs as f64 * 1000.0 + nanos as f64 / 1e6;
        let birth_ms = m
            .created()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0.0, |d| d.as_secs_f64() * 1000.0);
        Ok(Self {
            dev: m.dev(),
            ino: m.ino(),
            size: m.size(),
            mtime_ms: ms(m.mtime(), m.mtime_nsec()),
            birth_ms,
        })
    }

    #[cfg(windows)]
    fn of(path: &Path, follow: bool) -> io::Result<Self> {
        use std::os::windows::fs::OpenOptionsExt as _;
        use windows::Win32::Storage::FileSystem::{
            FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES,
            FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        };
        // libuv's stat: attributes only, so a file shut to readers still answers.
        let mut flags = FILE_FLAG_BACKUP_SEMANTICS.0;
        if !follow {
            flags |= FILE_FLAG_OPEN_REPARSE_POINT.0;
        }
        let file = std::fs::OpenOptions::new()
            .access_mode(FILE_READ_ATTRIBUTES.0)
            .share_mode((FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE).0)
            .custom_flags(flags)
            .open(path)?;
        let m = file.metadata()?;
        let [dev, ino] = crate::bounded_read::identity(&file)?;
        let ms = |t: io::Result<SystemTime>| {
            t.ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0.0, |d| d.as_secs_f64() * 1000.0)
        };
        Ok(Self {
            dev: dev as u64,
            ino: ino as u64,
            size: m.len(),
            mtime_ms: ms(m.modified()),
            birth_ms: ms(m.created()),
        })
    }
}

/// `pathExists`: `fs.stat` succeeds.
pub fn exists(path: &Path) -> bool {
    std::fs::metadata(path).is_ok()
}

/// `removePathWithRetry`: `fs.rm` with `force`, retried while the path is busy.
pub fn remove_with_retry(path: &Path) {
    for attempt in 0..REMOVE_ATTEMPTS {
        match std::fs::remove_file(path) {
            Ok(()) => return,
            Err(error) if missing(&error) => return,
            Err(error) if busy(&error) => {
                log::debug!(target: "cross-process-lock", "remove {} busy: {error}", path.display());
                if attempt + 1 < REMOVE_ATTEMPTS {
                    thread::sleep(RETRY * (attempt + 1));
                }
            }
            Err(error) => {
                log::warn!(target: "cross-process-lock", "remove {} failed: {error}", path.display());
                return;
            }
        }
    }
}

/// `removeEmptyDirectoryWithRetry`: never recursive.
pub fn remove_empty_dir_with_retry(dir: &Path) {
    for attempt in 0..EMPTY_DIR_REMOVE_ATTEMPTS {
        match std::fs::remove_dir(dir) {
            Ok(()) => return,
            Err(error) if missing(&error) => return,
            Err(error) if busy(&error) || error.kind() == ErrorKind::DirectoryNotEmpty => {
                if attempt + 1 < EMPTY_DIR_REMOVE_ATTEMPTS {
                    thread::sleep(RETRY * (attempt + 1));
                }
            }
            Err(error) => {
                log::warn!(target: "cross-process-lock", "rmdir {} failed: {error}", dir.display());
                return;
            }
        }
    }
}

/// `restoreMovedPathSafely`: a hard link back, so a successor at `to` stays.
pub fn restore_moved(from: &Path, to: &Path) {
    match std::fs::hard_link(from, to) {
        Ok(()) => remove_with_retry(from),
        Err(error) => {
            log::warn!(target: "cross-process-lock", "{} kept aside: {error}", from.display());
        }
    }
}
