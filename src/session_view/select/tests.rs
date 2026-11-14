use gpui::{Modifiers, MouseButton, Pixels, Point, TestAppContext, VisualTestContext, point, px};

use super::{Cell, Point as Place, Run, Selection, cells, picked};
use crate::session_view::user_row::tests::{at, open_roles};

mod card;
mod clicks;
mod other;
mod user;

const TEXT: &str = "alpha beta\n\nsecond `code` here\n\n```\nlet a = 1;\n```\n\nlast";

fn draw<'a>(cx: &'a mut TestAppContext, dir: &std::path::Path) -> &'a mut VisualTestContext {
    show(cx, dir, TEXT)
}

fn show<'a>(
    cx: &'a mut TestAppContext,
    dir: &std::path::Path,
    text: &str,
) -> &'a mut VisualTestContext {
    open_roles(cx, dir, 2400.0, &[("user", "hi"), ("assistant", text)])
}

/// A point of block `path`: `x` and `y` are shares of its box.
fn spot(cx: &mut VisualTestContext, path: &str, (x, y): (f32, f32)) -> Point<Pixels> {
    let block = at(cx, &format!("msg-c1-md-{path}")).unwrap_or_else(|| panic!("no block {path}"));
    block.origin + point(block.size.width * x, block.size.height * y)
}

/// A point `x` and `y` inside the scroller's far corner.
fn corner(cx: &mut VisualTestContext, (x, y): (f32, f32)) -> Point<Pixels> {
    let scroller = at(cx, "session-scroll").unwrap();
    point(scroller.right() - px(x), scroller.bottom() - px(y))
}

fn press(cx: &mut VisualTestContext, at: Point<Pixels>) {
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
}

fn lift(cx: &mut VisualTestContext, at: Point<Pixels>) {
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
}

/// A press, a move and a release, the selection as it stood.
fn pull(cx: &mut VisualTestContext, from: Point<Pixels>, to: Point<Pixels>) {
    press(cx, from);
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    lift(cx, to);
}

/// A drag from no selection: a click in the blank corner drops the last one.
fn drag(cx: &mut VisualTestContext, from: Point<Pixels>, to: Point<Pixels>) {
    let blank = corner(cx, (2.0, 2.0));
    press(cx, blank);
    lift(cx, blank);
    pull(cx, from, to);
}

fn held(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|_, cx| picked(cx))
}

#[gpui::test]
fn a_drag_selects_from_the_press_to_the_pointer(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = draw(cx, dir.path());
    let (start, end) = (spot(cx, "0", (0.0, 0.5)), spot(cx, "0", (1.0, 0.5)));
    let quads = cx.update(|window, _| window.painted_quads().len());
    drag(cx, start, end);
    assert_eq!(held(cx), ["alpha beta"]);
    // One run's highlight is one quad under its text.
    assert_eq!(
        cx.update(|window, _| window.painted_quads().len()),
        quads + 1
    );
    // Backwards, and over a face's seam: each run holds its own part.
    let (from, to) = (spot(cx, "1", (1.0, 0.5)), spot(cx, "0", (0.0, 0.5)));
    drag(cx, from, to);
    assert_eq!(held(cx), ["alpha beta", "second ", "code", " here"]);
    // Through a code block to the last block's end.
    let under = corner(cx, (300.0, 2.0));
    drag(cx, start, under);
    let all = [
        "alpha beta",
        "second ",
        "code",
        " here",
        "let a = 1;",
        "last",
    ];
    assert_eq!(held(cx), all);
}

