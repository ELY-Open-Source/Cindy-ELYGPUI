use std::time::Duration;

use gpui::{Modifiers, TestAppContext, VisualTestContext, px};

use super::super::super::user_row::tests::{at, lines, open, open_json};
use super::{CHIP, lines as read};
use crate::open_external::tests as opener;

const WEB: &str = "https://example.com/a";

/// A length as layout snaps it to the test window's half pixel.
fn snapped(length: f32) -> gpui::Pixels {
    px((length * 2.0).round() / 2.0)
}

/// A line's text, where its chips stand, and its links as `(nth, start, end)`.
type Read = (String, Vec<usize>, Vec<(usize, usize, usize)>);

fn read_lines(body: &str, legacy: bool) -> Vec<Read> {
    let line = |line: super::Line| {
        let chips = line.chips.iter().map(|(at, _)| *at).collect();
        let link = |link: &super::Link| (link.nth, link.range.start, link.range.end);
        (line.text, chips, line.links.iter().map(link).collect())
    };
    read(body, legacy, true).into_iter().map(line).collect()
}

#[test]
fn a_line_holds_its_chips_and_links() {
    let chip = CHIP.to_string();
    let body = format!("/go see @a.rs\r\nx\r {WEB} @docs/ {WEB}\n");
    assert_eq!(
        read_lines(&body, true),
        [
            (format!("{chip} see {chip}"), vec![0, 8], vec![]),
            (
                format!("x {WEB} {chip} {WEB}"),
                vec![24],
                vec![(0, 2, 23), (1, 28, 49)]
            ),
        ]
    );
    // Without the heuristic a command is text; a final line feed alone keeps its one line.
    assert_eq!(
        read_lines("/go\n", false),
        [("/go".to_owned(), vec![], vec![])]
    );
    assert_eq!(read_lines("\n", true), [(String::new(), vec![], vec![])]);
    assert_eq!(read_lines("a\n\n", true).len(), 2);
    assert_eq!(read_lines("", true), [(String::new(), vec![], vec![])]);
    // Session links and image paths are 9.5l's chips.
    let other = "cindy://session/abc /tmp/a.png https://x.y";
    assert_eq!(read_lines(other, true)[0].2, [(0, 31, 42)]);
    // The stand-in typed by hand is text, and a folded message reads no link.
    assert_eq!(read_lines(&chip, true), [(chip, vec![], vec![])]);
    assert!(read(WEB, true, false)[0].links.is_empty());
}

#[gpui::test]
fn chips_sit_in_their_line_of_text(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let body =
        "/compact now\nsee @src/App.tsx and @docs/ with @.claude/agents/rev.md\nmail a@b.co @you";
    let cx = open(
        cx,
        dir.path(),
        2400.0,
        &[(body, None), ("plain\n\nlines", None)],
    );
    let text = at(cx, "msg-c0-text").unwrap();
    let chip = |cx: &mut VisualTestContext, n: usize| at(cx, &format!("msg-c0-chip-{n}")).unwrap();
    let (command, file, dir_chip, agent) = (chip(cx, 0), chip(cx, 1), chip(cx, 2), chip(cx, 3));
    assert!(
        at(cx, "msg-c0-chip-4").is_none(),
        "an address and a name are text"
    );
    assert!(at(cx, "msg-c1-chip-0").is_none());
    let plain = at(cx, "msg-c1-text").unwrap();
    assert_eq!(plain.size.height, px(72.0), "an empty line keeps its row");
    assert_eq!(command.left(), text.left());
    // `leading-5`, `py-0.5` and the border.
    for chip in [command, file, dir_chip, agent] {
        assert_eq!(chip.size.height, px(26.0));
    }
    // 7.2 pixels a letter at 12px and the insets; layout's snaps may add up to a pixel.
    let wide = |chip: gpui::Bounds<gpui::Pixels>, letters: f32, rest: f32| {
        let over = f32::from(chip.size.width) - (letters * 7.2 + rest);
        assert!((0.0..1.0).contains(&over), "{letters} letters: {over}");
    };
    wide(command, 8.0, 18.0);
    wide(file, 7.0, 38.0);
    wide(dir_chip, 5.0, 38.0);
    wide(agent, 3.0, 38.0);
    // One row holds the second line's chips, between its words.
    assert_eq!((file.top(), dir_chip.top()), (agent.top(), agent.top()));
    assert!(file.left() > text.left() + px(30.0), "after `see `");
    assert!(file.right() < dir_chip.left() && dir_chip.right() < agent.left());
    assert!(file.top() > command.bottom());
}

/// The test font: ascent 1.025 em, descent -0.275 em, x-height 0.516 em, as gpui centres it.
#[gpui::test]
fn a_chip_row_grows_about_the_chip(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(
        cx,
        dir.path(),
        2400.0,
        &[("a @a.rs b", None), ("a b", None)],
    );
    let (tall, plain) = (
        at(cx, "msg-c0-text").unwrap(),
        at(cx, "msg-c1-text").unwrap(),
    );
    let chip = at(cx, "msg-c0-chip-0").unwrap();
    assert_eq!(plain.size.height, px(24.0));
    // The baseline lies 21.75 down; the chip's middle 3.87 over it, its margin box 12 more.
    let top = 21.75 - 0.516 * 15.0 / 2.0 - 12.0;
    assert_eq!(tall.size.height, snapped(top + 24.0));
    // The border box starts a pixel over the margin box, and `top-[-1px]` lifts it one more.
    assert_eq!(chip.top() - tall.top(), snapped(top - 2.0));
    assert_eq!(chip.bottom(), tall.bottom(), "the margin box ends the row");
}

