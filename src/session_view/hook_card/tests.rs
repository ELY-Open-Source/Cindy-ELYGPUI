use std::time::Duration;

use gpui::{Modifiers, TestAppContext, VisualTestContext, px};

use crate::session_view::{
    flow::draw,
    links::tests::{click, focus, key},
    user_row::tests::{at, lines, open, open_meta, settle},
};

const FEISHU: &str = r#"{"hookSource":{"im":"feishu"}}"#;
const CLEAN: &str = r#"{"hookSource":{"im":"feishu","contentFormat":"user-text"}}"#;
const THREAD: &str = r#"{"hookSource":{"im":"feishu","threadContext":[{"author":"Ann","text":"first"},{"author":"Bo","text":"a\nb"}],"contextSnapshot":{"groupContext":"g1\ng2","groupMessageCount":2}}}"#;

fn height(cx: &mut VisualTestContext, selector: &str) -> f32 {
    at(cx, selector).unwrap().size.height.into()
}

#[gpui::test]
fn the_card_sits_left_with_its_header_over_the_words(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(
        cx,
        dir.path(),
        2400.0,
        &[("hello", Some(FEISHU)), ("plain", None)],
    );
    let column = at(cx, "session-messages").unwrap();
    let card = at(cx, "msg-c0-hook").unwrap();
    assert_eq!(card.left(), column.left(), "justify-start");
    assert_eq!(card.top(), column.top() + px(28.0));
    assert_eq!(card.size.width, px(560.0), "w-full max-w-[560px]");
    let head = at(cx, "msg-c0-hook-head").unwrap();
    assert_eq!(
        head.top(),
        card.top() + px(11.0),
        "the border and pt-[10px]"
    );
    assert_eq!(head.size.height, px(25.5), "13px at 1.5 and pb-[6px]");
    let title = at(cx, "msg-c0-hook-title").unwrap();
    assert_eq!(title.left(), card.left() + px(1.0 + 14.0 + 14.0 + 8.0));
    let text = at(cx, "msg-c0-hook-text").unwrap();
    assert_eq!(text.top(), head.bottom());
    assert_eq!(text.left(), card.left() + px(15.0), "px-[14px]");
    assert_eq!(text.size.height, px(22.5), "14px at 1.6");
    assert_eq!(card.bottom(), text.bottom() + px(13.0), "pb-[12px]");
    for part in ["bubble", "orca", "hook-fold", "hook-context", "sources"] {
        assert!(at(cx, &format!("msg-c0-{part}")).is_none(), "{part}");
    }
    let next = at(cx, "item-msg-c1").unwrap();
    assert_eq!(next.top(), card.bottom() + px(14.0));
    assert!(at(cx, "msg-c1-hook").is_none());
}

#[gpui::test]
fn the_labels_and_files_stand_left_over_the_card(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let meta = r#"{"hookSource":{"im":"slack"},"origin":{"kind":"scheduler","scheduleId":"hook:x","scheduleName":"Hook · Team"},"sourceDevice":{"deviceId":"d1","platform":"mobile"},"goalObjective":{"updated":true}}"#;
    let content = serde_json::json!({
        "text": "hello",
        "files": [{ "name": "a.txt", "path": "/tmp/a.txt" }],
        "images": [{ "url": "xdt-image://s1/gone.png", "mimeType": "image/png", "originalName": "gone.png" }],
    })
    .to_string();
    let cx = open_meta(cx, dir.path(), 2400.0, &[(content.as_str(), Some(meta))]);
    let column = at(cx, "session-messages").unwrap();
    let labels = at(cx, "msg-c0-sources").unwrap();
    assert_eq!(labels.left(), column.left(), "align start");
    assert!(at(cx, "msg-c0-source-device").is_some());
    assert!(
        at(cx, "msg-c0-source-hook").is_none(),
        "the card names the channel"
    );
    assert!(
        at(cx, "msg-c0-goal").is_none(),
        "the goal badge is the bubble's"
    );
    let image = at(cx, "msg-c0-image-0-failed").unwrap();
    assert_eq!(image.left(), column.left(), "items-start");
    assert_eq!(image.top(), labels.bottom() + px(8.0));
    let files = at(cx, "msg-c0-files").unwrap();
    let file = at(cx, "msg-c0-file-0").unwrap();
    assert_eq!(files.top(), image.bottom() + px(8.0));
    assert_eq!(file.left(), column.left(), "justify-start");
    let card = at(cx, "msg-c0-hook").unwrap();
    assert_eq!(card.top(), files.bottom() + px(8.0), "gap-2");
}

