use gpui::TestAppContext;
use rusqlite::Connection;

use super::{Failure, Gate, gate, session};
use crate::{chat_store::Store, preferences::Preferences, router, sessions};

/// The list's columns, and the message columns the history view reads.
pub(super) fn db() -> Connection {
    let db = sessions::tests::db();
    db.execute_batch(
        "DROP TABLE messages;
         CREATE TABLE messages (id TEXT PRIMARY KEY, client_id TEXT NOT NULL, session_id TEXT NOT NULL,
           role TEXT NOT NULL, content TEXT NOT NULL, tool_use_id TEXT, agent_meta TEXT,
           agent_kind TEXT, created_at INTEGER NOT NULL, rewind_at INTEGER);",
    )
    .unwrap();
    sessions::tests::add(&db, "s1", "desktop", "active", 1);
    sessions::tests::add(&db, "old", "desktop", "archived", 2);
    sessions::tests::add(&db, "gone", "desktop", "deleted", 3);
    sessions::tests::add(&db, "make", "cindy-make", "active", 4);
    db
}

/// The globals the route needs, with `s1` listed and the database served.
pub(super) fn install(cx: &mut TestAppContext) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    cx.update(|cx| {
        cx.set_global(Preferences::load(dir.path()).unwrap());
        cx.set_global(crate::i18n::Translator::new("en").unwrap());
        crate::chat_store::install(cx);
        sessions::install(std::path::PathBuf::new(), None, cx);
        crate::localdb::install(None, cx);
        sessions::load_for_tests(&[("s1", "active", "desktop")], cx);
        router::install(cx);
        super::install(cx);
    });
    dir
}

pub(super) fn serve(cx: &mut TestAppContext) {
    cx.update(|cx| crate::localdb::serve(db(), cx));
}

pub(super) fn go(to: &str, cx: &mut TestAppContext) {
    cx.update(|cx| router::navigate(to, cx));
    cx.run_until_parked();
}

fn status(cx: &mut TestAppContext) -> Option<String> {
    cx.update(|cx| session(cx).map(|s| s.status.clone()))
}

fn path(cx: &mut TestAppContext) -> String {
    cx.update(|cx| router::path(cx).to_owned())
}

fn visits(id: &str, cx: &mut TestAppContext) -> usize {
    cx.update(|cx| Store::global(cx).visits(id))
}

#[gpui::test]
fn a_listed_task_opens_at_once_and_visits_the_store(cx: &mut TestAppContext) {
    let _dir = install(cx);
    serve(cx);
    cx.update(|cx| router::navigate("/cc-agent/s1", cx));
    // The list knows it, so the content needs no read.
    assert_eq!(cx.update(|cx| gate(cx)), Some(Gate::Open));
    assert_eq!(visits("s1", cx), 1);
    cx.run_until_parked();
    let state = cx.update(|cx| Store::global(cx).state("s1")).unwrap();
    assert!(state.history_loaded);
    assert_eq!(status(cx).as_deref(), Some("active"));
    // The read that lands does not visit again.
    assert_eq!(visits("s1", cx), 1);
    go("/cc-agent/new", cx);
    assert_eq!(cx.update(|cx| gate(cx)), None);
    assert_eq!(visits("s1", cx), 0);
}

#[gpui::test]
fn an_unlisted_task_waits_for_its_read(cx: &mut TestAppContext) {
    let _dir = install(cx);
    serve(cx);
    cx.update(|cx| router::navigate("/cc-agent/old", cx));
    assert_eq!(cx.update(|cx| gate(cx)), Some(Gate::Waiting));
    assert_eq!(visits("old", cx), 0);
    cx.run_until_parked();
    // An archived task stays open, from the read row.
    assert_eq!(cx.update(|cx| gate(cx)), Some(Gate::Open));
    assert_eq!(status(cx).as_deref(), Some("archived"));
    assert_eq!(visits("old", cx), 1);
}

#[gpui::test]
fn a_missing_task_draws_the_entry_error(cx: &mut TestAppContext) {
    let _dir = install(cx);
    serve(cx);
    go("/cc-agent/nope", cx);
    assert_eq!(
        cx.update(|cx| gate(cx)),
        Some(Gate::Failed(Failure::Missing))
    );
    assert_eq!(visits("nope", cx), 0);
    // The address stays; going back is the user's choice.
    assert_eq!(path(cx), "/cc-agent/nope");
}

