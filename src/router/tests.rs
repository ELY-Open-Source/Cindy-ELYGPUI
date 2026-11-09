use gpui::{App, TestAppContext};

use super::{CHAT_KEY, Route, install, navigate, path, route};
use crate::{preferences::Preferences, sessions};

#[test]
fn routes_match_as_router_tsx_declares_them() {
    assert_eq!(route("/"), Route::Root);
    assert_eq!(route("/cc-agent"), Route::Index);
    assert_eq!(route("/cc-agent/"), Route::Index);
    assert_eq!(route("/cc-agent/new"), Route::New);
    assert_eq!(route("/cc-agent/scheduled/"), Route::Scheduled);
    assert_eq!(route("/cc-agent/abc"), Route::Session("abc"));
    assert_eq!(route("/cc-agent/abc/"), Route::Session("abc"));
    for other in [
        "/cc-agent/boot",
        "/cc-agent/files/abc",
        "/cc-agent/orca/abc",
        "/bots",
        "/settings",
    ] {
        assert_eq!(route(other), Route::Other, "{other}");
    }
}

fn start(cx: &mut TestAppContext, last: Option<&str>) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_owned();
    cx.update(|cx| {
        let mut prefs = Preferences::load(&path).unwrap();
        if let Some(last) = last {
            prefs.set(CHAT_KEY, last).unwrap();
        }
        cx.set_global(prefs);
        sessions::install(std::path::PathBuf::new(), None, cx);
        install(cx);
    });
    dir
}

fn here(cx: &mut TestAppContext) -> String {
    cx.update(|cx| path(cx).to_owned())
}

fn load(cx: &mut TestAppContext, tasks: &[(&str, &str, &str)]) {
    cx.update(|cx| sessions::load_for_tests(tasks, cx));
}

const TASKS: [(&str, &str, &str); 4] = [
    ("archived", "archived", "desktop"),
    ("routine", "active", "bot"),
    ("first", "active", "desktop"),
    ("second", "active", "feishu"),
];

#[gpui::test]
fn the_index_waits_for_the_list_then_opens_its_first_task(cx: &mut TestAppContext) {
    let _dir = start(cx, None);
    assert_eq!(here(cx), "/cc-agent", "MainEntryRedirect");
    load(cx, &TASKS);
    assert_eq!(here(cx), "/cc-agent/first");
}

#[test]
fn the_index_reopens_the_last_chat_view() {
    for (last, want) in [
        (
            r#"{"kind":"session","sessionId":"second"}"#,
            "/cc-agent/second",
        ),
        (
            r#"{"kind":"session","sessionId":"archived"}"#,
            "/cc-agent/first",
        ),
        (
            r#"{"kind":"session","sessionId":"gone"}"#,
            "/cc-agent/first",
        ),
        (r#"{"kind":"session","sessionId":""}"#, "/cc-agent/first"),
        (r#"{"kind":"new"}"#, "/cc-agent/new"),
        ("not json", "/cc-agent/first"),
    ] {
        let mut cx = TestAppContext::single();
        let _dir = start(&mut cx, Some(last));
        load(&mut cx, &TASKS);
        assert_eq!(here(&mut cx), want, "{last}");
    }
}

#[gpui::test]
fn an_empty_list_opens_a_draft(cx: &mut TestAppContext) {
    let _dir = start(cx, None);
    load(cx, &[("archived", "archived", "desktop")]);
    assert_eq!(here(cx), "/cc-agent/new");
}

fn saved(cx: &mut TestAppContext) -> Option<String> {
    cx.update(|cx: &mut App| {
        let prefs = cx.global::<Preferences>();
        prefs.text(CHAT_KEY).unwrap().map(str::to_owned)
    })
}

#[gpui::test]
fn chat_views_are_tracked_and_old_links_redirect(cx: &mut TestAppContext) {
    let _dir = start(cx, None);
    load(cx, &TASKS);
    assert_eq!(
        saved(cx).as_deref(),
        Some(r#"{"kind":"session","sessionId":"first"}"#)
    );
    cx.update(|cx| navigate("/cc-agent/scheduled", cx));
    assert_eq!(
        saved(cx).as_deref(),
        Some(r#"{"kind":"session","sessionId":"first"}"#),
        "automations are not a chat view"
    );
    cx.update(|cx| navigate("/cc-agent/new-dialogue", cx));
    assert_eq!(here(cx), "/cc-agent/new");
    assert_eq!(saved(cx).as_deref(), Some(r#"{"kind":"new"}"#));
    cx.update(|cx| navigate("/cc-agent", cx));
    assert_eq!(here(cx), "/cc-agent/new", "the index reopens the draft");
}
