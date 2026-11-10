//! What a click, a key and the menu on a link chip do.

use gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext, px};

use super::{lead, open, resolve, route};
use crate::{
    router,
    session_view::{
        links::tests::{click, focus, key, press, toasts},
        user_row::tests::{at, open as show, settle},
    },
    sessions::{self, group::Session, query},
};

const CHIP: &str = "msg-c0-chip-0";
const HERE: &str = "menu-row-Open in current window";
const APART: &str = "menu-row-Open in new window";
const LOCATE: &str = "menu-row-Locate project";
const COPY: &str = "menu-row-Copy link";
const FAILED: &str = "Failed to open link";
const NO_PROJECT: &str =
    "Project not found — it may have been archived or doesn't exist on this device";

fn path(cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| router::path(cx).to_owned())
}

/// A database with a lead, its worker, a worker without a team and a Cindy Make task.
fn teams() -> rusqlite::Connection {
    let db = sessions::tests::db();
    for (n, id) in ["lead", "worker", "stray", "plain"].iter().enumerate() {
        sessions::tests::add(&db, id, "desktop", "active", n as i64);
    }
    sessions::tests::add(&db, "make", "cindy-make", "active", 9);
    db.execute_batch(
        "UPDATE sessions SET orca_role = 'lead' WHERE id = 'lead';
         UPDATE sessions SET orca_role = 'worker' WHERE id IN ('worker', 'stray');
         CREATE TABLE orca_teams (id text PRIMARY KEY, lead_session_id text NOT NULL);
         CREATE TABLE orca_workers (id text PRIMARY KEY, team_id text NOT NULL,
           session_id text NOT NULL);
         INSERT INTO orca_teams VALUES ('t', 'lead');
         INSERT INTO orca_workers VALUES ('w', 't', 'worker');",
    )
    .unwrap();
    db
}

#[test]
fn a_task_s_route_follows_its_kind() {
    let row = query::by_ids(&teams(), &["plain".to_owned()])
        .unwrap()
        .remove(0);
    let task = |source: &str, worker: bool| Session {
        source: source.into(),
        orca_worker: worker,
        ..row.clone()
    };
    let none = || -> Option<String> { panic!("only a worker's lead is read") };
    assert_eq!(resolve("a", None, none), "/cc-agent/a");
    assert_eq!(
        resolve("a", Some(&task("desktop", false)), none),
        "/cc-agent/a"
    );
    assert_eq!(
        resolve("a", Some(&task("cindy-make", true)), none),
        "/cc-agent"
    );
    assert_eq!(
        resolve("a", Some(&task("cindy-make-merge", false)), none),
        "/cc-agent"
    );
    let worker = task("desktop", true);
    assert_eq!(
        resolve("a", Some(&worker), || Some("l".into())),
        "/cc-agent/l"
    );
    assert_eq!(resolve("a", Some(&worker), || None), "/cc-agent");
}

#[test]
fn the_route_is_read_from_the_task_and_its_team() {
    let db = teams();
    assert_eq!(lead(&db, "worker").unwrap().as_deref(), Some("lead"));
    assert_eq!(lead(&db, "stray").unwrap(), None);
    assert_eq!(route(&db, "plain"), "/cc-agent/plain");
    assert_eq!(route(&db, "lead"), "/cc-agent/lead");
    assert_eq!(route(&db, "worker"), "/cc-agent/lead");
    assert_eq!(route(&db, "stray"), "/cc-agent");
    assert_eq!(route(&db, "make"), "/cc-agent");
    assert_eq!(route(&db, "absent"), "/cc-agent/absent");
    // A read that fails is no task and no team, as Cindy's `catch` has it.
    db.execute_batch("DROP TABLE orca_workers").unwrap();
    assert_eq!(route(&db, "worker"), "/cc-agent");
    db.execute_batch("DROP TABLE bot_session_links").unwrap();
    assert_eq!(route(&db, "plain"), "/cc-agent/plain");
}

#[gpui::test]
fn a_click_on_a_session_link_opens_its_task(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = show(cx, dir.path(), 2400.0, &[("see cindy://session/old", None)]);
    assert_eq!(path(cx), "/cc-agent/s1");
    click(cx, CHIP, Modifiers::none());
    assert_eq!(path(cx), "/cc-agent/old");
    assert_eq!(toasts(cx), Vec::<String>::new());
}

