use std::{
    path::Path,
    time::{Duration, SystemTime},
};

use super::{
    Status, gate,
    identity::{self, Identity, parse_recorded_start_ms},
    record, with_lock,
};

const WAIT: Duration = Duration::from_millis(300);

/// A pid that cannot run: above Linux's `pid_max` limit.
const DEAD_PID: u32 = 4_194_400;

fn age(path: &Path, secs: u64) {
    let past = SystemTime::now() - Duration::from_secs(secs);
    let times = std::fs::FileTimes::new()
        .set_accessed(past)
        .set_modified(past);
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_times(times)
        .unwrap();
}

fn leftovers(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

#[test]
fn holds_writes_a_legacy_readable_record_and_releases_clean() {
    let dir = tempfile::tempdir().unwrap();
    let lock = dir.path().join("sidebar-settings.json.lock");
    let seen = with_lock(&lock, "test", WAIT, |status| {
        let raw: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&lock).unwrap()).unwrap();
        (status, raw)
    });
    assert_eq!(seen.0, Status::Held);
    assert_eq!(seen.1["pid"], std::process::id());
    assert!(seen.1["startedAt"].as_f64().is_some());
    assert!(record::valid_nonce(seen.1["nonce"].as_str().unwrap()));
    assert!(
        leftovers(dir.path()).is_empty(),
        "{:?}",
        leftovers(dir.path())
    );
    assert_eq!(with_lock(&lock, "test", WAIT, |s| s), Status::Held, "again");
}

