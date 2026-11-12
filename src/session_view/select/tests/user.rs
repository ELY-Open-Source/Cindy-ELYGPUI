//! A user message's bubble in the selection: its words, its chips' labels, its blank lines.

use gpui::{TestAppContext, point, px};

use super::{corner, drag, held, pull, spot};
use crate::{
    chat_quotes::{self, Quote},
    session_view::user_row::tests::{at, open_json, open_roles},
};

const SAID: &str = "Hello world\n\nsee cindy://session/abc now";

// Blink selects all of it, a chip's label too (`logs/review/9.6k1c-blink-{user,lit}.log`).

#[gpui::test]
fn a_bubbles_words_chips_and_line_feeds_select(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let said = [("user", SAID), ("assistant", "answer")];
    let cx = open_roles(cx, dir.path(), 2400.0, &said);
    let text = at(cx, "msg-c0-text").unwrap();
    let chip = at(cx, "msg-c0-chip-0").unwrap();
    let (first, last) = (text.top() + px(12.0), chip.center().y);
    let start = point(text.left() + px(1.0), first);
    let quads = cx.update(|window, _| window.painted_quads().len());
    drag(cx, start, point(text.right() - px(1.0), last));
    let all = held(cx);
    // A line feed is a place past its line's end, a blank line's too; a chip's label is a run.
    assert_eq!(all[..4], ["Hello world", "\n", "\n", "see "]);
    assert_eq!((all.len(), all[5].as_str()), (6, " now"));
    // Each of the six is lit, the two line feeds a space wide.
    let lit = cx.update(|window, _| window.painted_quads().len());
    assert_eq!(lit, quads + 6);
    let label = all[4].clone();
    // Right of a line is its end, before the line feed.
    drag(cx, start, point(text.right() - px(1.0), first));
    assert_eq!(held(cx), ["Hello world"]);
    // Left of the bubble a row's start: the line feeds before it are held.
    let left = at(cx, "session-scroll").unwrap().left() + px(4.0);
    drag(cx, start, point(left, last));
    assert_eq!(held(cx), ["Hello world", "\n", "\n"]);
    // From the blank line on: its line feed and the line after it.
    let blank = point(text.left() + px(30.0), first + px(24.0));
    drag(cx, blank, point(chip.right() - px(3.0), last));
    assert_eq!(held(cx), ["\n".to_owned(), "see ".to_owned(), label]);
    // The chip's padding is its label's nearer end.
    drag(cx, start, point(chip.left() + px(3.0), last));
    assert_eq!(held(cx), ["Hello world", "\n", "\n", "see "]);
    // On into the answer, and from under all the text back to the bubble.
    let answer = spot(cx, "0", (1.0, 0.5));
    drag(cx, start, answer);
    assert_eq!(held(cx).last().unwrap(), "answer");
    let under = corner(cx, (300.0, 2.0));
    drag(cx, under, point(chip.center().x, text.top() - px(4.0)));
    // The bubble's last line ends its block: that break is lit when the selection goes on.
    let all = held(cx);
    assert_eq!(
        (all.len(), all[6].as_str(), all[7].as_str()),
        (8, "\n", "answer")
    );
    // Under all the text of a lone bubble is its text's end, before that break.
    drag(cx, start, point(text.right() - px(1.0), last));
    assert_eq!(held(cx).len(), 6);
}

#[gpui::test]
fn a_last_line_feed_and_a_folded_rows_space_are_places(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let said = [("user", "Hello world\n"), ("assistant", "answer")];
    let cx = open_roles(cx, dir.path(), 2400.0, &said);
    let text = at(cx, "msg-c0-text").unwrap();
    let start = point(text.left() + px(1.0), text.center().y);
    let answer = spot(cx, "0", (1.0, 0.5));
    drag(cx, start, answer);
    assert_eq!(held(cx), ["Hello world", "\n", "answer"]);
    drag(cx, start, point(text.right() + px(30.0), text.center().y));
    assert_eq!(held(cx), ["Hello world"]);
}

#[gpui::test]
fn a_folded_messages_rows_keep_their_hanging_spaces(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let long = "alpha beta gamma delta epsilon ".repeat(40);
    let cx = open_roles(cx, dir.path(), 2400.0, &[("user", &long)]);
    assert!(at(cx, "msg-c0-fold").is_some());
    let text = at(cx, "msg-c0-text").unwrap();
    let start = point(text.left() + px(1.0), text.top() + px(12.0));
    drag(cx, start, point(text.right() + px(12.0), start.y));
    let all = held(cx);
    assert_eq!((all.len(), all[1].as_str()), (2, " "), "{all:?}");
}

#[gpui::test]
fn a_wraps_hanging_space_is_a_place(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let long = "alpha beta gamma delta epsilon ".repeat(8);
    let cx = open_roles(cx, dir.path(), 2400.0, &[("user", &long)]);
    let text = at(cx, "msg-c0-text").unwrap();
    let start = point(text.left() + px(1.0), text.top() + px(12.0));
    // Past the first row's last word the space that hangs there is held, and no more.
    drag(cx, start, point(text.right() + px(12.0), start.y));
    let all = held(cx);
    assert_eq!((all.len(), all[1].as_str()), (2, " "), "{all:?}");
    assert!(!all[0].ends_with(' '), "{all:?}");
    // The whole text: every byte is in a run.
    let end = point(text.right() + px(12.0), text.bottom() - px(12.0));
    drag(cx, start, end);
    assert_eq!(held(cx).concat(), long);
}

#[gpui::test]
fn a_quote_chips_box_starts_no_selection_off_its_label(cx: &mut TestAppContext) {
    let quote = Quote {
        text: "quoted words here".into(),
        source_path: Some("/w/notes.md".into()),
        ..Default::default()
    };
    let body = format!("Before \n\n{}\n\n after", chat_quotes::format(&quote));
    let content = serde_json::json!({ "text": body, "quotesEncoded": true }).to_string();
    let dir = tempfile::tempdir().unwrap();
    let cx = open_json(cx, dir.path(), 2400.0, &[&content]);
    let (text, chip) = (
        at(cx, "msg-c0-text").unwrap(),
        at(cx, "msg-c0-chip-0").unwrap(),
    );
    let (start, end) = (
        point(text.left() + px(1.0), chip.center().y),
        point(text.right() - px(1.0), chip.center().y),
    );
    drag(cx, start, end);
    let all = held(cx);
    assert_eq!(all, ["Before ", "quoted words here", " after"]);
    // The chip is `select-none`: a press on its padding or over its label keeps the selection.
    for quiet in [
        point(chip.left() + px(3.0), chip.center().y),
        point(chip.center().x, chip.top() + px(1.0)),
    ] {
        pull(cx, quiet, end);
        assert_eq!(held(cx), all, "{quiet:?}");
    }
    // Its icon is text too: a press there places a caret at the label's start.
    drag(cx, point(chip.left() + px(16.0), chip.center().y), end);
    assert_eq!(held(cx), ["quoted words here", " after"]);
    // Its label is text again: a press there selects from it.
    drag(cx, chip.center(), end);
    let from = held(cx);
    assert_eq!((from.len(), from[1].as_str()), (2, " after"), "{from:?}");
    // A drag that ends on the padding ends at the label's nearer end.
    drag(cx, start, point(chip.left() + px(3.0), chip.center().y));
    assert_eq!(held(cx), ["Before "]);
}