#[gpui::test]
fn a_task_no_row_names_still_opens_its_page(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let body = "cindy://session/nowhere?device=d";
    let cx = show(cx, dir.path(), 2400.0, &[(body, None)]);
    click(cx, CHIP, Modifiers::none());
    assert_eq!(path(cx), "/cc-agent/nowhere");
}

#[gpui::test]
fn a_message_link_opens_its_task(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let body = "cindy://session/old?message=m";
    let cx = show(cx, dir.path(), 2400.0, &[(body, None)]);
    click(cx, CHIP, Modifiers::none());
    assert_eq!(path(cx), "/cc-agent/old");
}

#[gpui::test]
fn a_modifier_or_the_middle_button_asks_for_a_window(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = show(cx, dir.path(), 2400.0, &[("cindy://session/old", None)]);
    for (n, modifiers) in [Modifiers::shift(), Modifiers::command()]
        .into_iter()
        .enumerate()
    {
        click(cx, CHIP, modifiers);
        assert_eq!(toasts(cx).len(), n + 1, "{modifiers:?}");
    }
    press(cx, CHIP, MouseButton::Middle);
    assert_eq!(
        toasts(cx),
        [FAILED; 3],
        "task windows come with their chapter"
    );
    assert_eq!(path(cx), "/cc-agent/s1");
    // Alt is no such modifier.
    click(cx, CHIP, Modifiers::alt());
    assert_eq!(path(cx), "/cc-agent/old");
}

#[gpui::test]
fn enter_and_space_open_with_their_modifiers(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = show(cx, dir.path(), 2400.0, &[("cindy://session/old", None)]);
    focus(cx, CHIP);
    assert_eq!(
        path(cx),
        "/cc-agent/s1",
        "a press that ends elsewhere opens nothing"
    );
    key(cx, "shift-enter");
    assert_eq!(toasts(cx), [FAILED]);
    key(cx, "ctrl-space");
    assert_eq!(toasts(cx), [FAILED; 2]);
    key(cx, "space");
    assert_eq!(path(cx), "/cc-agent/old");
    assert_eq!(toasts(cx).len(), 2, "one open a key press");
}

#[gpui::test]
fn the_link_s_menu_opens_and_copies(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let href = "cindy://session/old";
    let cx = show(cx, dir.path(), 2400.0, &[(href, None)]);
    let chip = at(cx, CHIP).unwrap();
    press(cx, CHIP, MouseButton::Right);
    assert_eq!(path(cx), "/cc-agent/s1", "a right click opens no task");
    let (here, apart, copy) = (
        at(cx, HERE).unwrap(),
        at(cx, APART).unwrap(),
        at(cx, COPY).unwrap(),
    );
    assert!(here.top() >= chip.center().y && here.left() >= chip.center().x);
    assert!(here.top() < apart.top() && apart.top() < copy.top());
    cx.simulate_click(copy.center(), Modifiers::none());
    settle(cx);
    let copied = cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(copied.as_deref(), Some(href));
    assert_eq!(toasts(cx), ["Link copied"]);
    assert!(at(cx, COPY).is_none(), "a choice closes the menu");
    press(cx, CHIP, MouseButton::Right);
    let apart = at(cx, APART).unwrap().center();
    cx.simulate_click(apart, Modifiers::none());
    settle(cx);
    assert_eq!(toasts(cx), [FAILED, "Link copied"]);
    press(cx, CHIP, MouseButton::Right);
    let here = at(cx, HERE).unwrap().center();
    cx.simulate_click(here, Modifiers::none());
    settle(cx);
    assert_eq!(path(cx), "/cc-agent/old");
}

#[gpui::test]
fn a_right_press_focuses_the_chip(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = show(cx, dir.path(), 2400.0, &[("cindy://session/old", None)]);
    // Nothing held focus: once the menu shuts, focus is the chip's and a key opens the task.
    press(cx, CHIP, MouseButton::Right);
    cx.simulate_keystrokes("escape");
    settle(cx);
    cx.simulate_keystrokes("enter");
    settle(cx);
    assert_eq!(path(cx), "/cc-agent/old");
}

