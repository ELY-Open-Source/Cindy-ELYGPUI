//! `readAtomicFileSync`: a file `atomicWriteFileSync` keeps, its missing main file put back.

use std::{
    io::{self, ErrorKind},
    path::Path,
    time::Duration,
};

/// `TRANSIENT_RENAME_CODES`: `EBUSY`, `EACCES` and `ENOTEMPTY`; `EPERM` is no lock.
#[cfg(unix)]
fn transient(error: &io::Error) -> bool {
    matches!(
        error.raw_os_error(),
        Some(libc::EBUSY | libc::EACCES | libc::ENOTEMPTY)
    )
}

/// The Windows errors libuv's `uv_translate_sys_error` turns into those three.
#[cfg(windows)]
fn transient(error: &io::Error) -> bool {
    matches!(
        error.raw_os_error(),
        Some(10013 | 740 | 1920 | 33 | 231 | 32 | 145)
    )
}

/// `renameSyncWithRetry`: three retries 20, 40 and 60 ms apart for a transient lock.
fn rename(from: &Path, to: &Path) -> io::Result<()> {
    for attempt in 0.. {
        match std::fs::rename(from, to) {
            Err(error) if attempt < 3 && transient(&error) => {
                std::thread::sleep(Duration::from_millis(20 * (attempt + 1)));
            }
            done => return done,
        }
    }
    unreachable!("the loop returns")
}

/// The text, none only for a missing file; a `.bak` left unrestored is an error.
pub fn read(path: &Path) -> io::Result<Option<String>> {
    let backup = crate::cross_process_lock::append_suffix(path, ".bak");
    if !path.exists() && backup.exists() {
        rename(&backup, path).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "Cannot restore {} from its .bak backup: {error}",
                    path.display()
                ),
            )
        })?;
        log::warn!(target: "atomicWriteFile", "restored {} from its .bak", path.display());
    }
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::read;

    #[test]
    fn a_missing_file_is_none_and_a_left_backup_comes_back() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("shadowed.json");
        assert_eq!(read(&file).unwrap(), None);
        std::fs::write(dir.path().join("shadowed.json.bak"), "[\"b1\"]").unwrap();
        assert_eq!(read(&file).unwrap().as_deref(), Some("[\"b1\"]"));
        assert!(
            !dir.path().join("shadowed.json.bak").exists(),
            "renamed into place"
        );
    }

    #[test]
    fn a_folder_in_the_files_place_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("x.json")).unwrap();
        assert!(read(&dir.path().join("x.json")).is_err());
    }
}
