use std::{fs, sync::Mutex};

use jiff::Zoned;
use log::LevelFilter;
use regex::Regex;

use super::{MainLog, config_level, record_line};

// Cindy's shared/mainLogRecordFormat.ts MAIN_LOG_RECORD_HEAD_RE.
const CINDY_HEAD: &str = r"^\[(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}[+-]\d{2}:\d{2})\] \[(TRACE|DEBUG|INFO |WARN |ERROR|FATAL)\] \[([^\]]*)\] ";

fn main_log(dir: &std::path::Path) -> MainLog {
    MainLog {
        dir: dir.to_owned(),
        level: LevelFilter::Info,
        day: Mutex::new(None),
        mirror: std::sync::atomic::AtomicBool::new(false),
    }
}

#[test]
fn line_matches_cindy_head_and_continuations_cannot_forge_one() {
    let head = Regex::new(CINDY_HEAD).unwrap();
    let now = Zoned::now();
    let forged = record_line(&now, "INFO ", "x", "a");
    let line = record_line(
        &now,
        "WARN ",
        "plugin",
        &format!("first\n{}", forged.trim_end()),
    );
    let lines: Vec<&str> = line.lines().collect();
    assert!(head.is_match(lines[0]), "{line}");
    assert!(!head.is_match(lines[1]), "{line}");
}

#[test]
fn new_file_starts_with_sentinel_and_expired_files_go() {
    let dir = tempfile::tempdir().unwrap();
    let now = Zoned::now();
    let old = now.date().saturating_sub(jiff::Span::new().days(30));
    let kept = now.date().saturating_sub(jiff::Span::new().days(29));
    for name in [
        format!("main-{old}.log"),
        format!("main-{kept}.log"),
        "main.log".into(),
    ] {
        fs::write(dir.path().join(name), "x").unwrap();
    }
    let log = main_log(dir.path());
    log.write(&now, "body\n");
    let today = fs::read_to_string(log.file_for(now.date())).unwrap();
    assert!(
        today
            .lines()
            .next()
            .unwrap()
            .ends_with("[logger] #cindy-log-format:2")
    );
    assert!(today.ends_with("body\n"));
    assert!(!dir.path().join(format!("main-{old}.log")).exists());
    assert!(dir.path().join(format!("main-{kept}.log")).exists());
    assert!(dir.path().join("main.log").exists());
}

#[test]
fn config_must_be_object_with_string_level() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("log-config.json"), r#"{"level":1}"#).unwrap();
    assert!(config_level(dir.path()).is_err());
    fs::write(dir.path().join("log-config.json"), "null").unwrap();
    assert!(config_level(dir.path()).is_err());
    fs::write(dir.path().join("log-config.json"), r#"{"level":"debug"}"#).unwrap();
    assert_eq!(config_level(dir.path()).unwrap().as_deref(), Some("debug"));
}
