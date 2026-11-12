//! The stream's other text in the selection: labels, a goal badge, the Orca and hook cards.

use gpui::{Bounds, Modifiers, Pixels, Point, TestAppContext, VisualTestContext, point, px};

use super::{drag, held, lift, press};
use crate::session_view::user_row::tests::{at, open, open_json, open_meta, settle};

// What is a `button` selects nothing (`logs/review/9.6k1d-blink-other.log`).

const SOURCES: &str = r#"{"origin":{"kind":"scheduler","scheduleId":"job 1","scheduleName":"Digest"},"goalObjective":{"updated":true}}"#;
const CHANNEL: &str =
    r#"{"origin":{"kind":"scheduler","scheduleId":"hook:x","scheduleName":"Hook · Team"}}"#;
const THREAD: &str = r#"{"hookSource":{"im":"feishu","threadContext":[{"author":"Ann","text":"first"},{"author":"Bo","text":"a\nb"}]}}"#;

fn start(of: Bounds<Pixels>) -> Point<Pixels> {
    point(of.left() + px(1.0), of.center().y)
}

fn end(of: Bounds<Pixels>) -> Point<Pixels> {
    point(of.right() - px(1.0), of.center().y)
}

fn click(cx: &mut VisualTestContext, selector: &str) {
    let on = at(cx, selector).unwrap().center();
    cx.simulate_click(on, Modifiers::none());
    settle(cx);
}

/// A press on `selector`, then off it with no click: what the selection holds meanwhile.
fn pressed(cx: &mut VisualTestContext, selector: &str) -> Vec<String> {
    let on = at(cx, selector).unwrap().center();
    press(cx, on);
    let kept = held(cx);
    let off = super::corner(cx, (2.0, 2.0));
    lift(cx, off);
    kept
}

#[gpui::test]
fn a_goal_badge_and_a_plain_label_select_and_a_button_does_not(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let rows = [("hello", Some(SOURCES)), ("plain", Some(CHANNEL))];
    let cx = open(cx, dir.path(), 2400.0, &rows);
    let (goal, text) = (at(cx, "msg-c0-goal"), at(cx, "msg-c0-text"));
    let (goal, text) = (goal.unwrap(), text.unwrap());
    drag(cx, start(goal), end(text));
    let badge = held(cx);
    assert_eq!((badge.len(), badge[1].as_str()), (2, "hello"), "{badge:?}");
    // A drag to the label that opens the automation stops at the next text's start.
    let label = at(cx, "msg-c0-source-scheduler").unwrap();
    drag(cx, end(text), label.center());
    assert_eq!(held(cx), badge);
    // A press on it keeps what is selected.
    assert_eq!(pressed(cx, "msg-c0-source-scheduler"), badge);
    // A label that opens nothing is text.
    let channel = at(cx, "msg-c1-source-hook").unwrap();
    drag(cx, start(channel), end(channel));
    assert_eq!(held(cx).len(), 1);
}

#[gpui::test]
fn a_plain_labels_avatar_starts_no_selection(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let bot = r#"{"origin":{"kind":"session","senderSessionId":"device::task","senderBotId":"b1","senderBotName":"Alice"}}"#;
    let cx = open(cx, dir.path(), 2400.0, &[("Plain words", Some(bot))]);
    let (label, text) = (at(cx, "msg-c0-source-bot"), at(cx, "msg-c0-text"));
    let (label, text) = (label.unwrap(), text.unwrap());
    // `BotAvatar` is `select-none`: 14px at the label's left.
    let avatar = point(label.left() + px(7.0), label.center().y);
    drag(cx, avatar, end(text));
    assert!(held(cx).is_empty());
    // A drag that ends on it is the label's start.
    drag(cx, end(text), avatar);
    assert_eq!(held(cx).len(), 2);
    // Past the disc's arc its box is the label's own space.
    drag(cx, label.origin + point(px(1.0), px(2.25)), end(text));
    assert_eq!(held(cx).len(), 2);
}

#[gpui::test]
fn a_file_chip_selects_nothing_and_keeps_the_selection(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let files = serde_json::json!({ "text": "With files", "files": [{ "name": "notes.md", "path": "/w/notes.md" }] });
    let cx = open_json(cx, dir.path(), 2400.0, &[&files.to_string()]);
    let (chip, text) = (at(cx, "msg-c0-file-0"), at(cx, "msg-c0-text"));
    let (chip, text) = (chip.unwrap(), text.unwrap());
    drag(cx, end(text), chip.center());
    assert_eq!(held(cx), ["With files"]);
    assert_eq!(pressed(cx, "msg-c0-file-0"), ["With files"]);
    drag(cx, start(chip), end(chip));
    assert!(held(cx).is_empty());
    // Past the pill's arc a press is no button's: it drops the selection.
    drag(cx, start(text), end(text));
    press(cx, chip.origin + point(px(3.0), px(3.0)));
    assert!(held(cx).is_empty());
    let off = super::corner(cx, (2.0, 2.0));
    lift(cx, off);
    drag(cx, start(text), end(text));
    press(cx, chip.origin + point(px(6.0), px(6.0)));
    assert_eq!(held(cx), ["With files"]);
    lift(cx, off);
}

#[gpui::test]
fn an_orca_cards_body_selects_and_its_head_does_not(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let lead = r#"{"orcaSource":"lead","content":"do it now\nsecond line"}"#;
    let cx = open(cx, dir.path(), 2400.0, &[(lead, None)]);
    click(cx, "msg-c0-orca-head");
    let head = at(cx, "msg-c0-orca-head").unwrap();
    let body = at(cx, "msg-c0-orca-text").unwrap();
    let from = point(body.left() + px(1.0), body.top() + px(4.0));
    drag(
        cx,
        from,
        point(body.right() - px(1.0), body.bottom() - px(4.0)),
    );
    assert_eq!(held(cx), ["do it now", "\n", "second line"]);
    // From the head the focus is the body's start.
    drag(cx, end(body), head.center());
    let all = ["do it now", "\n", "second line"];
    assert_eq!(held(cx), all);
    assert_eq!(pressed(cx, "msg-c0-orca-head"), all);
}

#[gpui::test]
fn a_hook_cards_heading_words_and_context_select(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open_meta(cx, dir.path(), 2400.0, &[("\"the ask\"", Some(THREAD))]);
    let (title, text) = (at(cx, "msg-c0-hook-title"), at(cx, "msg-c0-hook-text"));
    let (title, text) = (title.unwrap(), text.unwrap());
    drag(cx, start(title), end(text));
    let said = held(cx);
    assert_eq!((said.len(), said[1].as_str()), (2, "the ask"), "{said:?}");
    // The fold is a button: it keeps the selection and holds no text.
    assert_eq!(pressed(cx, "msg-c0-hook-context"), said);
    click(cx, "msg-c0-hook-context");
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(400));
    settle(cx);
    let section = at(cx, "msg-c0-hook-section-0").unwrap();
    let under = point(section.left() + px(200.0), section.bottom() + px(2.0));
    drag(cx, start(text), under);
    // Its heading, then each line: the author's face a run of its own.
    let all = held(cx);
    assert_eq!(all[..2], ["the ask", "\n"]);
    assert_eq!(
        all[3..],
        ["\n", "[Ann]", " first", "\n", "[Bo]", " a", "\n", "b"]
    );
}
