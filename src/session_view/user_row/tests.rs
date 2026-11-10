use gpui::{Bounds, Modifiers, Pixels, TestAppContext, VisualTestContext, px};
use rusqlite::Connection;

use crate::{layout_root::screen_tests, sessions};

/// A user row whose `content` column is this JSON.
fn put(db: &Connection, n: i64, content: &str, meta: Option<&str>) {
    db.execute(
        "INSERT INTO messages (id, client_id, session_id, role, content, agent_meta, created_at)
         VALUES (?, ?, 's1', 'user', ?, ?, ?)",
        (
            n.to_string(),
            format!("c{n}"),
            content,
            meta,
            1_700_000_000_000 + n,
        ),
    )
    .unwrap();
}

pub(in crate::session_view) fn settle(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

/// The content area `width` wide with `s1` open on these user messages, `c0` onwards.
pub(in crate::session_view) fn open<'a>(
    cx: &'a mut TestAppContext,
    dir: &std::path::Path,
    width: f32,
    rows: &[(&str, Option<&str>)],
) -> &'a mut VisualTestContext {
    let db = super::super::tests::db();
    for (n, (content, meta)) in rows.iter().enumerate() {
        put(
            &db,
            n as i64,
            &serde_json::to_string(content).unwrap(),
            *meta,
        );
    }
    serve(cx, dir, width, db)
}

/// As `open`, each row's `content` column this JSON as it stands.
pub(in crate::session_view) fn open_json<'a>(
    cx: &'a mut TestAppContext,
    dir: &std::path::Path,
    width: f32,
    rows: &[&str],
) -> &'a mut VisualTestContext {
    let db = super::super::tests::db();
    for (n, content) in rows.iter().enumerate() {
        put(&db, n as i64, content, None);
    }
    serve(cx, dir, width, db)
}

fn serve<'a>(
    cx: &'a mut TestAppContext,
    dir: &std::path::Path,
    width: f32,
    db: Connection,
) -> &'a mut VisualTestContext {
    let cx = screen_tests::open(cx, dir, screen_tests::tree(0.6, 0.4), width);
    cx.update(|_, cx| {
        crate::localdb::serve(db, cx);
        sessions::load_for_tests(&[("s1", "active", "desktop")], cx);
    });
    settle(cx);
    cx
}

pub(in crate::session_view) fn at(
    cx: &mut VisualTestContext,
    selector: &str,
) -> Option<Bounds<Pixels>> {
    cx.debug_bounds(Box::leak(selector.to_owned().into_boxed_str()))
}

pub(in crate::session_view) fn lines(n: usize) -> String {
    vec!["line"; n].join("\n")
}

const AUTOMATION: &str = r#"{"origin":{"kind":"scheduler","scheduleId":"job"}}"#;

#[gpui::test]
fn a_short_message_sits_right_in_its_bubble(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(
        cx,
        dir.path(),
        2400.0,
        &[
            ("hello", None),
            ("a\r\nb\n", None),
            ("a\rb", None),
            ("ab", None),
        ],
    );
    let column = at(cx, "session-messages").unwrap();
    let bubble = at(cx, "msg-c0-bubble").unwrap();
    assert_eq!(bubble.right(), column.right(), "justify-end");
    assert_eq!(bubble.top(), column.top() + px(28.0), "pt-7");
    let text = at(cx, "msg-c0-text").unwrap();
    assert_eq!(text.size.height, px(24.0), "15px at 1.6");
    assert_eq!(
        bubble.size.height,
        px(24.0 + 24.0 + 2.0),
        "py-3 and the border"
    );
    assert_eq!(
        bubble.size.width,
        text.size.width + px(34.0),
        "px-4 and the border"
    );
    assert!(at(cx, "msg-c0-fold").is_none());
    // CR draws nothing and the last line feed ends its line, as Blink lays `pre-wrap` out.
    let second = at(cx, "msg-c1-text").unwrap();
    assert_eq!(second.size.height, px(48.0));
    let width = |cx: &mut VisualTestContext, key| at(cx, key).unwrap().size.width;
    assert_eq!(width(cx, "msg-c2-text"), width(cx, "msg-c3-text"));
    let row = at(cx, "item-msg-c1").unwrap();
    assert_eq!(row.top(), bubble.bottom() + px(14.0), "gap-3.5");
}

