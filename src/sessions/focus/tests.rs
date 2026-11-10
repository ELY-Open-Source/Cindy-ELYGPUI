//! A project link's focus: the key it finds, the fold it opens, the scroll and the toast.

use std::time::Duration;

use gpui::{TestAppContext, VisualTestContext, px, size};

use super::{representative, request, settle};
use crate::{
    router,
    sessions::{
        Sessions, fold,
        reveal::Reveal,
        view_tests::{open, task},
    },
    toast::{Toasts, Variant},
};

const FOLD: &str = "project:local:/work/app";
const HEAD: &str = "project:local:/work/app-header";

fn toasts(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|window, cx| {
        let items = cx
            .global::<Toasts>()
            .items(window.window_handle().window_id());
        items.iter().map(|item| item.message.to_string()).collect()
    })
}

fn ask(cx: &mut VisualTestContext, dir: &str) {
    cx.update(|window, cx| request(dir, window, cx));
    cx.run_until_parked();
}

/// The list's offset after `ms` more and one frame.
fn frame(cx: &mut VisualTestContext, ms: u64) -> f32 {
    cx.executor().advance_clock(Duration::from_millis(ms));
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();
    cx.update(|_, cx| f32::from(cx.global::<Reveal>().handle.offset().y))
}

fn folded(cx: &mut VisualTestContext) -> bool {
    cx.update(|_, cx| cx.global::<Sessions>().collapsed.contains(FOLD))
}

#[test]
fn a_link_s_folder_finds_the_key_the_sidebar_draws() {
    let drawn = vec!["local:d:/école/project-a".to_owned()];
    // Cindy's own case: Windows folders compare without case, on Windows.
    assert_eq!(
        representative(&drawn, "local:D:/ÉCOLE/PROJECT-A", "win32").as_deref(),
        Some("local:d:/école/project-a")
    );
    assert_eq!(
        representative(&drawn, "local:D:/ÉCOLE/PROJECT-A", "linux"),
        None
    );
    let drawn = vec![
        "remote:h:/work/app".to_owned(),
        "local:/work/app".to_owned(),
    ];
    // A bare folder is a local key, slashes and the trailing one normalized.
    assert_eq!(
        representative(&drawn, "/work/app/", "linux").as_deref(),
        Some("local:/work/app")
    );
    assert_eq!(representative(&drawn, "/work", "linux"), None);
    // The drawn key compares folded too, and comes back as drawn.
    let drawn = vec!["local:D:/Work/App".to_owned()];
    assert_eq!(
        representative(&drawn, "d:/work/app", "win32").as_deref(),
        Some("local:D:/Work/App")
    );
    assert_eq!(representative(&drawn, "  ", "linux"), None);
}

#[gpui::test]
fn a_folded_project_unfolds(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    cx.update(|_, cx| {
        cx.global_mut::<Sessions>().collapsed.insert(FOLD.into());
        cx.refresh_windows();
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("task-a0").is_none());
    ask(cx, "/work/app");
    assert!(!folded(cx));
    let open = |cx: &mut VisualTestContext| cx.update(|_, cx| fold::openness(FOLD, false, cx));
    assert_eq!(open(cx), 0.0, "it unfolds from shut");
    frame(cx, 1000);
    assert_eq!(open(cx), 1.0);
    assert!(cx.debug_bounds("task-a0").is_some());
    assert_eq!(toasts(cx), Vec::<String>::new());
    // An open project stays open, and an empty folder asks nothing.
    ask(cx, "/work/app");
    assert_eq!(open(cx), 1.0, "no fold starts over");
    ask(cx, "");
    assert!(!folded(cx));
    assert_eq!(toasts(cx), Vec::<String>::new());
}

#[gpui::test]
fn a_project_the_sidebar_lacks_says_so(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    ask(cx, "/work/other");
    let warning = "Project not found — it may have been archived or doesn't exist on this device";
    assert_eq!(toasts(cx), [warning]);
    let variant = cx.update(|window, cx| {
        let id = window.window_handle().window_id();
        cx.global::<Toasts>().items(id)[0].variant
    });
    assert_eq!(variant, Variant::Warning);
    // The ask is consumed: a reload says nothing more.
    cx.update(|_, cx| settle(cx));
    cx.run_until_parked();
    assert_eq!(toasts(cx).len(), 1);
}

#[gpui::test]
fn the_ask_waits_out_the_first_load(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    let db = crate::sessions::tests::db();
    crate::sessions::tests::add(&db, "a0", "desktop", "active", 1);
    db.execute_batch(
        "UPDATE sessions SET working_dir = '/work/app', list_message_count = 1;
         CREATE TABLE recent_workdirs (path text PRIMARY KEY, last_used_at integer NOT NULL);
         CREATE TABLE project_aliases (project_key text PRIMARY KEY, alias text NOT NULL,
           updated_at integer NOT NULL);",
    )
    .unwrap();
    // The database opens, its read starts, and the ask finds nothing loaded yet.
    cx.update(|_, cx| {
        let store = cx.global_mut::<Sessions>();
        store.collapsed.insert(FOLD.into());
        store.loaded = None;
        crate::localdb::serve(db, cx);
    });
    cx.update(|window, cx| {
        assert!(cx.global::<Sessions>().loading);
        request("/work/app", window, cx);
    });
    assert!(folded(cx), "nothing is known yet");
    cx.run_until_parked();
    assert!(!folded(cx), "the read's end consumed the ask");
    assert_eq!(toasts(cx), Vec::<String>::new());
    // With nothing loaded and no read on its way, the ask finds no project.
    cx.update(|window, cx| {
        cx.global_mut::<Sessions>().loaded = None;
        request("/work/app", window, cx);
    });
    cx.run_until_parked();
    assert_eq!(toasts(cx).len(), 1);
}

#[gpui::test]
fn a_project_below_the_end_scrolls_into_view(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    cx.update(|_, cx| {
        let now = jiff::Timestamp::now().as_millisecond();
        let store = cx.global_mut::<Sessions>();
        let loaded = store.loaded.as_mut().unwrap();
        // Twelve newer projects stand above the folded one.
        for n in 0..12 {
            let dir = format!("/work/p{n}");
            loaded
                .sessions
                .push(task(&format!("p{n}"), Some(&dir), false, now - n));
        }
        store.collapsed.insert(FOLD.into());
        // No task of the folded project is the active one, whose row would scroll itself.
        router::navigate("/cc-agent/d0", cx);
        cx.refresh_windows();
    });
    cx.simulate_resize(size(px(400.0), px(274.0)));
    cx.run_until_parked();
    let list = cx.debug_bounds("main-list").unwrap();
    assert!(cx.debug_bounds(HEAD).unwrap().top() >= list.bottom());
    ask(cx, "/work/app");
    let first = frame(cx, 16);
    let done = frame(cx, 2000);
    assert!(
        done < 0.0 && first > done,
        "on its way: {first} then {done}"
    );
    let head = cx.debug_bounds(HEAD).unwrap();
    assert!(head.top() >= list.top() && head.bottom() <= list.bottom() + px(0.5));
}