#[gpui::test]
fn a_failed_read_retries(cx: &mut TestAppContext) {
    let _dir = install(cx);
    go("/cc-agent/old", cx);
    assert_eq!(
        cx.update(|cx| gate(cx)),
        Some(Gate::Failed(Failure::Failed))
    );
    serve(cx);
    // Retry draws nothing while it reads.
    cx.update(super::retry);
    assert_eq!(cx.update(|cx| gate(cx)), Some(Gate::Waiting));
    cx.run_until_parked();
    assert_eq!(cx.update(|cx| gate(cx)), Some(Gate::Open));
    assert_eq!(status(cx).as_deref(), Some("archived"));
}

#[gpui::test]
fn a_later_failed_read_keeps_the_found_task(cx: &mut TestAppContext) {
    let _dir = install(cx);
    serve(cx);
    go("/cc-agent/old", cx);
    cx.update(|cx| crate::localdb::install(None, cx));
    cx.update(super::reread);
    cx.run_until_parked();
    assert_eq!(cx.update(|cx| gate(cx)), Some(Gate::Open));
    assert_eq!(status(cx).as_deref(), Some("archived"));
}

#[gpui::test]
fn a_deleted_task_is_purged_and_left(cx: &mut TestAppContext) {
    let _dir = install(cx);
    serve(cx);
    go("/cc-agent/gone", cx);
    // The index goes on to the first listed task.
    assert_eq!(path(cx), "/cc-agent/s1");
    assert_eq!(visits("gone", cx), 0);
}

#[gpui::test]
fn a_task_deleted_while_shown_leaves_on_the_next_read(cx: &mut TestAppContext) {
    let _dir = install(cx);
    serve(cx);
    go("/cc-agent/old", cx);
    write(
        cx,
        "UPDATE sessions SET status = 'deleted' WHERE id = 'old'",
    );
    cx.update(super::reread);
    cx.run_until_parked();
    assert_eq!(path(cx), "/cc-agent/s1");
    assert_eq!(visits("old", cx), 0);
    assert!(cx.update(|cx| Store::global(cx).state("old")).is_none());
}

#[gpui::test]
fn a_cindy_make_task_goes_back(cx: &mut TestAppContext) {
    let _dir = install(cx);
    serve(cx);
    go("/cc-agent/make", cx);
    assert_eq!(path(cx), "/cc-agent/s1");
    assert_eq!(visits("make", cx), 0);
}

#[gpui::test]
fn a_read_for_a_task_no_longer_shown_is_dropped(cx: &mut TestAppContext) {
    let _dir = install(cx);
    serve(cx);
    // The first read is still queued when the route moves on.
    cx.update(|cx| router::navigate("/cc-agent/old", cx));
    go("/cc-agent/nope", cx);
    assert_eq!(
        cx.update(|cx| gate(cx)),
        Some(Gate::Failed(Failure::Missing))
    );
    assert_eq!(visits("old", cx), 0);
}

#[gpui::test]
fn a_listed_archived_task_opens_but_shows_its_read_row(cx: &mut TestAppContext) {
    let _dir = install(cx);
    cx.update(|cx| sessions::load_for_tests(&[("old", "archived", "desktop")], cx));
    serve(cx);
    cx.update(|cx| router::navigate("/cc-agent/old", cx));
    // The gate finds it in any bucket; the view's list row is the `active` bucket's.
    assert_eq!(cx.update(|cx| gate(cx)), Some(Gate::Open));
    assert_eq!(status(cx), None);
    cx.run_until_parked();
    assert_eq!(status(cx).as_deref(), Some("archived"));
}

#[gpui::test]
fn the_gate_finds_a_task_in_the_all_bucket(cx: &mut TestAppContext) {
    let _dir = install(cx);
    let all = [("old", "archived", "desktop")];
    cx.update(|cx| sessions::load_buckets_for_tests(&[], &all, cx));
    serve(cx);
    cx.update(|cx| router::navigate("/cc-agent/old", cx));
    assert_eq!(cx.update(|cx| gate(cx)), Some(Gate::Open));
}