#[gpui::test]
fn the_menu_key_opens_the_menu_under_the_chip(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let body = "see cindy://session/old?message=m";
    let cx = show(cx, dir.path(), 2400.0, &[(body, None)]);
    focus(cx, CHIP);
    cx.simulate_keystrokes("shift-f10");
    settle(cx);
    let (chip, row) = (at(cx, CHIP).unwrap(), at(cx, HERE).unwrap());
    assert!(row.top() >= chip.bottom() && row.top() < chip.bottom() + px(16.0));
    assert!(row.left() >= chip.left() && row.left() < chip.left() + px(16.0));
    cx.simulate_keystrokes("escape");
    settle(cx);
    assert!(at(cx, HERE).is_none());
    // Focus is back on the chip: a key opens it.
    cx.simulate_keystrokes("enter");
    settle(cx);
    assert_eq!(path(cx), "/cc-agent/old");
}

#[gpui::test]
fn a_project_link_asks_the_sidebar_for_its_project(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let href = "cindy://project/%2Fwork%2Fgone";
    let body = format!("{href} https://example.com/a");
    let cx = show(cx, dir.path(), 2400.0, &[(&body, None)]);
    // A modifier changes nothing, and the middle button does nothing.
    click(cx, CHIP, Modifiers::shift());
    assert_eq!(toasts(cx), [NO_PROJECT]);
    press(cx, CHIP, MouseButton::Middle);
    assert_eq!(toasts(cx).len(), 1);
    focus(cx, CHIP);
    key(cx, "enter");
    assert_eq!(toasts(cx).len(), 2);
    assert_eq!(path(cx), "/cc-agent/s1");
    press(cx, CHIP, MouseButton::Right);
    assert!(at(cx, HERE).is_none() && at(cx, "menu-row-Open in browser").is_none());
    let (locate, copy) = (at(cx, LOCATE).unwrap(), at(cx, COPY).unwrap());
    assert!(locate.top() < copy.top());
    cx.simulate_click(locate.center(), Modifiers::none());
    settle(cx);
    assert_eq!(toasts(cx).len(), 3);
    press(cx, CHIP, MouseButton::Right);
    let copy = at(cx, COPY).unwrap().center();
    cx.simulate_click(copy, Modifiers::none());
    settle(cx);
    let copied = cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(copied.as_deref(), Some(href));
    // The message's web address keeps its own menu.
    press(cx, "msg-c0-link-0-0", MouseButton::Right);
    assert!(at(cx, LOCATE).is_none());
    assert!(at(cx, "menu-row-Open in browser").is_some());
}

#[gpui::test]
fn a_chip_s_latest_ask_alone_navigates(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let body = "cindy://session/old cindy://session/gone";
    let cx = show(cx, dir.path(), 2400.0, &[(body, None)]);
    let (first, second) = (CHIP, "msg-c0-chip-1");
    // A chip's later ask, here for a window, ends its earlier one.
    cx.update(|window, cx| {
        open("cindy://session/old", first, false, window, cx);
        open("cindy://session/old", first, true, window, cx);
    });
    settle(cx);
    assert_eq!(path(cx), "/cc-agent/s1");
    assert_eq!(toasts(cx), [FAILED]);
    // Another chip's ask ends nothing: the first read to land navigates, and unmounts both.
    cx.update(|window, cx| {
        open("cindy://session/old", first, false, window, cx);
        open("cindy://session/gone", second, false, window, cx);
    });
    settle(cx);
    assert_eq!(path(cx), "/cc-agent/old");
}

/// The chips with an ask on its way.
fn asked(cx: &mut gpui::App) -> Vec<String> {
    let found = super::held_mut(cx).unwrap();
    let mut ids: Vec<String> = found.opening.keys().cloned().collect();
    ids.sort();
    ids
}