#[gpui::test]
fn clean_text_folds_past_fourteen_rows_to_ten(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let rows = [(lines(14), Some(CLEAN)), (lines(15), Some(CLEAN))];
    let rows: Vec<(&str, Option<&str>)> = rows.iter().map(|(c, m)| (c.as_str(), *m)).collect();
    let cx = open(cx, dir.path(), 2400.0, &rows);
    assert_eq!(height(cx, "msg-c0-hook-text"), 14.0 * 22.5);
    assert!(at(cx, "msg-c0-hook-fold").is_none());
    assert_eq!(height(cx, "msg-c1-hook-text"), 10.0 * 22.5, "line-clamp-10");
    let (text, fold) = (
        at(cx, "msg-c1-hook-text").unwrap(),
        at(cx, "msg-c1-hook-fold").unwrap(),
    );
    assert_eq!(fold.top(), text.bottom() + px(8.0), "mt-2");
    assert_eq!(fold.left(), text.left());
    assert_eq!(fold.size.height, px(27.0), "12px at 1.6 and py-1");
    assert!(fold.size.width < px(200.0), "w-fit");
    let card = at(cx, "msg-c1-hook").unwrap();
    assert_eq!(card.bottom(), fold.bottom() + px(13.0));
    click(cx, "msg-c1-hook-fold", Modifiers::none());
    assert_eq!(height(cx, "msg-c1-hook-text"), 15.0 * 22.5);
    click(cx, "msg-c1-hook-fold", Modifiers::none());
    assert_eq!(height(cx, "msg-c1-hook-text"), 10.0 * 22.5);
}

#[gpui::test]
fn a_stored_prompt_never_folds(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let text = lines(15);
    let cx = open(cx, dir.path(), 2400.0, &[(text.as_str(), Some(FEISHU))]);
    assert_eq!(height(cx, "msg-c0-hook-text"), 15.0 * 22.5);
    assert!(at(cx, "msg-c0-hook-fold").is_none());
}

#[gpui::test]
fn the_context_unfolds_over_200ms_and_folds_away(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), 2400.0, &[("hello", Some(THREAD))]);
    let text = at(cx, "msg-c0-hook-text").unwrap();
    let toggle = at(cx, "msg-c0-hook-context").unwrap();
    assert_eq!(toggle.top(), text.bottom() + px(8.0), "gap-2");
    assert_eq!(toggle.left(), text.left());
    assert_eq!(toggle.size.height, px(26.0), "12px at 1.5 and py-1");
    assert!(
        at(cx, "msg-c0-hook-context-body").is_none(),
        "folded at first"
    );
    let card = at(cx, "msg-c0-hook").unwrap();
    assert_eq!(card.bottom(), toggle.bottom() + px(13.0));
    click(cx, "msg-c0-hook-context", Modifiers::none());
    // pt-2; a heading and three lines; a heading and two lines; gap-1 and gap-3.
    let full = 8.0 + (4.0 * 18.0 + 2.0 * 4.0) + 12.0 + (3.0 * 18.0 + 4.0);
    assert!(
        at(cx, "msg-c0-hook-collapse").is_none(),
        "it starts from nothing"
    );
    cx.executor().advance_clock(Duration::from_millis(100));
    settle(cx);
    // Half of 200ms along `--motion-ease-move`, as layout snaps it.
    let half = (full * crate::motion::ease_move(0.5) * 2.0).round() / 2.0;
    assert_eq!(height(cx, "msg-c0-hook-collapse"), half);
    assert!(half > 0.0 && half < full, "{half}");
    cx.executor().advance_clock(Duration::from_millis(100));
    settle(cx);
    assert!(
        at(cx, "msg-c0-hook-collapse").is_none(),
        "open, the body stands alone"
    );
    let body = at(cx, "msg-c0-hook-context-body").unwrap();
    assert_eq!(body.size.height, px(full));
    assert_eq!(body.top(), at(cx, "msg-c0-hook-context").unwrap().bottom());
    let (first, second) = (
        at(cx, "msg-c0-hook-section-0").unwrap(),
        at(cx, "msg-c0-hook-section-1").unwrap(),
    );
    assert_eq!(first.top(), body.top() + px(8.0), "pt-2");
    assert_eq!(first.left(), text.left() + px(18.0), "pl-[18px]");
    assert_eq!(first.size.height, px(4.0 * 18.0 + 8.0));
    assert_eq!(second.top(), first.bottom() + px(12.0), "gap-3");
    assert_eq!(second.size.height, px(3.0 * 18.0 + 4.0));
    let card = at(cx, "msg-c0-hook").unwrap();
    assert_eq!(card.bottom(), body.bottom() + px(13.0));
    click(cx, "msg-c0-hook-context", Modifiers::none());
    cx.executor().advance_clock(Duration::from_millis(100));
    settle(cx);
    let closing = height(cx, "msg-c0-hook-collapse");
    assert!(closing > 0.0 && closing < full, "{closing}");
    cx.executor().advance_clock(Duration::from_millis(100));
    settle(cx);
    assert!(
        at(cx, "msg-c0-hook-context-body").is_none(),
        "closed, it unmounts"
    );
}