#[gpui::test]
fn a_chip_wraps_as_a_word_and_stops_at_its_widest(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let long = format!("@src/{}.rs", "n".repeat(60));
    let body = format!("{} @src/main.rs", "word ".repeat(9));
    let cx = open(cx, dir.path(), 2400.0, &[(&body, None), (&long, None)]);
    let text = at(cx, "msg-c0-text").unwrap();
    let chip = at(cx, "msg-c0-chip-0").unwrap();
    // 45 letters fill 405 of the 454 pixels; the chip's 105 go to the next row.
    assert_eq!(chip.left(), text.left());
    assert!(chip.top() > text.top() + px(24.0));
    assert_eq!(
        text.size.width,
        px(454.0),
        "a wrapped line fills the bubble"
    );
    // `max-w-[min(240px,55vw)]`.
    assert_eq!(at(cx, "msg-c1-chip-0").unwrap().size.width, px(240.0));
}

#[gpui::test]
fn an_icon_takes_its_room_in_the_row(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    // 360 pixels of words: a nine-letter label fits beside them, its icon does not.
    let body = format!("{}@abcdef.rs", "word ".repeat(8));
    let cx = open(cx, dir.path(), 2400.0, &[(&body, None)]);
    let (text, chip) = (
        at(cx, "msg-c0-text").unwrap(),
        at(cx, "msg-c0-chip-0").unwrap(),
    );
    assert_eq!(chip.left(), text.left());
    assert!(chip.top() > text.top() + px(20.0));
}

#[gpui::test]
fn the_fold_counts_the_plain_text_and_clamps_the_drawn_rows(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    // Five paths of three plain rows each fold; their five chips need no clamp.
    let path = format!("@src/{}.rs", "n".repeat(120));
    let body = vec![path; 5].join("\n");
    // A final line feed ends the fourteenth line and opens no fifteenth.
    let ended = format!("{}\n", lines(14));
    let cx = open(cx, dir.path(), 2400.0, &[(&body, None), (&ended, None)]);
    assert!(at(cx, "msg-c0-fold").is_some());
    assert!(at(cx, "msg-c0-chip-4").is_some());
    assert_eq!(
        at(cx, "msg-c0-text").unwrap().size.width,
        px(240.0),
        "no ellipsis"
    );
    assert!(at(cx, "msg-c1-fold").is_none());
}

#[gpui::test]
fn a_narrow_window_narrows_the_widest_chip(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let long = format!("@src/{}.rs", "n".repeat(60));
    let cx = open(cx, dir.path(), 400.0, &[(&long, None)]);
    let viewport = cx.update(|window, _| window.viewport_size().width);
    assert!(viewport < px(436.0), "{viewport:?}");
    assert_eq!(at(cx, "msg-c0-chip-0").unwrap().size.width, viewport * 0.55);
}

#[gpui::test]
fn stored_command_ranges_end_the_heuristic(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let ranged = r#"{"text":"/compact now","slashCommandRanges":[{"start":0,"end":8}]}"#;
    let cx = open_json(
        cx,
        dir.path(),
        2400.0,
        &[ranged, r#"{"text":"/compact now"}"#],
    );
    assert!(at(cx, "msg-c0-chip-0").is_none());
    assert!(at(cx, "msg-c1-chip-0").is_some());
}

#[gpui::test]
fn a_link_beside_a_chip_keeps_its_place(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let body = format!("{WEB} @a.rs http://e.co/b");
    let cx = open(cx, dir.path(), 2400.0, &[(&body, None)]);
    let chip = at(cx, "msg-c0-chip-0").unwrap();
    let first = at(cx, "msg-c0-link-0-0").unwrap();
    let second = at(cx, "msg-c0-link-1-0").unwrap();
    assert_eq!(
        (first.size.width, second.size.width),
        (px(189.0), px(117.0))
    );
    assert!(first.right() < chip.left() && chip.right() < second.left());
    assert_eq!(first.top(), second.top());
    assert!(at(cx, "msg-c0-link-1-1").is_none(), "one row, one box");
    cx.simulate_click(second.center(), Modifiers::none());
    assert_eq!(opener::take(), ["http://e.co/b"]);
}

#[gpui::test]
fn a_folded_message_keeps_its_chips(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let body = format!("@a.rs\n{}\n@b.rs", lines(15));
    let cx = open(cx, dir.path(), 2400.0, &[(&body, None)]);
    assert!(at(cx, "msg-c0-fold").is_some());
    assert!(at(cx, "msg-c0-chip-0").is_some());
    assert!(at(cx, "msg-c0-chip-1").is_none(), "past the tenth row");
}

#[gpui::test]
fn a_chip_tells_its_path_after_300ms(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), 2400.0, &[("@src/App.tsx /x", None)]);
    let chip = at(cx, "msg-c0-chip-0").unwrap();
    cx.simulate_mouse_move(chip.center(), None, Modifiers::none());
    cx.executor().advance_clock(Duration::from_millis(299));
    cx.run_until_parked();
    assert!(at(cx, "tip").is_none());
    cx.executor().advance_clock(Duration::from_millis(1));
    cx.run_until_parked();
    assert!(at(cx, "tip").is_some());
    assert!(at(cx, "tip-mono").is_some(), "a path sets in the code face");
}

#[gpui::test]
fn a_chip_with_nothing_to_tell_shows_no_tip(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), 2400.0, &[("@.claude/agents/.md", None)]);
    let chip = at(cx, "msg-c0-chip-0").unwrap();
    cx.simulate_mouse_move(chip.center(), None, Modifiers::none());
    cx.executor().advance_clock(Duration::from_millis(600));
    cx.run_until_parked();
    assert!(at(cx, "tip").is_none());
}