#[gpui::test]
fn a_point_off_the_text_selects_as_blink_does(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = draw(cx, dir.path());
    let start = spot(cx, "0", (0.0, 0.5));
    // The margin between two blocks is the next block's start.
    let (first, second) = (spot(cx, "0", (0.5, 1.0)), spot(cx, "1", (0.5, 0.0)));
    let margin = point(first.x, (first.y + second.y) / 2.0);
    drag(cx, start, margin);
    assert_eq!(held(cx), ["alpha beta"]);
    // Left of the column a row's start, right of it the row's end.
    let row = spot(cx, "1", (0.5, 0.5));
    let left = at(cx, "session-scroll").unwrap().left();
    drag(cx, start, point(left + px(1.0), row.y));
    assert_eq!(held(cx), ["alpha beta"]);
    let right = corner(cx, (2.0, 2.0)).x;
    drag(cx, start, point(right, row.y));
    assert_eq!(held(cx), ["alpha beta", "second ", "code", " here"]);
    // Over all the text its start: a press under it all selects back to there.
    let (under, top) = (
        corner(cx, (300.0, 2.0)),
        at(cx, "session-scroll").unwrap().top(),
    );
    drag(cx, under, point(row.x, top + px(1.0)));
    // The user's message over the answer is text too, and its block's end.
    assert_eq!(held(cx).len(), 8);
}

#[gpui::test]
fn a_press_drops_the_selection_and_a_press_on_it_holds_it(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = draw(cx, dir.path());
    let (start, end) = (spot(cx, "0", (0.0, 0.5)), spot(cx, "0", (1.0, 0.5)));
    let (on, far) = (start + point(px(12.0), px(0.0)), spot(cx, "3", (0.5, 0.5)));
    drag(cx, start, end);
    // Blink drags the selected text: the selection stays through the move.
    pull(cx, on, far);
    assert_eq!(held(cx), ["alpha beta"]);
    // A click on it drops it when the button lifts.
    press(cx, on);
    assert_eq!(held(cx), ["alpha beta"]);
    lift(cx, on);
    assert!(held(cx).is_empty());
    // A press elsewhere drops it at once.
    drag(cx, start, end);
    press(cx, far);
    assert!(held(cx).is_empty());
    lift(cx, far);
    // A panel that floats drops what its long press selected.
    pull(cx, start, end);
    cx.update(|window, cx| {
        super::unselect(cx);
        window.refresh();
    });
    cx.run_until_parked();
    assert!(held(cx).is_empty());
}

fn place(id: &'static str, at: usize) -> Option<Place> {
    Some(Place { id: id.into(), at })
}

