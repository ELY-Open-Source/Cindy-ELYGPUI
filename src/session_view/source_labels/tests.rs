use std::time::Duration;

use gpui::{Modifiers, TestAppContext, VisualTestContext, px};

use crate::{
    router,
    session_view::{
        links::tests::{click, focus, key},
        user_row::tests::{at, open, open_meta},
    },
};

const SOURCES: &str = r#"{"origin":{"kind":"scheduler","scheduleId":"job 1","scheduleName":"Digest"},"sourceDevice":{"deviceId":"d1","platform":"mobile","name":"Pixel"},"goalObjective":{"updated":true}}"#;
const HOOK: &str =
    r#"{"origin":{"kind":"scheduler","scheduleId":"hook:x","scheduleName":"Hook · Team"}}"#;

fn location(cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| router::location(cx).to_owned())
}

#[gpui::test]
fn the_labels_and_the_goal_badge_stand_over_the_bubble(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let rows = [("hello", Some(SOURCES)), ("plain", None)];
    let cx = open(cx, dir.path(), 2400.0, &rows);
    let column = at(cx, "session-messages").unwrap();
    let row = at(cx, "msg-c0-sources").unwrap();
    assert_eq!(row.top(), column.top() + px(28.0));
    assert_eq!(row.right(), column.right(), "justify-end");
    // 11px at the inherited 1.5.
    assert_eq!(row.size.height, px(16.5));
    let sender = at(cx, "msg-c0-source-scheduler").unwrap();
    let device = at(cx, "msg-c0-source-device").unwrap();
    assert_eq!(device.right(), row.right());
    assert_eq!(device.left(), sender.right() + px(12.0), "gap-x-3");
    assert_eq!(
        sender.left(),
        row.left(),
        "the row is as wide as its labels"
    );
    let goal = at(cx, "msg-c0-goal").unwrap();
    assert_eq!(goal.top(), row.bottom() + px(8.0), "gap-2");
    assert_eq!(goal.right(), column.right());
    assert_eq!(goal.size.height, px(20.5), "py-0.5");
    let bubble = at(cx, "msg-c0-bubble").unwrap();
    assert_eq!(bubble.top(), goal.bottom() + px(8.0));
    for part in ["sources", "goal", "source-scheduler"] {
        assert!(at(cx, &format!("msg-c1-{part}")).is_none(), "{part}");
    }
}

#[gpui::test]
fn a_set_goal_and_an_updated_one_read_apart(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let rows = [
        ("a", Some(r#"{"goalObjective":{"updated":false}}"#)),
        ("b", Some(r#"{"goalObjective":{"updated":true}}"#)),
        ("c", Some(r#"{"goalObjective":{}}"#)),
    ];
    let cx = open(cx, dir.path(), 2400.0, &rows);
    let width =
        |cx: &mut VisualTestContext, n| at(cx, &format!("msg-c{n}-goal")).unwrap().size.width;
    let (set, updated, bare) = (width(cx, 0), width(cx, 1), width(cx, 2));
    assert!(updated > set + px(30.0), "Goal updated against Goal");
    assert_eq!(bare, set);
}

#[gpui::test]
fn a_label_alone_draws_with_no_bubble(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), 2400.0, &[(" ", Some(SOURCES))]);
    assert!(at(cx, "msg-c0-sources").is_some());
    assert!(at(cx, "msg-c0-goal").is_some());
    assert!(at(cx, "msg-c0-bubble").is_none());
}

#[gpui::test]
fn a_click_on_a_label_opens_its_route(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), 2400.0, &[("hello", Some(SOURCES))]);
    click(cx, "msg-c0-source-scheduler", Modifiers::none());
    assert_eq!(location(cx), "/cc-agent/scheduled?focus=job%201");
    assert_eq!(
        cx.update(|_, cx| router::path(cx).to_owned()),
        "/cc-agent/scheduled"
    );
}

#[gpui::test]
fn the_device_label_opens_my_devices(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), 2400.0, &[("hello", Some(SOURCES))]);
    click(cx, "msg-c0-source-device", Modifiers::none());
    let devices = "/settings?tab=remote-control&section=devices&device=d1";
    assert_eq!(location(cx), devices);
}

#[gpui::test]
fn a_focused_label_opens_on_a_key(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), 2400.0, &[("hello", Some(SOURCES))]);
    focus(cx, "msg-c0-source-scheduler");
    assert_eq!(location(cx), "/cc-agent/s1");
    key(cx, "enter");
    assert_eq!(location(cx), "/cc-agent/scheduled?focus=job%201");
}

#[gpui::test]
fn a_channel_label_is_plain_words(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), 2400.0, &[("hello", Some(HOOK))]);
    click(cx, "msg-c0-source-hook", Modifiers::none());
    key(cx, "enter");
    assert_eq!(location(cx), "/cc-agent/s1");
    // No tooltip either.
    let label = at(cx, "msg-c0-source-hook").unwrap();
    cx.simulate_mouse_move(label.center(), None, Modifiers::none());
    cx.executor().advance_clock(Duration::from_millis(600));
    cx.run_until_parked();
    assert!(at(cx, "tip").is_none());
}

#[gpui::test]
fn the_tooltip_names_the_way_and_the_ids(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), 2400.0, &[("hello", Some(SOURCES))]);
    let label = at(cx, "msg-c0-source-scheduler").unwrap();
    cx.simulate_mouse_move(label.center(), None, Modifiers::none());
    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    // Two lines of 13px at 1.375 in the tip's `py-1.5`.
    let tip = at(cx, "tip").unwrap();
    assert!(
        tip.size.height > px(44.0) && tip.size.height < px(52.0),
        "{tip:?}"
    );
}

