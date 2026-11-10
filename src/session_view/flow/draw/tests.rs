use std::time::Duration;

use gpui::{Modifiers, TestAppContext, VisualTestContext, px};

use super::super::super::user_row::tests::{at, lines, open, open_json, settle};
use super::{CHIP, Island, Ranges, lines as read};
use crate::{
    chat_quotes::Quote,
    i18n::Translator,
    open_external::tests as opener,
    rich_answer::js,
    user_message::sent::{Pasted, Reference, Span},
};

const WEB: &str = "https://example.com/a";

/// A length as layout snaps it to the test window's half pixel.
fn snapped(length: f32) -> gpui::Pixels {
    px((length * 2.0).round() / 2.0)
}

/// A line's text, where its chips stand, and its links as `(nth, start, end)`.
type Read = (String, Vec<usize>, Vec<(usize, usize, usize)>);

fn flat(lines: Vec<super::Line>) -> Vec<Read> {
    let line = |line: super::Line| {
        let chips = line.chips.iter().map(|(at, _)| *at).collect();
        let link = |link: &super::Link| (link.nth, link.range.start, link.range.end);
        (line.text, chips, line.links.iter().map(link).collect())
    };
    lines.into_iter().map(line).collect()
}

fn ranges(legacy: bool) -> Ranges {
    Ranges {
        slash: (!legacy).then(Vec::new),
        ..Default::default()
    }
}

fn read_lines(body: &str, legacy: bool) -> Vec<Read> {
    let t = Translator::new("en").unwrap();
    flat(read(&[Island::Text(body, ranges(legacy))], true, &t))
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
    let t = Translator::new("en").unwrap();
    assert!(
        read(&[Island::Text(WEB, ranges(true))], false, &t)[0]
            .links
            .is_empty()
    );
}