#[test]
fn a_fresh_lock_is_busy_and_a_live_owner_keeps_a_stale_one() {
    let dir = tempfile::tempdir().unwrap();
    let lock = dir.path().join("x.lock");
    std::fs::write(&lock, r#"{"pid":1,"startedAt":1,"nonce":"n"}"#).unwrap();
    assert_eq!(with_lock(&lock, "test", WAIT, |s| s), Status::Busy);
    let own = format!(
        r#"{{"pid":{},"startedAt":1,"nonce":"n"}}"#,
        std::process::id()
    );
    std::fs::write(&lock, own).unwrap();
    age(&lock, 60);
    assert_eq!(
        with_lock(&lock, "test", WAIT, |s| s),
        Status::Busy,
        "its pid runs"
    );
    let extra = format!(
        r#"{{"pid":{},"startedAt":1,"nonce":"n","extra":1e400}}"#,
        std::process::id()
    );
    std::fs::write(&lock, extra).unwrap();
    age(&lock, 60);
    assert_eq!(
        with_lock(&lock, "test", WAIT, |s| s),
        Status::Busy,
        "JSON.parse takes 1e400"
    );
    assert!(lock.exists());
}

#[test]
fn takes_over_a_stale_lock_whose_owner_is_gone() {
    let dir = tempfile::tempdir().unwrap();
    let lock = dir.path().join("x.lock");
    let dead = format!(r#"{{"pid":{DEAD_PID},"startedAt":1,"nonce":"old"}}"#);
    std::fs::write(&lock, dead).unwrap();
    age(&lock, 60);
    assert_eq!(with_lock(&lock, "test", WAIT, |s| s), Status::Held);
    std::fs::write(&lock, "not json").unwrap();
    age(&lock, 60);
    assert_eq!(
        with_lock(&lock, "test", WAIT, |s| s),
        Status::Held,
        "malformed residue"
    );
    assert!(
        leftovers(dir.path()).is_empty(),
        "{:?}",
        leftovers(dir.path())
    );
}

#[test]
fn release_leaves_a_successor_with_another_nonce() {
    let dir = tempfile::tempdir().unwrap();
    let lock = dir.path().join("x.lock");
    let successor = r#"{"pid":1,"startedAt":2,"nonce":"successor"}"#;
    with_lock(&lock, "test", WAIT, |_| {
        std::fs::write(&lock, successor).unwrap()
    });
    assert_eq!(std::fs::read_to_string(&lock).unwrap(), successor);
}

#[test]
fn reclaims_a_crashed_gate_before_the_stale_lock() {
    let dir = tempfile::tempdir().unwrap();
    let lock = dir.path().join("x.lock");
    std::fs::write(&lock, format!(r#"{{"pid":{DEAD_PID},"startedAt":1}}"#)).unwrap();
    age(&lock, 60);
    let gates = dir.path().join("x.lock.reclaim.d");
    std::fs::create_dir(&gates).unwrap();
    let nonce = uuid::Uuid::new_v4();
    let crashed = gates.join(format!("gate-{DEAD_PID}-{nonce}.json"));
    let text = format!(
        r#"{{"pid":{DEAD_PID},"startedAt":1,"nonce":"{nonce}","processStartIdentity":"start-ms:1","state":"held"}}"#
    );
    std::fs::write(&crashed, text).unwrap();
    age(&crashed, 60);
    assert_eq!(with_lock(&lock, "test", WAIT, |s| s), Status::Held);
    assert!(
        leftovers(dir.path()).is_empty(),
        "{:?}",
        leftovers(dir.path())
    );
}

#[test]
fn a_live_gate_blocks_and_a_stale_legacy_gate_goes() {
    let dir = tempfile::tempdir().unwrap();
    let lock = dir.path().join("x.lock");
    let held = gate::acquire(&lock, WAIT).expect("the gate is free");
    assert_eq!(with_lock(&lock, "test", WAIT, |s| s), Status::Busy);
    gate::release(held);
    let legacy = dir.path().join("x.lock.reclaim");
    std::fs::write(&legacy, format!(r#"{{"pid":{DEAD_PID},"startedAt":1}}"#)).unwrap();
    age(&legacy, 60);
    assert_eq!(with_lock(&lock, "test", WAIT, |s| s), Status::Held);
    assert!(!legacy.exists());
}

#[test]
fn reads_gate_records_as_read_lock_record_does() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("gate.json");
    let read = |text: &str, complete: bool| {
        std::fs::write(&file, text).unwrap();
        record::read_full(&file, complete)
    };
    assert_eq!(read("[]", false), record::Read::Malformed);
    assert_eq!(
        read(r#"{"pid":1.5,"startedAt":1}"#, false),
        record::Read::Malformed
    );
    assert_eq!(
        read(r#"{"pid":1,"startedAt":1,"nonce":""}"#, false),
        record::Read::Malformed
    );
    assert_eq!(
        read(r#"{"pid":1,"startedAt":1,"state":"x"}"#, false),
        record::Read::Malformed
    );
    assert_eq!(
        read(r#"{"pid":1,"startedAt":1}"#, true),
        record::Read::Malformed
    );
    assert_eq!(
        read(r#"{"pid":1,"startedAt":1e400}"#, false),
        record::Read::Malformed,
        "startedAt must be finite"
    );
    let extra = read(r#"{"pid":1,"startedAt":1,"x":1e400}"#, false);
    assert!(matches!(extra, record::Read::Record(_)), "{extra:?}");
    let record::Read::Record(r) = read(
        r#"{"pid":1,"startedAt":1,"nonce":null,"state":"released"}"#,
        false,
    ) else {
        panic!("a record");
    };
    assert!(r.released && r.nonce.is_none());
    assert_eq!(
        record::read_full(&dir.path().join("none"), false),
        record::Read::Missing
    );
}

#[test]
fn compares_process_identities_as_cindy_does() {
    assert_eq!(
        parse_recorded_start_ms("start-ms:1700000000000"),
        Some(1.7e12)
    );
    assert_eq!(parse_recorded_start_ms("start-ms:0"), None);
    let ticks = parse_recorded_start_ms("638000000000000000").unwrap();
    assert_eq!(
        ticks,
        638_000_000_000_000_000.0 / 10_000.0 - 62_135_596_800_000.0
    );
    assert_eq!(
        parse_recorded_start_ms("Sun Nov 8 14:00:00 2026"),
        Some(1_794_146_400_000.0)
    );
    let current = Identity {
        key: "start-ms:10000".into(),
        started_ms: 10_000.0,
    };
    assert!(identity::identities_match("start-ms:11999", &current));
    assert!(!identity::identities_match("start-ms:12001", &current));
    assert!(identity::identities_match("start-ms:10000", &current));
}

#[cfg(unix)]
#[test]
fn reads_this_process_start_from_ps() {
    match identity::probe(i64::from(std::process::id())) {
        identity::Probe::Found(found) => {
            assert!((found.started_ms - super::fsops::now_ms()).abs() < 86_400_000.0);
        }
        other => panic!("ps gave {other:?}"),
    }
    assert_eq!(
        identity::kill0(i64::from(DEAD_PID)),
        identity::Kill::Missing
    );
}
