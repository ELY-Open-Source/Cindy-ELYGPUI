use rusqlite::Connection;

use super::{
    settings,
    tests::{add, db},
    write,
};

#[test]
fn pins_and_unpins_as_update_session_in_db_does() {
    let db = db();
    add(&db, "s", "desktop", "active", 5);
    db.execute("UPDATE sessions SET summary = 'old' WHERE id = 's'", [])
        .unwrap();
    let read = |db: &Connection| {
        db.query_row(
            "SELECT pinned_at, summary, updated_at FROM sessions WHERE id = 's'",
            [],
            |r| {
                Ok((
                    r.get::<_, Option<i64>>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            },
        )
        .unwrap()
    };
    write::patch(&db, "s", &write::Meta::Pinned(Some(9))).unwrap();
    assert_eq!(
        read(&db),
        (Some(9), Some("old".into()), 5),
        "a pin keeps the summary"
    );
    write::patch(&db, "s", &write::Meta::Pinned(None)).unwrap();
    assert_eq!(read(&db), (None, None, 5), "an unpin clears it");
}

fn promote(dir: &std::path::Path, entry: &str, platform: &str) -> anyhow::Result<Vec<String>> {
    settings::save(dir, &settings::Mutation::Promote(entry.into()), platform)
}

#[test]
fn promotes_pins_as_save_pinned_order_does() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(settings::FILE);
    assert_eq!(promote(dir.path(), "a", "linux").unwrap(), ["a"]);
    assert_eq!(promote(dir.path(), "b", "linux").unwrap(), ["b", "a"]);
    assert_eq!(
        promote(dir.path(), "b", "linux").unwrap(),
        ["b", "a"],
        "already first"
    );
    std::fs::write(
        &file,
        r#"{"pinnedOrder":["project:local:D:/École/A","s","project:local:d:/école/a"],"other":1e400,"hiddenProjectKeys":["/x"]}"#,
    )
    .unwrap();
    assert_eq!(
        promote(dir.path(), "project:local:D:/ÉCOLE/A", "win32").unwrap(),
        ["project:local:D:/École/A", "s"],
        "one spelling per identity, the first kept"
    );
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!(
        saved["other"],
        serde_json::Value::Null,
        "JSON.stringify of Infinity"
    );
    assert_eq!(
        saved["hiddenProjectKeys"],
        serde_json::json!(["/x"]),
        "other keys stay"
    );
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("\n  \""), "two-space indent: {text}");
    assert!(!dir.path().join("sidebar-settings.json.tmp").exists());
    assert!(!dir.path().join("sidebar-settings.json.lock").exists());
    std::fs::write(&file, "[1]").unwrap();
    assert!(
        promote(dir.path(), "c", "linux").is_err(),
        "an unreadable file stays"
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "[1]");
    std::fs::remove_file(&file).unwrap();
    std::fs::write(dir.path().join("sidebar-settings.json.bak"), "{}").unwrap();
    assert!(
        promote(dir.path(), "c", "linux").is_err(),
        "a backup blocks"
    );
    assert!(!file.exists());
    assert!(promote(dir.path(), "", "linux").is_err());
}

#[gpui::test]
fn overlapping_pins_save_in_order_and_failures_leave_no_phantom(cx: &mut gpui::TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = super::view_tests::open(cx, dir.path());
    let order = |cx: &mut gpui::VisualTestContext| {
        cx.update(|_, cx| {
            let loaded = cx.global::<super::Sessions>().loaded.as_ref().unwrap();
            loaded.settings.pinned_order.clone()
        })
    };
    cx.update(|window, cx| {
        super::pin::toggle("a0", false, window, cx);
        super::pin::toggle("a1", false, window, cx);
    });
    assert_eq!(order(cx), ["a1", "a0"], "shown at once");
    cx.run_until_parked();
    assert!(
        order(cx).is_empty(),
        "no owner: both fail, nothing saved stays"
    );
    let owner = dir.path().join("owner");
    cx.update(|_, cx| {
        let store = cx.global_mut::<super::Sessions>();
        store.owner_dir = Some(owner.clone());
        store.watcher = Some(notify::recommended_watcher(|_| {}).unwrap());
    });
    cx.update(|window, cx| {
        super::pin::toggle("a0", false, window, cx);
        super::pin::toggle("a1", false, window, cx);
    });
    cx.run_until_parked();
    assert_eq!(order(cx), ["a1", "a0"]);
    let saved = settings::read(&owner, "linux").pinned_order;
    assert_eq!(saved, ["a1", "a0"], "the second save ran after the first");
    assert!(
        cx.update(|_, cx| cx.global::<super::Sessions>().owner_watched),
        "the folder the first pin made is watched"
    );
}

#[test]
fn a_read_during_a_save_moves_the_baseline_only() {
    let mut writes = super::pin::Writes::default();
    let mut read = vec!["disk".to_string()];
    writes.read(&mut read, &["shown".to_string()]);
    assert_eq!(read, ["disk"], "nothing pending: the read shows");
}