fn text_lines(cx: &mut VisualTestContext, key: &str) -> f32 {
    at(cx, &format!("{key}-text")).unwrap().size.height / px(24.0)
}

#[gpui::test]
fn a_long_message_folds_to_ten_lines_and_opens(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(
        cx,
        dir.path(),
        2400.0,
        &[(&lines(14), None), (&lines(15), None)],
    );
    assert_eq!(text_lines(cx, "msg-c0"), 14.0, "14 lines stay");
    assert!(at(cx, "msg-c0-fold").is_none());
    assert_eq!(text_lines(cx, "msg-c1"), 10.0, "line-clamp-10");
    let fold = at(cx, "msg-c1-fold").unwrap();
    let (text, bubble) = (
        at(cx, "msg-c1-text").unwrap(),
        at(cx, "msg-c1-bubble").unwrap(),
    );
    assert_eq!(fold.top(), text.bottom() + px(8.0), "mt-2");
    assert_eq!(fold.left(), text.left());
    assert_eq!(fold.size.height, px(24.0), "leading-5 and py-0.5");
    assert_eq!(bubble.bottom(), fold.bottom() + px(13.0));
    cx.simulate_click(fold.center(), Modifiers::none());
    settle(cx);
    assert_eq!(text_lines(cx, "msg-c1"), 15.0);
    // The button stays to fold it again.
    let fold = at(cx, "msg-c1-fold").unwrap();
    cx.simulate_click(fold.center(), Modifiers::none());
    settle(cx);
    assert_eq!(text_lines(cx, "msg-c1"), 10.0);
}

#[gpui::test]
fn an_automation_message_folds_past_four_lines_to_three(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let hook = r#"{"origin":{"kind":"scheduler","scheduleId":"hook:x"}}"#;
    let rows = [
        (lines(4), Some(AUTOMATION)),
        (lines(5), Some(AUTOMATION)),
        // A hook channel's message is a person's, and so is another task's.
        (lines(5), Some(hook)),
        (
            lines(5),
            Some(r#"{"origin":{"kind":"session","senderSessionId":"s2"}}"#),
        ),
    ];
    let rows: Vec<_> = rows
        .iter()
        .map(|(text, meta)| (text.as_str(), *meta))
        .collect();
    let cx = open(cx, dir.path(), 2400.0, &rows);
    assert_eq!(text_lines(cx, "msg-c0"), 4.0);
    assert!(at(cx, "msg-c0-fold").is_none());
    assert_eq!(text_lines(cx, "msg-c1"), 3.0, "line-clamp-3");
    assert!(at(cx, "msg-c1-fold").is_some());
    for key in ["msg-c2", "msg-c3"] {
        assert_eq!(text_lines(cx, key), 5.0, "{key}");
        assert!(at(cx, &format!("{key}-fold")).is_none(), "{key}");
    }
}

/// Rows the first page holds.
const COUNT: usize = 20;

/// Each text folds exactly when its own bubble wraps it past 14 lines.
fn folds_as_drawn(
    cx: &mut TestAppContext,
    (width, line): (f32, f32),
    units: impl Fn(usize) -> usize,
) {
    let lines = |cx: &mut VisualTestContext, key: &str| text_lines(cx, key) * 24.0 / line;
    let dir = tempfile::tempdir().unwrap();
    let texts: Vec<String> = (0..COUNT).map(|n| "a ".repeat(units(n))).collect();
    let rows: Vec<_> = texts.iter().map(|text| (text.as_str(), None)).collect();
    let cx = open(cx, dir.path(), width, &rows);
    let mut folded = 0;
    for n in 0..COUNT {
        let key = format!("msg-c{n}");
        let fold = at(cx, &format!("{key}-fold")).is_some();
        if fold {
            assert_eq!(lines(cx, &key), 10.0, "{key}");
            cx.update(|_, cx| super::toggle(&key, cx));
            settle(cx);
        }
        assert_eq!(lines(cx, &key) > 14.0, fold, "{key}");
        folded += usize::from(fold);
    }
    assert!((1..COUNT).contains(&folded), "{folded} of them fold");
}

#[gpui::test]
fn the_fold_counts_wrapped_lines_at_the_bubble_width(cx: &mut TestAppContext) {
    folds_as_drawn(cx, (2400.0, 24.0), |n| 300 + 20 * n);
}

#[gpui::test]
fn a_narrow_bubble_folds_sooner(cx: &mut TestAppContext) {
    // A compact column: 14px lines of 22.4, snapped to 22.5.
    folds_as_drawn(cx, (600.0, 22.5), |n| 100 + 15 * n);
}

#[gpui::test]
fn a_compact_column_draws_a_pixel_smaller(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), 900.0, &[(&lines(15), None)]);
    let column = at(cx, "session-messages").unwrap();
    assert!(column.size.width < px(660.0));
    // Ten lines of 14px at 1.6, each snapped to the device pixel.
    assert_eq!(at(cx, "msg-c0-text").unwrap().size.height, px(225.0));
    let fold = at(cx, "msg-c0-fold").unwrap();
    assert_eq!(fold.size.height, px(24.0));
}