/// Review 9.3a r1: the content's read ends after the visit, so the purge holds.
#[gpui::test]
fn an_unlisted_deleted_task_stays_purged(cx: &mut TestAppContext) {
    let _dir = install(cx);
    serve(cx);
    go("/cc-agent/gone", cx);
    assert_eq!(path(cx), "/cc-agent/s1");
    assert!(cx.update(|cx| Store::global(cx).state("gone")).is_none());
}

/// Review 9.3a r1: the content's row never opens the gate for it.
#[gpui::test]
fn a_task_gone_from_list_and_database_rechecks_its_gate(cx: &mut TestAppContext) {
    let _dir = install(cx);
    serve(cx);
    go("/cc-agent/s1", cx);
    assert_eq!(status(cx).as_deref(), Some("active"));
    write(cx, "DELETE FROM sessions WHERE id = 's1'");
    cx.update(|cx| {
        sessions::load_for_tests(&[], cx);
        super::reread(cx);
    });
    cx.run_until_parked();
    assert_eq!(
        cx.update(|cx| gate(cx)),
        Some(Gate::Failed(Failure::Missing))
    );
    assert_eq!(visits("s1", cx), 0);
}

/// Review 9.3a r1: the entry error replaces the content, which leaves the store.
#[gpui::test]
fn the_entry_error_unmounts_the_visit(cx: &mut TestAppContext) {
    let _dir = install(cx);
    go("/cc-agent/s1", cx);
    assert_eq!(visits("s1", cx), 1);
    cx.update(|cx| sessions::load_for_tests(&[], cx));
    cx.run_until_parked();
    assert_eq!(
        cx.update(|cx| gate(cx)),
        Some(Gate::Failed(Failure::Failed))
    );
    assert_eq!(visits("s1", cx), 0);
    assert_eq!(status(cx), None);
}

#[gpui::test]
fn a_content_read_lands_only_on_its_own_task(cx: &mut TestAppContext) {
    let _dir = install(cx);
    let all = [("old", "archived", "desktop")];
    cx.update(|cx| sessions::load_buckets_for_tests(&[("s1", "active", "desktop")], &all, cx));
    serve(cx);
    // The index opened s1 before the database; it reads again, then the route moves on.
    assert_eq!(path(cx), "/cc-agent/s1");
    cx.update(super::reread);
    cx.update(|cx| router::navigate("/cc-agent/old", cx));
    while cx.executor().tick() {
        let id = cx.update(|cx| session(cx).map(|s| s.id.clone()));
        assert_ne!(id.as_deref(), Some("s1"), "s1's read landed on old");
    }
    cx.run_until_parked();
    assert_eq!(status(cx).as_deref(), Some("archived"));
}

/// Review 9.3a r2: the list learning the task cancels the gate's read in flight.
#[gpui::test]
fn a_cancelled_gate_read_never_opens_the_gate_later(cx: &mut TestAppContext) {
    let _dir = install(cx);
    serve(cx);
    cx.update(|cx| router::navigate("/cc-agent/old", cx));
    assert_eq!(cx.update(|cx| gate(cx)), Some(Gate::Waiting));
    cx.update(|cx| sessions::load_for_tests(&[("old", "archived", "desktop")], cx));
    cx.run_until_parked();
    assert_eq!(cx.update(|cx| gate(cx)), Some(Gate::Open));
    write(cx, "DELETE FROM sessions WHERE id = 'old'");
    cx.update(|cx| sessions::load_for_tests(&[], cx));
    cx.run_until_parked();
    assert_eq!(
        cx.update(|cx| gate(cx)),
        Some(Gate::Failed(Failure::Missing))
    );
    assert_eq!(visits("old", cx), 0);
}