#[gpui::test]
fn reduced_motion_unfolds_the_context_at_once(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), 2400.0, &[("hello", Some(THREAD))]);
    cx.update(|_, cx| cx.set_global(crate::motion::ReducedMotion(true)));
    click(cx, "msg-c0-hook-context", Modifiers::none());
    assert!(at(cx, "msg-c0-hook-collapse").is_none());
    assert!(at(cx, "msg-c0-hook-context-body").is_some());
    click(cx, "msg-c0-hook-context", Modifiers::none());
    assert!(at(cx, "msg-c0-hook-context-body").is_none());
}

#[gpui::test]
fn the_keyboard_works_both_disclosures(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let meta = r#"{"hookSource":{"im":"feishu","contentFormat":"user-text","threadContext":[{"author":"Ann","text":"first"}]}}"#;
    let text = lines(15);
    let cx = open(cx, dir.path(), 2400.0, &[(text.as_str(), Some(meta))]);
    cx.update(|_, cx| cx.set_global(crate::motion::ReducedMotion(true)));
    focus(cx, "msg-c0-hook-fold");
    assert_eq!(
        height(cx, "msg-c0-hook-text"),
        10.0 * 22.5,
        "a press alone is no click"
    );
    key(cx, "enter");
    assert_eq!(height(cx, "msg-c0-hook-text"), 15.0 * 22.5);
    cx.update(|window, cx| window.focus_next(cx));
    key(cx, "space");
    assert!(
        at(cx, "msg-c0-hook-context-body").is_some(),
        "the next Tab stop"
    );
    assert_eq!(height(cx, "msg-c0-hook-text"), 15.0 * 22.5);
}

#[gpui::test]
fn a_narrow_column_shrinks_the_card_and_its_text(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), 700.0, &[("hello", Some(THREAD))]);
    cx.update(|_, cx| cx.set_global(crate::motion::ReducedMotion(true)));
    let column = at(cx, "session-messages").unwrap();
    let card = at(cx, "msg-c0-hook").unwrap();
    assert!(card.size.width < px(560.0));
    assert_eq!(card.size.width, column.size.width);
    // The compact rail takes a pixel off each size.
    assert_eq!(height(cx, "msg-c0-hook-head"), 18.0 + 6.0, "12px at 1.5");
    assert_eq!(height(cx, "msg-c0-hook-text"), 21.0, "13px at 1.6");
    assert_eq!(height(cx, "msg-c0-hook-context"), 16.5 + 8.0, "11px at 1.5");
    click(cx, "msg-c0-hook-context", Modifiers::none());
    let full = 8.0 + (4.0 * 16.5 + 2.0 * 4.0) + 12.0 + (3.0 * 16.5 + 4.0);
    assert_eq!(height(cx, "msg-c0-hook-context-body"), full);
}