#[gpui::test]
fn a_narrow_column_bounds_the_bubble(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), 600.0, &[(&"word ".repeat(200), None)]);
    let column = at(cx, "session-messages").unwrap();
    assert!(column.size.width < px(488.0), "{:?}", column.size.width);
    let bubble = at(cx, "msg-c0-bubble").unwrap();
    assert_eq!(bubble.size.width, column.size.width);
    assert_eq!(bubble.left(), column.left());
}

#[gpui::test]
fn other_forms_and_empty_text_draw_no_bubble(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let rows = [
        (r#"{"orcaSource":"lead","content":"do it"}"#, None),
        ("from a channel", Some(r#"{"hookSource":{"im":"feishu"}}"#)),
        (" \n ", None),
        ("shown", None),
    ];
    let cx = open(cx, dir.path(), 2400.0, &rows);
    for n in 0..3 {
        assert!(at(cx, &format!("item-msg-c{n}")).is_none(), "c{n}");
    }
    let column = at(cx, "session-messages").unwrap();
    let row = at(cx, "item-msg-c3").unwrap();
    assert_eq!(
        row.top(),
        column.top() + px(28.0),
        "no gap for a row not drawn"
    );
}

#[gpui::test]
fn the_bubble_shows_the_decoded_text(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let db = super::super::tests::db();
    let quoted = "> <!-- cindy-composer-quote -->\n> first quote\n\nfirst response";
    let content = serde_json::json!({ "text": quoted, "quotesEncoded": true }).to_string();
    put(&db, 0, &content, None);
    // A request another task forwarded, and the same text typed by hand.
    let envelope = "[来源] 用户原话由宿主从已接收的输入原样保留。\n用户原话：\"只检查\"\nagent补充：\"第一段\"";
    let envelope = serde_json::to_string(envelope).unwrap();
    put(
        &db,
        1,
        &envelope,
        Some(r#"{"origin":{"kind":"session","senderSessionId":"s2"}}"#),
    );
    put(&db, 2, &envelope, None);
    let cx = serve(cx, dir.path(), 2400.0, db);
    assert_eq!(
        text_lines(cx, "msg-c0"),
        1.0,
        "the quote's lines are no text"
    );
    assert_eq!(text_lines(cx, "msg-c1"), 9.0, "the envelope decoded");
    assert_eq!(text_lines(cx, "msg-c2"), 3.0);
}

/// Blink's line counts at any font (`logs/review/9.5e-r1-blink-spaces.log`).
#[gpui::test]
fn white_space_at_a_wrap_hangs(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let s = " ".repeat(1000);
    let cases = [
        (format!("a{s}"), 1.0),
        (format!("a{s}b"), 2.0),
        (format!("{s}b"), 2.0),
        (format!("a{s}b{s}c"), 3.0),
        (format!("a{}", "\t".repeat(400)), 1.0),
        (format!("a{}b", "\t".repeat(400)), 2.0),
        (format!("a{}b", "\u{3000}".repeat(300)), 2.0),
        (format!("a{s}\nb{s}\nc"), 3.0),
    ];
    let rows: Vec<_> = cases
        .iter()
        .map(|(text, _)| (text.as_str(), None))
        .collect();
    let cx = open(cx, dir.path(), 2400.0, &rows);
    for (n, (_, lines)) in cases.iter().enumerate() {
        assert_eq!(text_lines(cx, &format!("msg-c{n}")), *lines, "c{n}");
    }
}

#[gpui::test]
fn other_white_space_keeps_its_room(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let nbsp = format!("a{}b", "\u{a0}".repeat(600));
    let lines15 = format!("{}{}", "x  \n".repeat(14), " ".repeat(1000));
    let rows = [
        ("hi", None),
        ("hi          ", None),
        (nbsp.as_str(), None),
        (lines15.as_str(), None),
    ];
    let cx = open(cx, dir.path(), 2400.0, &rows);
    let width = |cx: &mut VisualTestContext, key| at(cx, key).unwrap().size.width;
    assert!(
        width(cx, "msg-c1-text") > width(cx, "msg-c0-text"),
        "spaces that fit are drawn"
    );
    assert!(
        text_lines(cx, "msg-c2") > 2.0,
        "a no-break space wraps like a letter"
    );
    // Fifteen lines, the last all spaces: one line it stays, so the message folds.
    assert!(at(cx, "msg-c3-fold").is_some());
}

#[test]
fn stored_ranges_move_into_the_shown_text() {
    use crate::rich_answer::js;
    let field = |source: &str| Some(std::rc::Rc::new(js::parse(source).unwrap()));
    let mut message = crate::chat_message::merge::Fields::new();
    // The shown text starts five units in; a range past its end or before its start goes.
    let pasted = r#"[{"start":1,"end":3,"display":"no"},{"start":11,"end":16,"display":"yes"}]"#;
    message.insert("pastedTextRanges".into(), field(pasted));
    let slash = r#"[{"start":5,"end":6},{"start":16,"end":17}]"#;
    message.insert("slashCommandRanges".into(), field(slash));
    let (content, shown) = ("é[x] héllo PASTE", "héllo PASTE");
    let ranges = super::ranges(&message, content, shown, shown);
    let spans = |all: &[super::Span]| -> Vec<(f64, f64)> {
        all.iter().map(|span| (span.start, span.end)).collect()
    };
    let pasted: Vec<super::Span> = ranges.pasted.iter().map(|p| p.span).collect();
    assert_eq!(spans(&pasted), [(6.0, 11.0)]);
    assert_eq!(ranges.pasted[0].display, "yes");
    assert_eq!(spans(&ranges.slash.unwrap()), [(0.0, 1.0)]);
    assert!(ranges.references.is_empty());
    // A shown text the content does not hold places no range, and none stored stays none.
    let lost = super::ranges(&message, content, "decoded", "decoded");
    assert!(lost.pasted.is_empty() && lost.slash.unwrap().is_empty());
    message.shift_remove("slashCommandRanges");
    assert!(
        super::ranges(&message, content, shown, shown)
            .slash
            .is_none()
    );
}