#[gpui::test]
fn a_task_label_reads_the_held_title_and_a_teammate_its_mark(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let from = |id: &str, more: &str| {
        format!(
            r#"{{"origin":{{"kind":"session","senderSessionId":"{id}","senderSessionTitle":"A stored title of some length"{more}}}}}"#
        )
    };
    let (held, gone) = (from("s1", ""), from("s9", ""));
    let mate = from("s9", r#","senderBotId":"b1","senderBotName":"ada""#);
    let rows = [
        ("a", Some(held.as_str())),
        ("b", Some(gone.as_str())),
        ("c", Some(mate.as_str())),
    ];
    let cx = open(cx, dir.path(), 2400.0, &rows);
    let held = at(cx, "msg-c0-source-session").unwrap();
    let gone = at(cx, "msg-c1-source-session").unwrap();
    assert!(
        held.size.width + px(100.0) < gone.size.width,
        "the task list names s1 `s1`"
    );
    let mate = at(cx, "msg-c2-source-bot").unwrap();
    assert_eq!(mate.size.height, px(16.5), "the 14px mark fits the line");
    click(cx, "msg-c2-source-bot", Modifiers::none());
    assert_eq!(location(cx), "/cc-agent/s9");
}

#[gpui::test]
fn labels_too_wide_for_one_line_wrap_and_stand_right(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let long = "a name of some length ".repeat(2);
    let meta = format!(
        r#"{{"origin":{{"kind":"scheduler","scheduleName":"{long}"}},"sourceDevice":{{"deviceId":"d1","platform":"desktop","name":"{long}and more"}}}}"#
    );
    let cx = open(cx, dir.path(), 2400.0, &[("hello", Some(meta.as_str()))]);
    let row = at(cx, "msg-c0-sources").unwrap();
    assert!(row.size.width <= px(488.0), "the column's `max-w-[488px]`");
    let sender = at(cx, "msg-c0-source-scheduler").unwrap();
    let device = at(cx, "msg-c0-source-device").unwrap();
    assert_eq!(device.top(), sender.bottom() + px(2.0), "gap-y-0.5");
    assert!(
        sender.size.width < device.size.width,
        "{sender:?} {device:?}"
    );
    assert_eq!((sender.right(), device.right()), (row.right(), row.right()));
}

#[gpui::test]
fn the_labels_stand_over_the_attachments(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let content = r#"{"text":"hello","images":[{"url":"xdt-image://s1/gone.png","mimeType":"image/png","originalName":"a.png"}],"files":[{"name":"a.txt","path":"/tmp/a.txt"}]}"#;
    let cx = open_meta(cx, dir.path(), 2400.0, &[(content, Some(SOURCES))]);
    let goal = at(cx, "msg-c0-goal").unwrap();
    let image = at(cx, "msg-c0-image-0-failed").unwrap();
    assert_eq!(image.top(), goal.bottom() + px(8.0));
    let file = at(cx, "msg-c0-files").unwrap();
    assert_eq!(file.top(), image.bottom() + px(8.0));
}

#[gpui::test]
fn a_name_with_line_feeds_draws_on_one_line(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let meta = r#"{"origin":{"kind":"scheduler","scheduleName":"two\nlines\r\n\tof  it"}}"#;
    let flat = r#"{"origin":{"kind":"scheduler","scheduleName":"two lines of it"}}"#;
    let cx = open(
        cx,
        dir.path(),
        2400.0,
        &[("a", Some(meta)), ("b", Some(flat))],
    );
    let label = at(cx, "msg-c0-source-scheduler").unwrap();
    assert_eq!(label.size.height, px(16.5));
    assert_eq!(label.size, at(cx, "msg-c1-source-scheduler").unwrap().size);
}

/// Review 9.5p r1: the route's parameter decodes to the task's own id.
#[gpui::test]
fn a_task_label_opens_a_task_whose_id_needs_escapes(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let meta = r#"{"origin":{"kind":"session","senderSessionId":"a b/c?d"}}"#;
    let cx = open(cx, dir.path(), 2400.0, &[("hello", Some(meta))]);
    cx.update(|_, cx| {
        let tasks = [
            ("s1", "active", "desktop"),
            ("a b/c?d", "active", "desktop"),
        ];
        crate::sessions::load_for_tests(&tasks, cx);
    });
    click(cx, "msg-c0-source-session", Modifiers::none());
    assert_eq!(location(cx), "/cc-agent/a%20b%2Fc%3Fd");
    let opened = cx.update(|_, cx| (router::active_session(cx), crate::session_view::gate(cx)));
    let open = crate::session_view::Gate::Open;
    assert_eq!(opened, (Some("a b/c?d".into()), Some(open)));
}

/// Review 9.5p r2: a literal percent escape in an id comes back through the route.
#[gpui::test]
fn a_task_label_opens_a_task_whose_id_holds_an_escape(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let meta = r#"{"origin":{"kind":"session","senderSessionId":"50%25"}}"#;
    let cx = open(cx, dir.path(), 2400.0, &[("hello", Some(meta))]);
    cx.update(|_, cx| {
        let tasks = [("s1", "active", "desktop"), ("50%25", "active", "desktop")];
        crate::sessions::load_for_tests(&tasks, cx);
    });
    click(cx, "msg-c0-source-session", Modifiers::none());
    assert_eq!(location(cx), "/cc-agent/50%2525");
    let opened = cx.update(|_, cx| (router::active_session(cx), crate::session_view::gate(cx)));
    let open = crate::session_view::Gate::Open;
    assert_eq!(opened, (Some("50%25".into()), Some(open)));
}