#[gpui::test]
fn the_words_are_the_channels_own_in_pre_wrap(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let said = r#"{"hookSource":{"im":"feishu","userText":"one\ntwo\n"}}"#;
    let none = r#"{"hookSource":{"im":"feishu","userText":"\r\f"}}"#;
    let prompt = "<thread_context>x</thread_context>\nask\r\nmore";
    let rows = [
        ("stored prompt", Some(said)),
        ("stored prompt", Some(none)),
        (prompt, Some(FEISHU)),
    ];
    let cx = open(cx, dir.path(), 2400.0, &rows);
    assert_eq!(height(cx, "msg-c0-hook-text"), 45.0, "the last feed ends");
    assert_eq!(height(cx, "msg-c1-hook-text"), 0.0, "no CR and no FF");
    assert_eq!(
        height(cx, "msg-c2-hook-text"),
        45.0,
        "less the thread context"
    );
}

#[gpui::test]
fn rows_break_clamp_and_keep_their_semibold_lead(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), 2400.0, &[("hello", None)]);
    cx.update(|window, _| {
        let mut style = window.text_style();
        style.font_size = px(12.0).into();
        struct Read {
            rows: Vec<(String, usize)>,
            total: usize,
        }
        let read = |text: &str, look, width: f32| {
            let pre = draw::pre(text, look, (&style, px(width)), window);
            let rows = pre.rows.into_iter().map(|row| (row.text, row.bold));
            Read {
                rows: rows.collect(),
                total: pre.total,
            }
        };
        let all = read("[Ann] one\ntwo\n", (5, None), 500.0);
        assert_eq!(all.rows, [("[Ann] one".into(), 5), ("two".into(), 0)]);
        assert_eq!(all.total, 2);
        // A lead that runs over a line feed is semibold on both lines.
        let split = read("[A\nB] x", (6, None), 500.0);
        assert_eq!(split.rows, [("[A".into(), 2), ("B] x".into(), 3)]);
        assert_eq!(read("", (0, None), 500.0).total, 0);
        assert_eq!(read("\n", (0, None), 500.0).rows, [(String::new(), 0)]);
        let long = "word ".repeat(40);
        let wrapped = read(&long, (0, None), 100.0);
        assert!(wrapped.total > 3);
        let kept = read(&long, (0, Some(2)), 100.0);
        assert_eq!((kept.rows.len(), kept.total), (2, wrapped.total));
        assert_eq!(kept.rows[0], wrapped.rows[0]);
        assert!(kept.rows[1].0.ends_with('\u{2026}'), "{:?}", kept.rows[1]);
        let fits = read(&long, (0, Some(wrapped.total)), 100.0);
        assert_eq!(
            fits.rows, wrapped.rows,
            "a clamp that cuts nothing adds no dots"
        );
        // A lead that wraps stays semibold on each row it reaches.
        let lead = read(&long, (long.len(), None), 100.0);
        assert!(lead.rows.iter().all(|(row, bold)| *bold == row.len()));
        let short = read(&long, (wrapped.rows[0].0.len() + 2, None), 100.0);
        let bold: Vec<usize> = short.rows.iter().take(3).map(|(_, bold)| *bold).collect();
        assert_eq!(
            bold,
            [wrapped.rows[0].0.len(), 1, 0],
            "{:?}",
            &short.rows[..3]
        );
    });
}

#[gpui::test]
fn another_speaker_shows_their_id_as_the_tip(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let guest = r#"{"hookSource":{"im":"feishu","origin":{"sender":{"isOwner":false,"name":"Ann","id":"u2"},"chat":{"kind":"direct"}}}}"#;
    let cx = open(
        cx,
        dir.path(),
        2400.0,
        &[("a", Some(guest)), ("b", Some(FEISHU))],
    );
    for (n, tip) in [(1, false), (0, true)] {
        let title = at(cx, &format!("msg-c{n}-hook-title")).unwrap();
        cx.simulate_mouse_move(title.center(), None, Modifiers::none());
        cx.executor().advance_clock(Duration::from_millis(600));
        cx.run_until_parked();
        assert_eq!(at(cx, "tip").is_some(), tip, "c{n}");
    }
}

#[gpui::test]
fn a_long_heading_stays_on_one_line_inside_the_card(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let name = "A name\\nof some length ".repeat(8);
    let meta = format!(
        r#"{{"hookSource":{{"im":"feishu","origin":{{"sender":{{"isOwner":false,"name":"{name}"}}}}}}}}"#
    );
    let cx = open(cx, dir.path(), 2400.0, &[("a", Some(meta.as_str()))]);
    let (head, title) = (
        at(cx, "msg-c0-hook-head").unwrap(),
        at(cx, "msg-c0-hook-title").unwrap(),
    );
    assert_eq!(title.size.height, px(19.5), "truncate");
    assert_eq!(
        title.right(),
        head.right() - px(14.0),
        "min-w-0 inside px-[14px]"
    );
}