#[gpui::test]
fn a_chip_the_stream_stops_drawing_loses_its_ask(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let body = format!("cindy://session/old\n{}", vec!["line"; 30].join("\n"));
    let cx = show(cx, dir.path(), 2400.0, &[(&body, None)]);
    click(cx, "msg-c0-fold", Modifiers::none());
    let ask = |cx: &mut VisualTestContext, ids: &[&str]| {
        cx.update(|window, cx| {
            let found = super::held_mut(cx).unwrap();
            found
                .opening
                .extend(ids.iter().map(|id| (id.to_string(), 1)));
            window.refresh();
        });
        settle(cx);
        cx.update(|_, cx| asked(cx))
    };
    assert_eq!(
        ask(cx, &[CHIP, "msg-c9-chip-0"]),
        [CHIP],
        "a drawn chip keeps its ask"
    );
    // Folded, the chip is drawn no more: what was drawn before does not count.
    click(cx, "msg-c0-fold", Modifiers::none());
    assert_eq!(ask(cx, &[CHIP]), Vec::<String>::new());
}

/// The stored content of `c0` becomes this text, as a history read brings it.
fn rewrite(cx: &mut VisualTestContext, text: &str) {
    let content = serde_json::to_string(text).unwrap();
    cx.update(|_, cx| {
        let crate::localdb::State::Ready(db) = &cx.global::<crate::localdb::LocalDb>().state else {
            panic!("the database is served");
        };
        let sql = "UPDATE messages SET content = ? WHERE client_id = 'c0'";
        drop(db.call(move |conn| conn.execute(sql, [content]).unwrap()));
        // The forced resume read hydrates a row already held.
        let store = crate::chat_store::Store::global(cx);
        store.leave_view("s1");
        store.enter_view("s1", cx);
    });
    settle(cx);
}

#[gpui::test]
fn a_chip_whose_link_changes_loses_its_ask(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = show(cx, dir.path(), 2400.0, &[("cindy://session/old", None)]);
    let ask = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| {
            let found = super::held_mut(cx).unwrap();
            found.opening.insert(CHIP.to_owned(), 1);
        });
    };
    ask(cx);
    // The same target under the legacy scheme at the same place is the same chip.
    rewrite(cx, "xdt-maker://session/old");
    assert_eq!(cx.update(|_, cx| asked(cx)), [CHIP]);
    // Another task, another message or another place is not.
    for text in [
        "cindy://session/other",
        "cindy://session/other?message=m",
        "see cindy://session/other?message=m",
        "a\nsee cindy://session/other?message=m",
    ] {
        rewrite(cx, text);
        assert!(at(cx, CHIP).is_some(), "{text}");
        assert_eq!(cx.update(|_, cx| asked(cx)), Vec::<String>::new(), "{text}");
        ask(cx);
    }
}

#[gpui::test]
fn an_ask_ends_with_its_chip(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let body = format!("cindy://session/old\n{}", vec!["line"; 30].join("\n"));
    let cx = show(cx, dir.path(), 2400.0, &[(&body, None)]);
    click(cx, "msg-c0-fold", Modifiers::none());
    assert!(at(cx, CHIP).is_some(), "unfolded, the link is a chip");
    // The message folds while the read is on its way: the chip is gone, and so is its ask.
    cx.update(|window, cx| {
        open("cindy://session/old", CHIP, false, window, cx);
        crate::session_view::user_row::toggle("msg-c0", cx);
        assert!(asked(cx).is_empty(), "before any frame draws");
    });
    settle(cx);
    assert!(at(cx, CHIP).is_none());
    assert_eq!(path(cx), "/cc-agent/s1");
    // So it is when the task's content leaves, and for a link that names no task.
    click(cx, "msg-c0-fold", Modifiers::none());
    cx.update(|window, cx| {
        open("cindy://session/old", CHIP, false, window, cx);
        router::navigate("/cc-agent/new", cx);
    });
    settle(cx);
    assert_eq!(path(cx), "/cc-agent/new");
    cx.update(|window, cx| open("cindy://session/", CHIP, false, window, cx));
    settle(cx);
    assert_eq!(path(cx), "/cc-agent/new");
}

#[gpui::test]
fn a_folded_message_draws_no_link_chip(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let body = format!("cindy://session/old\n{}", vec!["line"; 30].join("\n"));
    let cx = show(cx, dir.path(), 2400.0, &[(&body, None)]);
    assert!(at(cx, "msg-c0-fold").is_some());
    assert!(at(cx, CHIP).is_none(), "the link stays its words");
}
