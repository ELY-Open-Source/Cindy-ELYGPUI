//! `fs.lstatSync(…, { bigint: true })` as libuv fills it, for the lease record identity.

use std::{io, path::Path};

/// dev, ino, size, mtime, ctime and birth time, the times in ns since the epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stat {
    pub file: bool,
    pub dev: u64,
    pub ino: u64,
    pub size: u64,
    pub mtime: i128,
    pub ctime: i128,
    pub birth: i128,
}

#[cfg(unix)]
pub fn lstat(path: &Path) -> io::Result<Stat> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::symlink_metadata(path)?;
    let ns = |secs: i64, nanos: i64| i128::from(secs) * 1_000_000_000 + i128::from(nanos);
    let birth = meta
        .created()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok());
    Ok(Stat {
        file: meta.is_file(),
        dev: meta.dev(),
        ino: meta.ino(),
        size: meta.size(),
        mtime: ns(meta.mtime(), meta.mtime_nsec()),
        ctime: ns(meta.ctime(), meta.ctime_nsec()),
        birth: birth.map_or(0, |d| d.as_nanos() as i128),
    })
}

/// libuv on Windows: volume serial, file index, and FILETIMEs moved to the Unix epoch.
#[cfg(windows)]
pub fn lstat(path: &Path) -> io::Result<Stat> {
    use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
    use windows::Win32::{
        Foundation::HANDLE,
        Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, FILE_BASIC_INFO, FILE_FLAG_BACKUP_SEMANTICS,
            FILE_FLAG_OPEN_REPARSE_POINT, FileBasicInfo, GetFileInformationByHandle,
            GetFileInformationByHandleEx,
        },
    };
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0 | FILE_FLAG_BACKUP_SEMANTICS.0)
        .open(path)?;
    let meta = file.metadata()?;
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
    let unix = |filetime: i64| (i128::from(filetime) - 116_444_736_000_000_000) * 100;
    Ok(Stat {
        file: meta.is_file() && !meta.file_type().is_symlink(),
        dev: u64::from(id.dwVolumeSerialNumber),
        ino: (u64::from(id.nFileIndexHigh) << 32) | u64::from(id.nFileIndexLow),
        size: meta.len(),
        mtime: unix(basic.LastWriteTime),
        ctime: unix(basic.ChangeTime),
        birth: unix(basic.CreationTime),
    })
}