#[gpui::test]
fn the_keyboard_draws_the_focus_outline(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), 2400.0, &[("hello", Some(THREAD))]);
    let toggle = at(cx, "msg-c0-hook-context").unwrap();
    focus(cx, "msg-c0-hook-context");
    assert!(
        at(cx, "msg-c0-hook-context-outline").is_none(),
        "a press shows none"
    );
    key(cx, "shift");
    let outline = at(cx, "msg-c0-hook-context-outline").unwrap();
    // 2px wide, 2px off the button.
    assert_eq!(outline.left(), toggle.left() - px(4.0));
    assert_eq!(outline.size.height, toggle.size.height + px(8.0));
}

/// The rows `text` takes at `width` CSS pixels and `size`, as the card breaks it.
fn rows_at(cx: &mut VisualTestContext, text: &str, size: f32, width: f32) -> usize {
    cx.update(|window, _| {
        let mut style = window.text_style();
        style.font_size = px(size).into();
        draw::pre(text, (0, None), (&style, px(width)), window).total
    })
}

#[gpui::test]
fn the_words_and_the_context_break_at_their_own_widths(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let long = "x".repeat(1500);
    let meta = serde_json::json!({ "hookSource": {
        "im": "feishu",
        "contextSnapshot": { "replyContext": long },
    } })
    .to_string();
    let cx = open(
        cx,
        dir.path(),
        2400.0,
        &[(long.as_str(), Some(meta.as_str()))],
    );
    cx.update(|_, cx| cx.set_global(crate::motion::ReducedMotion(true)));
    // 560 less the border and `px-[14px]`; the context less `pl-[18px]` more.
    let (said, quoted) = (
        rows_at(cx, &long, 14.0, 530.0),
        rows_at(cx, &long, 12.0, 512.0),
    );
    assert!(rows_at(cx, &long, 14.0, 560.0) < said && rows_at(cx, &long, 12.0, 530.0) < quoted);
    assert_eq!(height(cx, "msg-c0-hook-text"), said as f32 * 22.5);
    click(cx, "msg-c0-hook-context", Modifiers::none());
    let section = height(cx, "msg-c0-hook-section-0");
    assert_eq!(section, (quoted + 1) as f32 * 18.0 + 4.0);
}

#[gpui::test]
fn a_narrow_card_breaks_its_words_at_its_own_width(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let long = "x".repeat(1500);
    let cx = open(cx, dir.path(), 700.0, &[(long.as_str(), Some(FEISHU))]);
    let card = f32::from(at(cx, "msg-c0-hook").unwrap().size.width);
    let rows = rows_at(cx, &long, 13.0, card - 30.0);
    assert!(rows_at(cx, &long, 13.0, 530.0) < rows);
    assert_eq!(height(cx, "msg-c0-hook-text"), rows as f32 * 21.0);
}

#[gpui::test]
fn an_empty_line_keeps_its_height(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), 2400.0, &[("a\n\nb", Some(FEISHU))]);
    assert_eq!(height(cx, "msg-c0-hook-text"), 3.0 * 22.5);
}

#[gpui::test]
fn files_that_wrap_start_each_row_at_the_left(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let file = |n: usize| serde_json::json!({ "name": format!("a file with a long name {n}.txt"), "path": format!("/tmp/{n}.txt") });
    let files: Vec<serde_json::Value> = (0..4).map(file).collect();
    let content = serde_json::json!({ "text": "hello", "files": files }).to_string();
    let cx = open_meta(cx, dir.path(), 700.0, &[(content.as_str(), Some(FEISHU))]);
    let row = at(cx, "msg-c0-files").unwrap();
    let (first, last) = (
        at(cx, "msg-c0-file-0").unwrap(),
        at(cx, "msg-c0-file-3").unwrap(),
    );
    assert!(last.top() > first.top(), "the chips wrap");
    assert_eq!(first.left(), row.left());
    assert_eq!(last.left(), row.left(), "justify-start");
    assert!(last.right() < row.right(), "the last row has room to spare");
}
