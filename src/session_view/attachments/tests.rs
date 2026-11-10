use std::time::Duration;

use ely_gpui_component::primitives::IconName;
use gpui::{Modifiers, TestAppContext, VisualTestContext, px};

use super::{super::chip, face, most};
use crate::session_view::user_row::tests::{at, open_json};

/// A user row's content: its text and its files, each a name and a path.
fn content(text: &str, files: &[(&str, &str)]) -> String {
    let file = |(name, path): &(&str, &str)| serde_json::json!({ "name": name, "path": path });
    let files: Vec<serde_json::Value> = files.iter().map(file).collect();
    serde_json::json!({ "text": text, "files": files }).to_string()
}

/// `s1` open on these rows, 2400 wide.
fn open<'a>(cx: &'a mut TestAppContext, rows: &[String]) -> &'a mut VisualTestContext {
    let dir = tempfile::tempdir().unwrap();
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    open_json(cx, dir.path(), 2400.0, &rows)
}

#[gpui::test]
fn a_chip_stands_right_over_the_bubble(cx: &mut TestAppContext) {
    let cx = open(
        cx,
        &[content("hello", &[("report.pdf", "/tmp/report.pdf")])],
    );
    let (chip, bubble) = (
        at(cx, "msg-c0-file-0").unwrap(),
        at(cx, "msg-c0-bubble").unwrap(),
    );
    // `h-7`; the border, `px-2.5`, the 14px icon, `gap-1.5` and ten letters at 13px.
    assert_eq!(chip.size, gpui::size(px(120.0), px(28.0)));
    assert_eq!(chip.right(), bubble.right());
    // The column's `gap-2`.
    assert_eq!(chip.bottom() + px(8.0), bubble.top());
}

#[gpui::test]
fn files_alone_draw_no_bubble(cx: &mut TestAppContext) {
    let files = [("a.txt", "/tmp/a.txt"), ("b.txt", "/tmp/b.txt")];
    let cx = open(cx, &[content("", &files), content(" \n", &files)]);
    for key in ["msg-c0", "msg-c1"] {
        let chip = |cx: &mut VisualTestContext, n| at(cx, &format!("{key}-file-{n}")).unwrap();
        let (first, second, column) = (chip(cx, 0), chip(cx, 1), at(cx, "session-messages"));
        assert!(at(cx, &format!("{key}-bubble")).is_none());
        // `gap-1.5` between two chips on one row, the last at the column's right.
        assert_eq!(first.right() + px(6.0), second.left());
        assert_eq!(first.top(), second.top());
        assert_eq!(second.right(), column.unwrap().right());
        // The row is the item: nothing stands under the chips.
        assert_eq!(
            at(cx, &format!("item-{key}")).unwrap().size.height,
            px(28.0)
        );
    }
}

#[gpui::test]
fn a_message_without_files_has_no_row(cx: &mut TestAppContext) {
    let cx = open(cx, &[content("hello", &[]), content("", &[])]);
    assert!(at(cx, "msg-c0-bubble").is_some());
    assert!(at(cx, "msg-c0-files").is_none());
    assert!(at(cx, "item-msg-c1").is_none());
}

#[gpui::test]
fn long_names_stop_at_280_and_wrap_to_the_right(cx: &mut TestAppContext) {
    let long = "n".repeat(300);
    let files = [
        (long.as_str(), "/tmp/l"),
        (&long, "/tmp/l"),
        ("s.rs", "/s.rs"),
    ];
    let cx = open(cx, &[content("x", &files)]);
    let chip = |cx: &mut VisualTestContext, n| at(cx, &format!("msg-c0-file-{n}")).unwrap();
    let (first, second, third) = (chip(cx, 0), chip(cx, 1), chip(cx, 2));
    let row = at(cx, "msg-c0-files").unwrap();
    // Two chips of 280 do not share the column's 488.
    assert_eq!(
        (first.size.width, second.size.width),
        (px(280.0), px(280.0))
    );
    assert_eq!(row.size.width, px(488.0));
    assert_eq!(first.right(), row.right());
    assert_eq!(first.bottom() + px(6.0), second.top());
    // The third stands after the second, and the pair ends at the right.
    assert_eq!(second.right() + px(6.0), third.left());
    assert_eq!(third.right(), row.right());
    assert_eq!(
        row.bottom() + px(8.0),
        at(cx, "msg-c0-bubble").unwrap().top()
    );
}

#[test]
fn a_chip_never_outgrows_its_column() {
    assert_eq!(
        (most(488.0), most(280.0), most(200.0)),
        (280.0, 280.0, 200.0)
    );
}

#[gpui::test]
fn the_name_is_the_tip(cx: &mut TestAppContext) {
    let cx = open(
        cx,
        &[content("x", &[("report.pdf", "/tmp/r"), ("", "/tmp/e")])],
    );
    let named = at(cx, "msg-c0-file-0").unwrap();
    cx.simulate_mouse_move(named.center(), None, Modifiers::none());
    cx.executor().advance_clock(Duration::from_millis(499));
    cx.run_until_parked();
    assert!(at(cx, "tip").is_none());
    cx.executor().advance_clock(Duration::from_millis(1));
    cx.run_until_parked();
    // The name at 12px in the tip's `px-2`.
    let tip = at(cx, "tip").unwrap();
    assert!(
        tip.size.width > px(60.0) && tip.size.width < px(120.0),
        "{tip:?}"
    );
    // `title=""` shows nothing.
    let unnamed = at(cx, "msg-c0-file-1").unwrap();
    cx.simulate_mouse_move(unnamed.center(), None, Modifiers::none());
    cx.executor().advance_clock(Duration::from_millis(600));
    cx.run_until_parked();
    assert!(at(cx, "tip").is_none());
}

#[gpui::test]
fn white_space_in_a_name_collapses(cx: &mut TestAppContext) {
    let files = [("a  b\n\tc", "/tmp/a"), ("a b c", "/tmp/b")];
    let cx = open(cx, &[content("x", &files)]);
    let (spaced, plain) = (at(cx, "msg-c0-file-0"), at(cx, "msg-c0-file-1"));
    assert_eq!(spaced.unwrap().size, plain.unwrap().size);
}

#[test]
fn a_file_the_chat_only_saves_draws_download() {
    let download = IconName::Download.svg();
    assert_eq!(face("setup.exe", "/tmp/setup.exe"), (true, download));
    assert_eq!(face("notes.txt", "/tmp/staged.bin"), (true, download));
    let pdf = face("report.pdf", "/tmp/report.pdf");
    assert_eq!(pdf, (false, chip::file_icon("report.pdf")));
    assert_ne!(pdf.1, download);
}

#[gpui::test]
fn the_hook_form_draws_no_chip_yet(cx: &mut TestAppContext) {
    let hook = r#"{"hookSource":{"im":"feishu"}}"#;
    let dir = tempfile::tempdir().unwrap();
    let row = content("hello", &[("a.txt", "/tmp/a.txt")]);
    let rows = [(row.as_str(), Some(hook)), (&row, None)];
    let cx = crate::session_view::user_row::tests::open(cx, dir.path(), 2400.0, &rows);
    assert!(at(cx, "msg-c1-files").is_some());
    assert!(at(cx, "msg-c0-files").is_none());
}