/// Review 9.3a r2: a read from an earlier mount of the same task is dropped.
#[gpui::test]
fn an_old_content_read_misses_a_remount_of_its_task(cx: &mut TestAppContext) {
    let _dir = install(cx);
    serve(cx);
    let old = [("old", "archived", "desktop")];
    cx.update(|cx| sessions::load_buckets_for_tests(&[("s1", "active", "desktop")], &old, cx));
    let before = cx.update(|cx| Store::global(cx).epoch("s1"));
    write(cx, "UPDATE sessions SET status = 'deleted' WHERE id = 's1'");
    cx.update(super::reread);
    write(cx, "UPDATE sessions SET status = 'active' WHERE id = 's1'");
    cx.update(|cx| router::navigate("/cc-agent/old", cx));
    cx.update(|cx| router::navigate("/cc-agent/s1", cx));
    cx.run_until_parked();
    assert_eq!(cx.update(|cx| Store::global(cx).epoch("s1")), before);
    assert_eq!(path(cx), "/cc-agent/s1");
    assert_eq!(status(cx).as_deref(), Some("active"));
}

/// Runs `sql` on the served database.
fn write(cx: &mut TestAppContext, sql: &'static str) {
    let done = cx.update(|cx| {
        let crate::localdb::State::Ready(db) = &cx.global::<crate::localdb::LocalDb>().state else {
            panic!("served")
        };
        db.call(move |conn| conn.execute(sql, []))
    });
    assert!(futures::executor::block_on(done).unwrap().is_ok());
}

type Stream = (Vec<String>, Option<(bool, usize)>);

fn stream(cx: &mut TestAppContext) -> Option<Stream> {
    use crate::render_items::Item;
    cx.update(|cx| {
        let shown = super::items::items(cx)?.as_ref().unwrap();
        let keys = shown.items.iter().map(|item| item.key().to_owned());
        let group = shown.items.iter().find_map(|item| match &**item {
            Item::WorkGroup {
                children, deferred, ..
            } => Some((deferred.as_ref()?.expanded, children.len())),
            _ => None,
        });
        Some((keys.collect(), group))
    })
}

fn deferred(
    cx: &mut TestAppContext,
    run: impl FnOnce(&crate::render_items::Deferred, &mut gpui::App),
) {
    use crate::render_items::Item;
    cx.update(|cx| {
        let shown = super::items::items(cx).unwrap().as_ref().unwrap().clone();
        let work = shown.items.iter().find_map(|item| match &**item {
            Item::WorkGroup { deferred, .. } => deferred.as_ref(),
            _ => None,
        });
        run(work.unwrap(), cx);
    });
    cx.run_until_parked();
}

#[gpui::test]
fn the_ready_view_renders_and_its_folded_work_opens(cx: &mut TestAppContext) {
    let _dir = install(cx);
    cx.update(|cx| {
        let db = db();
        let put = crate::chat_store::tests::put;
        put(&db, 0, "user", "\"Look\"", None);
        let call = r#"{"toolName":"Bash","toolUseId":"b","input":{"command":"ls"}}"#;
        put(&db, 1, "tool_use", call, Some("b"));
        put(&db, 2, "tool_result", "\"ok\"", Some("b"));
        put(&db, 3, "assistant", "\"Done\"", None);
        crate::localdb::serve(db, cx);
    });
    assert_eq!(stream(cx), None);
    go("/cc-agent/s1", cx);
    let folded = Some((
        vec!["msg-c0".into(), "work-c1".into(), "msg-c3".into()],
        Some((false, 0)),
    ));
    assert_eq!(stream(cx), folded);
    let view = |cx: &gpui::App| Store::global(cx).view("s1").unwrap();
    let mut release = None;
    deferred(cx, |work, cx| {
        release = Some(work.set_visible(&view(cx), true, false))
    });
    let open = stream(cx).unwrap().1;
    assert_eq!(open, Some((true, 1)));
    cx.update(|_| release.take().unwrap()());
    cx.run_until_parked();
    // The small body stays read once its hold lets go.
    assert_eq!(stream(cx).unwrap().1, Some((false, 1)));
    deferred(cx, |work, cx| work.toggle(&view(cx)));
    assert_eq!(stream(cx).unwrap().1, open);
    deferred(cx, |work, cx| work.retry(&view(cx)));
    assert_eq!(stream(cx).unwrap().1, open);
    go("/cc-agent/new", cx);
    assert_eq!(stream(cx), None);
}
