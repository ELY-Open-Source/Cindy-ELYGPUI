use std::path::{Path, PathBuf};

use super::{Failure, Record, acquire_reader, read_record, sidecar};

fn db(dir: &Path) -> PathBuf {
    dir.join("cindy-local-v1.db")
}

fn readers(db: &Path) -> Vec<String> {
    let dir = PathBuf::from(format!("{}.schema-readers", db.display()));
    std::fs::read_dir(dir)
        .map(|e| {
            e.map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

fn writer(db: &Path) -> PathBuf {
    PathBuf::from(format!("{}.schema-writer.lock", db.display()))
}

/// A pid no process holds: the kernel's maximum is far below it.
const DEAD: u32 = 0x3fff_fff0;

#[test]
fn a_reader_record_lives_as_long_as_the_lease() {
    let dir = tempfile::tempdir().unwrap();
    let db = db(dir.path());
    let lease = acquire_reader(&db).unwrap();
    let held = readers(&db);
    assert_eq!(held.len(), 1);
    assert!(held[0].starts_with(&format!("{}-", std::process::id())) && held[0].ends_with(".json"));
    let bytes = std::fs::read(lease.path.clone()).unwrap();
    let owner: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(bytes.ends_with(b"\n") && owner["createdAt"].as_str().unwrap().ends_with('Z'));
    drop(lease);
    assert!(readers(&db).is_empty(), "released on close");
    assert!(!PathBuf::from(format!("{}.schema-transition", db.display())).exists());
}

#[test]
fn a_live_writer_blocks_and_a_dead_one_is_cleared() {
    let dir = tempfile::tempdir().unwrap();
    let db = db(dir.path());
    let owner =
        |pid: u32| format!(r#"{{"pid":{pid},"token":"t","createdAt":"2026-01-01T00:00:00.000Z"}}"#);
    std::fs::write(writer(&db), owner(std::process::id())).unwrap();
    assert_eq!(acquire_reader(&db).err(), Some(Failure::WriterActive));
    std::fs::write(writer(&db), owner(DEAD)).unwrap();
    let lease = acquire_reader(&db).expect("a dead writer is cleared");
    assert!(!writer(&db).exists());
    let backups = std::fs::read_dir(dir.path()).unwrap().filter(|e| {
        e.as_ref()
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains(".schema-writer.lock.schema-lease-backup-")
    });
    assert_eq!(backups.count(), 1, "the dead record is kept for support");
    drop(lease);
}

#[test]
fn a_live_writer_is_read_as_json_parse_reads_it() {
    let dir = tempfile::tempdir().unwrap();
    let db = db(dir.path());
    let pid = std::process::id();
    let record = format!(
        r#"{{"pid":{pid}e0,"token":"t","createdAt":"2026-01-01T00:00:00.000Z","future":1e400}}"#
    );
    std::fs::write(writer(&db), &record).unwrap();
    for _ in 0..2 {
        assert_eq!(acquire_reader(&db).err(), Some(Failure::WriterActive));
    }
    assert_eq!(
        std::fs::read_to_string(writer(&db)).unwrap(),
        record,
        "never isolated"
    );
}

#[test]
fn an_unknown_writer_needs_a_second_boot() {
    let dir = tempfile::tempdir().unwrap();
    let db = db(dir.path());
    std::fs::write(writer(&db), b"not an owner").unwrap();
    assert_eq!(acquire_reader(&db).err(), Some(Failure::LockCorrupt));
    assert_eq!(
        acquire_reader(&db).err(),
        Some(Failure::LockCorrupt),
        "same boot"
    );
    let observation = PathBuf::from(format!("{}.recovery-observation", writer(&db).display()));
    let seen: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&observation).unwrap()).unwrap();
    let Record::Readable { fingerprint, .. } = read_record(&writer(&db)) else {
        panic!("readable")
    };
    assert_eq!(seen["fingerprint"], fingerprint.as_str());
    let other = serde_json::json!({ "boot": "linux:another-boot", "fingerprint": fingerprint });
    std::fs::write(&observation, other.to_string()).unwrap();
    let lease = acquire_reader(&db).expect("another boot saw the same file");
    assert!(!writer(&db).exists() && !observation.exists());
    drop(lease);
}

#[test]
fn the_transition_gate_yields_to_the_living_only() {
    let dir = tempfile::tempdir().unwrap();
    let db = db(dir.path());
    let gate = PathBuf::from(format!("{}.schema-transition", db.display()));
    let marker = |pid: u32| format!("owner-{pid}-0123abcd-0123-4567-89ab-0123456789ab");
    std::fs::create_dir(&gate).unwrap();
    std::fs::write(gate.join(marker(std::process::id())), b"").unwrap();
    assert_eq!(acquire_reader(&db).err(), Some(Failure::RecoveryBusy));
    std::fs::remove_file(gate.join(marker(std::process::id()))).unwrap();
    std::fs::write(gate.join(marker(DEAD)), b"").unwrap();
    drop(acquire_reader(&db).expect("a dead owner's gate is cleared"));
    std::fs::create_dir(&gate).unwrap();
    std::fs::write(gate.join("stranger"), b"").unwrap();
    assert_eq!(acquire_reader(&db).err(), Some(Failure::LockUnreadable));
}

#[test]
fn reader_siblings_live_beside_the_readers_folder() {
    let file = Path::new("/d/cindy.db.schema-readers/1-t.json");
    assert_eq!(
        sidecar(file, ".candidate-1-t"),
        Path::new("/d/cindy.db.schema-readers.1-t.json.candidate-1-t")
    );
    let writer = Path::new("/d/cindy.db.schema-writer.lock");
    assert_eq!(
        sidecar(writer, ".x"),
        Path::new("/d/cindy.db.schema-writer.lock.x")
    );
}
