//! Cindy's `ensureReady` for an instance that never migrates: lease, open, check, or refuse.

use std::{path::Path, sync::Once};

use rusqlite::{Connection, OpenFlags};

use super::{admission, cjk, lease, worker::Db};

/// The strings `open` may need, translated before it leaves the main thread.
pub const TEXT_KEYS: [&str; 5] = [
    "localDbFatal.schemaLease.writer-active",
    "localDbFatal.schemaLease.lock-corrupt",
    "localDbFatal.schemaLease.lock-unreadable",
    "localDbFatal.schemaLease.recovery-busy",
    "localDbFatal.passiveCorrupt.description",
];

/// The `LocalDbFatalScreen` input: Cindy's error code and its message.
#[derive(Debug, Clone)]
pub struct Fatal {
    pub code: &'static str,
    pub message: String,
}

/// `restrictDbFilePermissions`: the database and its WAL siblings, owner only.
fn restrict(db: &Path) {
    #[cfg(unix)]
    for suffix in ["", "-wal", "-shm"] {
        use std::os::unix::fs::PermissionsExt;
        let target = std::path::PathBuf::from(format!("{}{suffix}", db.display()));
        match std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600)) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                log::warn!(target: "localDb", "[localDb] chmod 0600 failed for {}: {error}", target.display());
            }
            _ => {}
        }
    }
    #[cfg(not(unix))]
    let _ = db;
}

/// `restrictLegacyBackupPermissions`: `.bak.clean`, `.bak.<ISO>` and `.bak.nuke-*`.
fn restrict_backups(db: &Path) {
    restrict(&std::path::PathBuf::from(format!(
        "{}.bak.clean",
        db.display()
    )));
    let (Some(dir), Some(base)) = (db.parent(), db.file_name()) else {
        return;
    };
    let base = base.to_string_lossy();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with(&format!("{base}.bak.")) {
            continue;
        }
        let iso = name.match_indices(".bak.").any(|(at, _)| {
            let d = &name.as_bytes()[at + 5..];
            d.len() >= 11
                && d[4] == b'-'
                && d[7] == b'-'
                && d[10] == b'T'
                && [0..4, 5..7, 8..10]
                    .iter()
                    .all(|r| d[r.clone()].iter().all(u8::is_ascii_digit))
        });
        if iso || name.starts_with(&format!("{base}.bak.nuke-")) {
            restrict(&entry.path());
        }
    }
}

/// `loadSqliteVec`: every connection gets the 0.1.9 extension Cindy ships.
fn register_vec() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        // SAFETY: the entry point sqlite-vec documents for `sqlite3_auto_extension`.
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute::<
                *const (),
                unsafe extern "C" fn(
                    *mut rusqlite::ffi::sqlite3,
                    *mut *mut std::ffi::c_char,
                    *const rusqlite::ffi::sqlite3_api_routines,
                ) -> std::ffi::c_int,
            >(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }
    });
}

/// `createBetterSqliteDatabase` then `openWithPragmas`, in Cindy's order.
fn connect(path: &Path) -> anyhow::Result<Connection> {
    restrict(path);
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let db = Connection::open_with_flags(path, flags)?;
    restrict(path);
    cjk::register(&db)?;
    let probe: String = db.query_row("SELECT cjk_seg(?) AS v", ["探针"], |row| row.get(0))?;
    anyhow::ensure!(
        probe == "探 针",
        "cjk_seg probe failed: expected '探 针', got {probe:?}"
    );
    cjk::ensure_triggers(&db)?;
    db.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;
         PRAGMA synchronous = NORMAL;
         PRAGMA temp_store = MEMORY;
         PRAGMA mmap_size = 268435456;
         PRAGMA cache_size = -65536;
         PRAGMA busy_timeout = 5000;",
    )?;
    cjk::ensure_triggers(&db)?;
    Ok(db)
}

fn corrupt(error: &anyhow::Error) -> bool {
    let sqlite = error
        .downcast_ref::<rusqlite::Error>()
        .and_then(rusqlite::Error::sqlite_error_code);
    sqlite == Some(rusqlite::ErrorCode::DatabaseCorrupt)
        || format!("{error:#}").to_lowercase().contains("corrupt")
}

/// Opens `path`, or names why not. Nothing here creates, migrates or repairs.
pub fn open(path: &Path, t: &dyn Fn(&str) -> String) -> Result<Db, Fatal> {
    if !path.exists() {
        let message = format!(
            "{} does not exist; Cindy creates it, this port never does",
            path.display()
        );
        log::warn!(target: "localDb", "local database missing: {}", path.display());
        return Err(Fatal {
            code: "DB_INIT_FAILED",
            message,
        });
    }
    let lease = lease::acquire_reader(path).map_err(|failure| {
        log::warn!(target: "localDb", "localDb.schemaLease.blocked reason {}", failure.reason());
        let message = t(&format!("localDbFatal.schemaLease.{}", failure.reason()));
        Fatal {
            code: "DB_SCHEMA_LOCKED",
            message,
        }
    })?;
    register_vec();
    let db = connect(path).map_err(|error| {
        log::error!(target: "localDb", "local database did not open: {error:#}");
        if corrupt(&error) {
            let message = t("localDbFatal.passiveCorrupt.description");
            Fatal {
                code: "DB_SCHEMA_LOCKED",
                message,
            }
        } else {
            Fatal {
                code: "DB_INIT_FAILED",
                message: format!("{error:#}"),
            }
        }
    })?;
    restrict_backups(path);
    match db.query_row("SELECT vec_version()", [], |row| row.get::<_, String>(0)) {
        Ok(version) => log::info!(target: "localDb", "localDb.sqliteVec.loaded version {version}"),
        Err(error) => log::error!(target: "localDb", "localDb.sqliteVec.failed: {error}"),
    }
    let report = admission::check(&db, path);
    if !report.issues.is_empty() {
        log::error!(target: "localDb", "localDb.ensureReady.packagedReadOnlyCompatibilityMismatch dbPath {} databaseVersion {} checkoutVersion {} issues {:?}", path.display(), report.database_version, report.checkout_version, report.issues);
        let summary: Vec<&str> = report
            .issues
            .iter()
            .take(3)
            .map(admission::Issue::kind)
            .collect();
        let details = if summary.is_empty() {
            "未知原因".to_owned()
        } else {
            summary.join(", ")
        };
        let message = format!(
            "当前安装版无法安全打开共享数据库。请关闭其它开发实例后重试，或安装包含该本地数据 schema 的更新版本。（详情：{details}）"
        );
        return Err(Fatal {
            code: "MIGRATE_FAILED",
            message,
        });
    }
    let worker = Db::spawn(db, lease).map_err(|error| Fatal {
        code: "DB_INIT_FAILED",
        message: error.to_string(),
    })?;
    log::info!(target: "localDb", "localDb.ensureReady.ok dbPath {}", path.display());
    Ok(worker)
}
