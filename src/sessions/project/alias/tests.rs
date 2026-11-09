//! `projectAliases.test.ts`'s write rules on a real table.

use rusqlite::Connection;

use super::{laid, upsert};

fn db() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE project_aliases (project_key text PRIMARY KEY, alias text NOT NULL,
         updated_at integer NOT NULL);",
    )
    .unwrap();
    db
}

fn all(db: &Connection) -> Vec<(String, String, i64)> {
    let mut q = db
        .prepare("SELECT project_key, alias, updated_at FROM project_aliases ORDER BY project_key")
        .unwrap();
    q.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn an_alias_replaces_the_last_and_a_blank_one_clears_it() {
    let db = db();
    upsert(&db, "local:/work/app/", "  App  ", "linux", 1).unwrap();
    assert_eq!(all(&db), [("local:/work/app".into(), "App".into(), 1)]);
    upsert(&db, "local:/work/app", "Renamed", "linux", 2).unwrap();
    assert_eq!(all(&db), [("local:/work/app".into(), "Renamed".into(), 2)]);
    upsert(&db, "local:/work/app", "   ", "linux", 3).unwrap();
    assert_eq!(all(&db), []);
}

#[test]
fn windows_folders_replace_every_spelling() {
    let db = db();
    db.execute_batch(
        "INSERT INTO project_aliases VALUES ('local:C:/Work/App', 'Old', 1);
         INSERT INTO project_aliases VALUES ('local:c:/work/app', 'Older', 0);
         INSERT INTO project_aliases VALUES ('local:/work/app', 'Posix', 0);",
    )
    .unwrap();
    upsert(&db, "local:C:/WORK/APP", "New", "win32", 5).unwrap();
    assert_eq!(
        all(&db),
        [
            ("local:/work/app".into(), "Posix".into(), 0),
            ("local:C:/WORK/APP".into(), "New".into(), 5),
        ]
    );
    upsert(&db, "local:C:/work/app", "Case", "linux", 6).unwrap();
    assert_eq!(all(&db).len(), 3, "other platforms keep case");
}

#[test]
fn a_bad_key_or_a_long_alias_is_refused() {
    let db = db();
    assert!(upsert(&db, "", "A", "linux", 1).is_err());
    assert!(upsert(&db, "local:/a", &"x".repeat(81), "linux", 1).is_err());
    assert!(
        upsert(&db, "local:/a", &"😀".repeat(41), "linux", 1).is_err(),
        "UTF-16 units"
    );
    upsert(&db, "local:/a", &"😀".repeat(40), "linux", 1).unwrap();
    upsert(&db, "/b", "Bare", "linux", 1).unwrap();
    assert!(
        all(&db).iter().any(|(k, ..)| k == "local:/b"),
        "a bare folder is local"
    );
}

#[test]
fn the_list_shows_the_new_name_first() {
    let rows = vec![
        ("local:/b".to_owned(), "B".to_owned()),
        ("local:/a/".to_owned(), "Old".to_owned()),
    ];
    assert_eq!(
        laid(&rows, "local:/a", " A "),
        [
            ("local:/a".into(), "A".into()),
            ("local:/b".into(), "B".into())
        ]
    );
    assert_eq!(
        laid(&rows, "local:/a", ""),
        [("local:/b".into(), "B".into())]
    );
}
