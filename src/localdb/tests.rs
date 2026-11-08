use std::path::{Path, PathBuf};

use rusqlite::Connection;

use super::{admission, open};

/// A database at the reference commit's schema history, with its runtime manifest.
pub(super) fn reference_db(dir: &Path) -> PathBuf {
    let path = dir.join("cindy-local-v1.db");
    let db = Connection::open(&path).unwrap();
    db.execute_batch(
        "CREATE TABLE migration_meta (key text PRIMARY KEY, value text);
         CREATE TABLE migration_history (seq integer PRIMARY KEY, file_name text, content_hash text);",
    )
    .unwrap();
    let reference = admission::reference();
    for m in &reference {
        db.execute(
            "INSERT INTO migration_history VALUES (?, ?, ?)",
            (m.seq, &m.file_name, &m.sql_hash),
        )
        .unwrap();
    }
    let last = reference.last().unwrap().seq.to_string();
    db.execute(
        "INSERT INTO migration_meta VALUES ('schema_version', ?)",
        [last],
    )
    .unwrap();
    let manifest = serde_json::json!({
        "version": 1,
        "legacyBaselineVersion": -1,
        "migrations": reference.iter().map(|m| serde_json::json!({
            "seq": m.seq, "fileName": m.file_name, "sqlHash": m.sql_hash, "scriptHash": m.script_hash,
        })).collect::<Vec<_>>(),
    });
    std::fs::write(
        format!("{}.migration-runtime.json", path.display()),
        manifest.to_string(),
    )
    .unwrap();
    path
}

fn kinds(path: &Path) -> Vec<&'static str> {
    let db = Connection::open(path).unwrap();
    admission::check(&db, path)
        .issues
        .iter()
        .map(admission::Issue::kind)
        .collect()
}

#[test]
fn the_reference_history_is_admitted_and_drift_is_named() {
    let dir = tempfile::tempdir().unwrap();
    let path = reference_db(dir.path());
    assert!(kinds(&path).is_empty());
    let db = Connection::open(&path).unwrap();
    db.execute("UPDATE migration_meta SET value = '0147'", [])
        .unwrap();
    assert_eq!(
        kinds(&path)[..2],
        ["history-unavailable", "schema-version-behind"]
    );
    db.execute("UPDATE migration_meta SET value = '149'", [])
        .unwrap();
    assert_eq!(kinds(&path), ["schema-version-ahead"]);
    db.execute("UPDATE migration_meta SET value = '148'", [])
        .unwrap();
    db.execute(
        "UPDATE migration_history SET content_hash = 'x' WHERE seq = 3",
        [],
    )
    .unwrap();
    db.execute(
        "INSERT INTO migration_history VALUES (900, 'later.sql', 'h')",
        [],
    )
    .unwrap();
    assert_eq!(
        kinds(&path),
        ["history-entry-mismatch", "history-entry-unexpected"]
    );
}

#[test]
fn the_runtime_manifest_must_match_save_for_0062s_repair() {
    let dir = tempfile::tempdir().unwrap();
    let path = reference_db(dir.path());
    let manifest_path = format!("{}.migration-runtime.json", path.display());
    let mut manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
    let entry = manifest["migrations"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|m| m["seq"] == 62)
        .unwrap();
    entry["scriptHash"] = "0ea82003cac0419a4a483b0afc1743d6fdba0b50085104720d5b2561e721072d".into();
    std::fs::write(&manifest_path, manifest.to_string()).unwrap();
    assert!(kinds(&path).is_empty(), "the one known repair");
    let entry = manifest["migrations"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|m| m["seq"] == 63)
        .unwrap();
    entry["sqlHash"] = "other".into();
    std::fs::write(&manifest_path, manifest.to_string()).unwrap();
    assert_eq!(kinds(&path), ["runtime-manifest-mismatch"]);
    std::fs::remove_file(&manifest_path).unwrap();
    assert_eq!(kinds(&path), ["runtime-manifest-unavailable"]);
}

fn texts(key: &str) -> String {
    format!("<{key}>")
}

#[test]
fn open_never_creates_and_refuses_what_it_cannot_admit() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("cindy-local-v1.db");
    let fatal = open::open(&missing, &texts).err().unwrap();
    assert_eq!(fatal.code, "DB_INIT_FAILED");
    assert!(!missing.exists(), "the port never creates the database");
    let path = reference_db(dir.path());
    Connection::open(&path)
        .unwrap()
        .execute("UPDATE migration_meta SET value = '147'", [])
        .unwrap();
    let fatal = open::open(&path, &texts).err().unwrap();
    assert_eq!(fatal.code, "MIGRATE_FAILED");
    let message = "当前安装版无法安全打开共享数据库。请关闭其它开发实例后重试，或安装包含该本地数据 schema 的更新版本。（详情：schema-version-behind）";
    assert_eq!(fatal.message, message);
    let readers = PathBuf::from(format!("{}.schema-readers", path.display()));
    assert_eq!(
        std::fs::read_dir(&readers).unwrap().count(),
        0,
        "a refusal releases the lease"
    );
}

#[test]
fn an_admitted_database_holds_its_lease_until_it_closes() {
    let dir = tempfile::tempdir().unwrap();
    let path = reference_db(dir.path());
    let db = open::open(&path, &texts).expect("admitted");
    let readers = PathBuf::from(format!("{}.schema-readers", path.display()));
    assert_eq!(std::fs::read_dir(&readers).unwrap().count(), 1);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
    drop(db);
    let closed = (0..200).any(|_| {
        std::thread::sleep(std::time::Duration::from_millis(5));
        std::fs::read_dir(&readers).unwrap().count() == 0
    });
    assert!(closed, "closing releases the lease");
}
