//! Cindy's `readBoundedFileNoFollow`: one handle checks the file and reads it.

use std::{
    fs::{File, OpenOptions},
    io::{self, Read},
    path::Path,
};

use anyhow::{Result, bail};

/// Opens the last path part itself, never what a link points to.
fn open_no_follow(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::custom_flags(&mut options, libc::O_NOFOLLOW);
    #[cfg(windows)]
    std::os::windows::fs::OpenOptionsExt::custom_flags(
        &mut options,
        windows::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT.0,
    );
    options.open(path)
}

/// `sameHandleVersion`: identity, mode, size, mtime and ctime of the open handle.
#[cfg(unix)]
fn version(file: &File) -> io::Result<[i64; 6]> {
    use std::os::unix::fs::MetadataExt;
    let m = file.metadata()?;
    let ns = |secs: i64, nanos: i64| secs * 1_000_000_000 + nanos;
    Ok([
        m.dev() as i64,
        m.ino() as i64,
        i64::from(m.mode()),
        m.size() as i64,
        ns(m.mtime(), m.mtime_nsec()),
        ns(m.ctime(), m.ctime_nsec()),
    ])
}

/// libuv's stat on a handle: volume serial and file index, attributes, write and change time.
#[cfg(windows)]
fn version(file: &File) -> io::Result<[i64; 6]> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::{
        Foundation::HANDLE,
        Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, FILE_BASIC_INFO, FileBasicInfo, GetFileInformationByHandle,
            GetFileInformationByHandleEx,
        },
    };
    let handle = HANDLE(file.as_raw_handle());
    let mut id = BY_HANDLE_FILE_INFORMATION::default();
    let mut basic = FILE_BASIC_INFO::default();
    // SAFETY: an open handle and structs sized for each call.
    unsafe {
        GetFileInformationByHandle(handle, &mut id)?;
        GetFileInformationByHandleEx(
            handle,
            FileBasicInfo,
            std::ptr::from_mut(&mut basic).cast(),
            size_of::<FILE_BASIC_INFO>() as u32,
        )?;
    }
    let index = (i64::from(id.nFileIndexHigh) << 32) | i64::from(id.nFileIndexLow);
    let size = (i64::from(id.nFileSizeHigh) << 32) | i64::from(id.nFileSizeLow);
    let attributes = i64::from(basic.FileAttributes);
    let serial = i64::from(id.dwVolumeSerialNumber);
    Ok([
        serial,
        index,
        attributes,
        size,
        basic.LastWriteTime,
        basic.ChangeTime,
    ])
}

/// The file's bytes, or `None` for a link, a non-file or one over `max_bytes`.
pub fn read(path: &Path, max_bytes: u64) -> Result<Option<Vec<u8>>> {
    let mut file = open_no_follow(path)?;
    let meta = file.metadata()?;
    if meta.file_type().is_symlink() || !meta.is_file() || meta.len() > max_bytes {
        return Ok(None);
    }
    let before = version(&file)?;
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    (&mut file).take(meta.len()).read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    if bytes.len() as u64 != meta.len() || !after.is_file() || version(&file)? != before {
        bail!("source file changed while being read");
    }
    Ok(Some(bytes))
}

#[cfg(test)]
mod tests {
    use super::{read, version};

    #[test]
    fn reads_regular_files_within_the_limit_only() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a");
        std::fs::write(&file, b"12345").unwrap();
        assert_eq!(read(&file, 5).unwrap().as_deref(), Some(&b"12345"[..]));
        assert_eq!(read(&file, 4).unwrap(), None, "over the limit");
        assert!(read(&dir.path().join("missing"), 5).is_err());
        let handle = std::fs::File::open(&file).unwrap();
        let before = version(&handle).unwrap();
        let mtime = handle.metadata().unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&file, b"54321").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&file)
            .unwrap()
            .set_modified(mtime)
            .unwrap();
        let after = version(&handle).unwrap();
        assert_eq!(
            (after[3], after[4]),
            (before[3], before[4]),
            "same size and mtime"
        );
        assert_ne!(after, before, "the change time sees a same-length rewrite");
        #[cfg(unix)]
        {
            let link = dir.path().join("link");
            std::os::unix::fs::symlink(&file, &link).unwrap();
            assert!(read(&link, 5).is_err(), "O_NOFOLLOW refuses the link");
        }
    }
}