#[test]
fn runs_hold_what_lies_between_the_two_ends() {
    let cover = |anchor, focus, runs: &[(&'static str, usize, usize)]| {
        let mut selection = Selection {
            anchor,
            focus,
            ..Selection::default()
        };
        let held = runs
            .iter()
            .map(|(id, start, len)| selection.cover(&(*id).into(), *start, *len));
        held.collect::<Vec<_>>()
    };
    let runs = [("a", 0, 5), ("a", 6, 4), ("b", 0, 3)];
    // Forwards and backwards alike, in bytes of each run.
    let wanted = [Some(2..5), Some(0..4), Some(0..1)];
    assert_eq!(cover(place("a", 2), place("b", 1), &runs), wanted);
    assert_eq!(cover(place("b", 1), place("a", 2), &runs), wanted);
    assert_eq!(
        cover(place("a", 1), place("a", 3), &runs),
        [Some(1..3), None, None]
    );
    // A collapsed selection holds nothing; nor does one whose block is gone.
    assert_eq!(
        cover(place("a", 2), place("a", 2), &runs),
        [None, None, None]
    );
    assert_eq!(
        cover(place("z", 0), place("z", 2), &runs),
        [None, None, None]
    );
    // A point in the space a wrap dropped counts at the next row's start.
    assert_eq!(
        cover(place("a", 5), place("a", 8), &runs),
        [None, Some(0..2), None]
    );
    // A point at a seam is met once.
    let seam = [("a", 0, 5), ("a", 5, 4)];
    assert_eq!(
        cover(place("a", 5), place("a", 7), &seam),
        [None, Some(0..2)]
    );
}

#[gpui::test]
fn a_code_blocks_tab_and_empty_row_are_places(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = show(cx, dir.path(), "```\na\tb\n\nc\n```\n");
    let (first, last) = (spot(cx, "0", (0.0, 0.0)), spot(cx, "0", (0.0, 1.0)));
    // The block's padding is 40 over and 16 about; its rows are 21 tall.
    let row = |nth: f32| first + point(px(17.0), px(40.0 + 21.0 * nth + 10.0));
    let quads = cx.update(|window, _| window.painted_quads().len());
    drag(cx, row(0.0), point(last.x + px(600.0), row(0.0).y));
    // The tab is a byte of its own, lit over its advance.
    assert_eq!(held(cx), ["a", "\t", "b"]);
    assert_eq!(
        cx.update(|window, _| window.painted_quads().len()),
        quads + 3
    );
    // The empty row is a place: a drag to it stops before the next row.
    drag(cx, row(0.0), row(1.0));
    assert_eq!(held(cx), ["a", "\t", "b"]);
    drag(cx, row(1.0), row(2.0) + point(px(600.0), px(0.0)));
    assert_eq!(held(cx), ["c"]);
}

#[gpui::test]
fn a_pointer_in_a_tables_cell_selects_in_that_cell(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let long = "longword ".repeat(400);
    let cx = show(
        cx,
        dir.path(),
        &format!("| h1 | h2 |\n| --- | --- |\n| s | {long} |\n"),
    );
    let short = at(cx, "msg-c1-md-0-r1c0").expect("the first body cell");
    let tall = at(cx, "msg-c1-md-0-r1c1").expect("the second body cell");
    assert!(
        short.size.height > px(60.0),
        "the long cell wraps: {short:?}"
    );
    // Under the short cell's text, in its box: its own text's end, not the cell beside it.
    let from = short.origin + point(px(13.0), px(18.0));
    let under = point(
        short.left() + short.size.width / 2.0,
        short.bottom() - px(10.0),
    );
    drag(cx, from, under);
    assert_eq!(held(cx), ["s"]);
    // In the long cell the same height selects there.
    drag(cx, from, point(tall.left() + px(40.0), under.y));
    assert!(held(cx).len() > 1);
}

fn run(cells: Vec<Cell>, picked: Option<std::ops::Range<usize>>) -> Run {
    Run {
        id: "a".into(),
        start: 0,
        text: "".into(),
        bounds: gpui::Bounds::default(),
        line: px(0.0)..px(0.0),
        scope: None,
        cells,
        picked,
        card: None,
        item: false,
    }
}

#[test]
fn a_glyph_is_hit_at_its_nearer_edge() {
    // `abcd`: a pointer in the last glyph's first half stays before it.
    let ltr = run(
        cells(vec![(0, 0.0), (1, 9.0), (2, 18.0), (3, 26.0)], "abcd", 35.0),
        None,
    );
    assert_eq!(
        [27.0, 28.4, 30.4, 31.0, 99.0].map(|x| ltr.hit(x)),
        [3, 3, 3, 4, 4]
    );
    assert_eq!([-5.0, 4.0, 5.0].map(|x| ltr.hit(x)), [0, 0, 1]);
    // No glyph: the run's one place.
    assert_eq!(run(Vec::new(), None).hit(3.0), 0);
}

#[test]
fn a_right_to_left_line_is_hit_and_lit_where_its_glyphs_stand() {
    // Two words of two letters, two bytes each, about a space: the first stands at the right.
    let glyphs = vec![(0, 40.0), (2, 30.0), (4, 25.0), (5, 10.0), (7, 0.0)];
    let hebrew: String = [0x5d0, 0x5d1, 0x20, 0x5d2, 0x5d3]
        .iter()
        .filter_map(|c| char::from_u32(*c))
        .collect();
    let found = cells(glyphs, &hebrew, 50.0);
    assert!(found.iter().all(|cell| cell.rtl));
    assert_eq!(
        found[0],
        Cell {
            bytes: 7..9,
            x: 0.0..10.0,
            rtl: true
        }
    );
    let rtl = run(found.clone(), None);
    // The line's right end is its start; a glyph's right edge is its first byte.
    assert_eq!(
        [99.0, 46.0, 44.0, 31.0, 26.0, 1.0, -9.0].map(|x| rtl.hit(x)),
        [0, 0, 2, 4, 5, 9, 9]
    );
    // The first word lights the right stretch; all of it one stretch, the line wide.
    let word = run(found.clone(), Some(0..4));
    assert_eq!(word.stretches(&[]), [(30.0..50.0, 1.0)]);
    assert!(word.lit(45.0) && !word.lit(5.0));
    assert_eq!(run(found, Some(0..9)).stretches(&[]), [(0.0..50.0, 1.0)]);
}

#[test]
fn the_highlight_fades_with_its_word() {
    let found = cells(
        vec![(0, 0.0), (1, 8.0), (2, 16.0), (3, 20.0), (4, 28.0)],
        "ab cd",
        36.0,
    );
    let lit = run(found, Some(0..5));
    let fades = [(0..2, 0.0), (3..5, 0.25)];
    assert_eq!(
        lit.stretches(&fades),
        [(0.0..16.0, 0.0), (16.0..20.0, 1.0), (20.0..36.0, 0.25)]
    );
}

#[test]
fn a_mixed_line_reads_each_letter_by_its_own_direction() {
    // `abc`, two Hebrew letters and `def` (`logs/review/9.6k1-r2-review-extra.log`).
    let hebrew: String = [0x5d0, 0x5d1]
        .iter()
        .filter_map(|c| char::from_u32(*c))
        .collect();
    let text = format!("abc {hebrew} def");
    let glyphs = vec![
        (0, 0.0),
        (1, 8.42),
        (2, 17.6),
        (3, 26.18),
        (6, 30.4),
        (4, 38.98),
        (8, 48.46),
        (9, 52.67),
        (10, 61.86),
        (11, 70.6),
    ];
    let found = cells(glyphs, &text, 76.16);
    let rtl: Vec<usize> = found
        .iter()
        .filter(|cell| cell.rtl)
        .map(|cell| cell.bytes.start)
        .collect();
    assert_eq!(rtl, [6, 4]);
    // The run's last letter stands at its left: its right edge is its start.
    let mixed = run(found, None);
    assert_eq!(
        [31.0, 37.0, 40.0, 47.0, 49.0].map(|x| mixed.hit(x)),
        [8, 6, 6, 4, 8]
    );
}

#[test]
fn a_ligatures_letters_share_its_box() {
    // `office` in DejaVu Sans: `ffi` is one glyph.
    let found = cells(
        vec![(0, 0.0), (1, 9.18), (4, 23.68), (5, 31.9)],
        "office",
        41.0,
    );
    let starts: Vec<usize> = found.iter().map(|cell| cell.bytes.start).collect();
    assert_eq!(starts, [0, 1, 2, 3, 4, 5]);
    let office = run(found, Some(0..2));
    assert_eq!(
        [10.0, 14.98, 17.0, 20.0, 23.0].map(|x| office.hit(x)),
        [1, 2, 3, 3, 4]
    );
    let lit = office.stretches(&[]);
    assert!(
        (lit[0].0.end - (9.18 + 14.5 / 3.0)).abs() < 0.001,
        "{lit:?}"
    );
    // A mark shaped apart from its letter is no place.
    let marked: String = ['e', char::from_u32(0x301).expect("a mark"), 'x']
        .iter()
        .collect();
    let found = cells(vec![(0, 0.0), (1, 4.0), (3, 8.0)], &marked, 16.0);
    assert_eq!(
        found
            .iter()
            .map(|cell| cell.bytes.clone())
            .collect::<Vec<_>>(),
        [0..3, 3..4]
    );
}
