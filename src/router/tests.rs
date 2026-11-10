use gpui::{App, TestAppContext};

use super::{CHAT_KEY, Route, install, location, navigate, path, route};
use crate::{preferences::Preferences, sessions};

#[test]
fn routes_match_as_router_tsx_declares_them() {
    assert_eq!(route("/"), Route::Root);
    assert_eq!(route("/cc-agent"), Route::Index);
    assert_eq!(route("/cc-agent/"), Route::Index);
    assert_eq!(route("/cc-agent/new"), Route::New);
    assert_eq!(route("/cc-agent/scheduled/"), Route::Scheduled);
    assert_eq!(route("/cc-agent/abc"), Route::Session("abc".into()));
    assert_eq!(route("/cc-agent/abc/"), Route::Session("abc".into()));
    // `decodePath`, then the param's `%2F` back to a slash.
    for (path, id) in [
        ("/cc-agent/a%20b%2Fc%3Fd", "a b/c?d"),
        ("/cc-agent/%E4%BB%BB", "任"),
        ("/cc-agent/a%252Fb", "a/b"),
        ("/cc-agent/a%FF%2Fb", "a%FF/b"),
        ("/cc-agent/50%", "50%"),
    ] {
        assert_eq!(route(path), Route::Session(id.into()), "{path}");
    }
    assert_eq!(route("/cc-agent/%6Eew"), Route::New);
    assert_eq!(route("/cc-agent/a%2Fb/c"), Route::Other);
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

#[gpui::test]
fn a_search_rides_the_location_and_the_routes_match_the_pathname(cx: &mut TestAppContext) {
    let _dir = start(cx, None);
    load(cx, &TASKS);
    let go = |cx: &mut TestAppContext, to: &str| {
        cx.update(|cx| {
            navigate(to, cx);
            (location(cx).to_owned(), path(cx).to_owned())
        })
    };
    let scheduled = "/cc-agent/scheduled?focus=job%201";
    assert_eq!(
        go(cx, scheduled),
        (scheduled.into(), "/cc-agent/scheduled".into())
    );
    // Another search on the same pathname is another location.
    let other = "/cc-agent/scheduled?focus=b";
    assert_eq!(go(cx, other).0, other);
    // The redirects and the last chat view read the pathname.
    assert_eq!(go(cx, "/?from=link").0, "/cc-agent/first");
    // The view takes the decoded id; the last chat view keeps `matchPath`'s on the raw pathname.
    for (to, id, kept) in [
        ("/cc-agent/job%201", Some("job 1"), "job%201"),
        ("/cc-agent/a%20b%2Fc%3Fd", Some("a b/c?d"), "a%20b/c%3Fd"),
        ("/cc-agent/a%252Fb", Some("a/b"), "a%252Fb"),
        ("/cc-agent/a%FF%2Fb", Some("a%FF/b"), "a%FF/b"),
        ("/cc-agent/%6Eew", None, "%6Eew"),
    ] {
        go(cx, to);
        let found = cx.update(|cx| {
            let saved = cx.global::<Preferences>().text(CHAT_KEY).unwrap();
            (super::active_session(cx), saved.map(str::to_owned))
        });
        let last = serde_json::json!({ "kind": "session", "sessionId": kept }).to_string();
        assert_eq!(found, (id.map(str::to_owned), Some(last)), "{to}");
    }
    // The index looks the kept id up among the tasks: an escaped one is no task's.
    assert_eq!(go(cx, "/cc-agent").0, "/cc-agent/first");
    go(cx, "/cc-agent/second?worker=w");
    let saved = cx.update(|cx| {
        cx.global::<Preferences>()
            .text(CHAT_KEY)
            .unwrap()
            .map(str::to_owned)
    });
    assert_eq!(
        saved.as_deref(),
        Some(r#"{"kind":"session","sessionId":"second"}"#)
    );
    assert_eq!(
        cx.update(|cx| super::active_session(cx)).as_deref(),
        Some("second")
    );
}