#[test]
fn saved_ranges_draw_their_chips() {
    let t = Translator::new("en").unwrap();
    let chip = CHIP.to_string();
    let parsed = |source: &str| js::parse(source).unwrap();
    let tab = r#"{"kind":"browser-tab","start":4,"end":7,"url":"https://x.y","title":"X"}"#;
    let message = r#"{"kind":"message","start":8,"end":11,"text":"hi"}"#;
    let ranges = Ranges {
        pasted: vec![Pasted::read(&parsed(
            r#"{"start":12,"end":15,"display":"3 lines"}"#,
        ))],
        slash: Some(vec![Span::read(&parsed(r#"{"start":0,"end":3}"#))]),
        references: vec![
            Reference::read(&parsed(tab)),
            Reference::read(&parsed(message)),
        ],
    };
    // A message reference keeps its text until 9.5k and 9.5l; `/no` is no stored command.
    let body = "/go tab msg a\nb\n/no @a.rs";
    assert_eq!(
        flat(read(&[Island::Text(body, ranges)], true, &t)),
        [
            (format!("{chip} {chip} msg {chip}"), vec![0, 4, 12], vec![]),
            (format!("/no {chip}"), vec![4], vec![]),
        ]
    );
}

#[test]
fn a_message_reference_over_two_lines_draws_two() {
    let t = Translator::new("en").unwrap();
    let parsed = js::parse(r#"{"kind":"message","start":2,"end":9}"#).unwrap();
    let ranges = Ranges {
        references: vec![Reference::read(&parsed)],
        ..Default::default()
    };
    assert_eq!(
        flat(read(&[Island::Text("x [a\r\nb] y", ranges)], true, &t)),
        [
            ("x [a".to_owned(), vec![], vec![]),
            ("b] y".to_owned(), vec![], vec![])
        ]
    );
}

#[gpui::test]
fn a_stored_reference_with_a_line_feed_draws(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let content = serde_json::json!({
        "text": "[a\nb](cindy://session/s?message=m)",
        "agentReferences": [{ "kind": "message", "start": 0, "end": 34, "href": "cindy://session/s?message=m" }],
    });
    let cx = open_json(cx, dir.path(), 2400.0, &[&content.to_string()]);
    assert_eq!(at(cx, "msg-c0-text").unwrap().size.height, px(48.0));
}

#[gpui::test]
fn stored_ranges_show_as_chips(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let ranged = concat!(
        r#"{"text":"/skill:git fix PASTE cindy://session/abc cindy://session/abc?message=m1","#,
        r#""slashCommandRanges":[{"start":0,"end":10}],"#,
        r#""pastedTextRanges":[{"start":15,"end":20,"display":"Pasted"}],"#,
        r#""agentReferences":[{"kind":"session","start":21,"end":40,"href":"cindy://session/abc","title":"Plan"},"#,
        r#"{"kind":"message","start":41,"end":71,"href":"cindy://session/abc?message=m1","text":"hi"}]}"#
    );
    let cx = open_json(cx, dir.path(), 2400.0, &[ranged]);
    let chip = |cx: &mut VisualTestContext, n: usize| at(cx, &format!("msg-c0-chip-{n}")).unwrap();
    let (command, pasted, session) = (chip(cx, 0), chip(cx, 1), chip(cx, 2));
    assert!(
        at(cx, "msg-c0-chip-3").is_none(),
        "a message reference waits for its chip"
    );
    // `/git`, `Pasted` and `Plan` at 7.2 pixels a letter, the last two with an icon.
    let wide = |chip: gpui::Bounds<gpui::Pixels>, letters: f32, rest: f32| {
        let over = f32::from(chip.size.width) - (letters * 7.2 + rest);
        assert!((0.0..1.0).contains(&over), "{letters} letters: {over}");
    };
    wide(command, 4.0, 18.0);
    wide(pasted, 6.0, 38.0);
    wide(session, 4.0, 38.0);
    assert!(command.right() < pasted.left() && pasted.right() < session.left());
    // The pasted chip tells its text in a frame 320 wide.
    cx.simulate_mouse_move(pasted.center(), None, Modifiers::none());
    cx.executor().advance_clock(Duration::from_millis(300));
    cx.run_until_parked();
    assert_eq!(at(cx, "tip").unwrap().size.width, px(320.0));
}

#[gpui::test]
fn the_fold_reads_a_pasted_run_as_its_chip(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let pasted = lines(20);
    let text = serde_json::to_string(&pasted).unwrap();
    let ranged = format!(
        r#"{{"text":{text},"pastedTextRanges":[{{"start":0,"end":{},"display":"Pasted 20 lines"}}]}}"#,
        pasted.len()
    );
    let plain = format!(r#"{{"text":{text}}}"#);
    let cx = open_json(cx, dir.path(), 2400.0, &[&ranged, &plain]);
    assert!(at(cx, "msg-c0-chip-0").is_some());
    assert!(at(cx, "msg-c0-fold").is_none(), "one chip is one row");
    assert!(at(cx, "msg-c1-fold").is_some());
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
    let ranged = r#"{"text":"/compact now","slashCommandRanges":[]}"#;
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

#[gpui::test]
fn a_chip_whose_words_run_long_folds_the_message(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let display = serde_json::to_string(&lines(20)).unwrap();
    let ranged = format!(
        r#"{{"text":"PASTE","pastedTextRanges":[{{"start":0,"end":5,"display":{display}}}]}}"#
    );
    let cx = open_json(cx, dir.path(), 2400.0, &[&ranged]);
    assert!(
        at(cx, "msg-c0-fold").is_some(),
        "the mirror reads the chip's words"
    );
    // The chip draws them on one line: `truncate` collapses the line feeds.
    assert_eq!(at(cx, "msg-c0-chip-0").unwrap().size.width, px(240.0));
    assert_eq!(at(cx, "msg-c0-text").unwrap().size.height, snapped(29.88));
}

#[test]
fn a_quote_stands_between_its_text_islands() {
    let t = Translator::new("en").unwrap();
    let chip = CHIP.to_string();
    let quote = Quote {
        text: "said".to_owned(),
        ..Default::default()
    };
    let islands = [
        Island::Text("/go a\n", ranges(true)),
        Island::Quote(&quote),
        Island::Text("/on b", ranges(false)),
    ];
    // Each island reads its own line start and keeps its own ranges.
    assert_eq!(
        flat(read(&islands, true, &t)),
        [
            (format!("{chip} a"), vec![0], vec![]),
            (format!("{chip}/on b"), vec![0], vec![]),
        ]
    );
}

#[gpui::test]
fn quote_chips_stand_among_the_text_with_its_ranges(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let text = "PASTE and\n> <!-- cindy-composer-quote -->\n> first  quote\n> — source: src/a.rs#L3\n\n/go on";
    let at_go = text.find("/go").unwrap();
    let content = serde_json::json!({
        "text": text,
        "quotesEncoded": true,
        "pastedTextRanges": [{ "start": 0, "end": 5, "display": "Pasted" }],
        "slashCommandRanges": [{ "start": at_go, "end": at_go + 3 }],
    });
    let cx = open_json(cx, dir.path(), 2400.0, &[&content.to_string()]);
    let chip = |cx: &mut VisualTestContext, n: usize| at(cx, &format!("msg-c0-chip-{n}")).unwrap();
    let (pasted, quote, command) = (chip(cx, 0), chip(cx, 1), chip(cx, 2));
    let text = at(cx, "msg-c0-text").unwrap();
    // One row: the islands' own line feeds are gone, and `mx-1` parts the quote from them.
    assert_eq!(pasted.left(), text.left());
    assert_eq!(quote.left(), pasted.right() + px(4.0 * 9.0 + 4.0));
    assert_eq!(command.left(), quote.right() + px(4.0));
    // `first quote` on one line, with its icon; `/go` from its range in the second island.
    assert_eq!(quote.size, gpui::size(px(118.0), px(26.0)));
    assert_eq!(command.size.width, px(40.0));
    // The quote's 26px box sits a pixel under a 24px chip, which `top-[-1px]` lifts.
    assert_eq!(quote.top(), pasted.top() + px(1.0));
    assert_eq!(text.size.height, snapped(21.75 - 0.516 * 7.5 + 13.0));
    cx.simulate_mouse_move(quote.center(), None, Modifiers::none());
    cx.executor().advance_clock(Duration::from_millis(300));
    cx.run_until_parked();
    assert_eq!(at(cx, "tip").unwrap().size.width, px(320.0));
}

#[gpui::test]
fn a_folded_message_reference_is_a_quote_chip(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let href = "cindy://session/s?message=m";
    let content = serde_json::json!({
        "text": format!("{href}\n{}", lines(15)),
        "agentReferences": [{ "kind": "message", "start": 0, "end": href.len(), "href": href, "text": "hi there" }],
    });
    let cx = open_json(cx, dir.path(), 2400.0, &[&content.to_string()]);
    let chip = at(cx, "msg-c0-chip-0").unwrap();
    let text = at(cx, "msg-c0-text").unwrap();
    // `hi there` with the quote's icon, set as the other chips of a line.
    assert_eq!(chip.size.width, px(96.0));
    assert_eq!(chip.top() - text.top(), px(4.0));
    let fold = at(cx, "msg-c0-fold").unwrap();
    cx.simulate_click(fold.center(), Modifiers::none());
    settle(cx);
    assert!(
        at(cx, "msg-c0-chip-0").is_none(),
        "unfolded, it waits for its link chip"
    );
}

#[gpui::test]
fn a_message_of_quotes_alone_draws_them(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let mark = "> <!-- cindy-composer-quote -->";
    let content = serde_json::json!({
        "text": format!("{mark}\n> hello\n\n{mark}\n> there"),
        "quotesEncoded": true,
    });
    let cx = open_json(cx, dir.path(), 2400.0, &[&content.to_string()]);
    let (first, second) = (
        at(cx, "msg-c0-chip-0").unwrap(),
        at(cx, "msg-c0-chip-1").unwrap(),
    );
    assert_eq!(second.left(), first.right() + px(8.0));
    // The line's strut holds the baseline the chips centre on.
    assert_eq!(at(cx, "msg-c0-text").unwrap().size.height, px(31.0));
}
