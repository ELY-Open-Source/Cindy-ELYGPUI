use gpui::{
    Bounds, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Pixels, Point, SharedString,
    TestAppContext, VisualTestContext, point, px, size,
};

use super::super::{Run, Selection, granular::Block};
use super::{Place, draw, held, show};

/// The middle of byte `at` of the drawn run whose text holds `needle`, counted from the needle.
fn glyph(cx: &mut VisualTestContext, needle: &str, at: usize) -> Point<Pixels> {
    cx.update(|_, cx| {
        let runs = &cx.global::<Selection>().runs;
        let run = runs.iter().find(|run| run.text.contains(needle));
        let run = run.unwrap_or_else(|| panic!("no run holds {needle:?}"));
        let byte = run.text.find(needle).unwrap() + at;
        let cell = run
            .cells
            .iter()
            .find(|cell| cell.bytes.contains(&byte))
            .unwrap();
        let x = run.bounds.left() + px((cell.x.start + cell.x.end) / 2.0);
        point(x, run.bounds.center().y)
    })
}

/// A press and release on byte `at` of the run that holds `needle`.
fn click(cx: &mut VisualTestContext, (needle, at): (&str, usize), clicks: usize, shift: bool) {
    let at = glyph(cx, needle, at);
    let modifiers = Modifiers {
        shift,
        ..Modifiers::none()
    };
    let button = MouseButton::Left;
    cx.simulate_event(MouseDownEvent {
        position: at,
        modifiers,
        button,
        click_count: clicks,
        first_mouse: false,
    });
    cx.simulate_event(MouseUpEvent {
        position: at,
        modifiers,
        button,
        click_count: clicks,
    });
    cx.run_until_parked();
}

/// A single, a double and a triple click, as the platform counts them.
fn clicks(cx: &mut VisualTestContext, at: (&str, usize), count: usize) {
    (1..=count).for_each(|n| click(cx, at, n, false));
}

#[gpui::test]
fn a_double_click_selects_a_word_and_a_triple_click_its_paragraph(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = draw(cx, dir.path());
    clicks(cx, ("beta", 1), 2);
    assert_eq!(held(cx), ["beta"]);
    // A word goes on through a code's face; the space is a word of its own.
    clicks(cx, ("alpha", 2), 3);
    assert_eq!(held(cx), ["alpha beta"]);
    clicks(cx, ("let a", 0), 3);
    assert_eq!(held(cx), ["let a = 1;"]);
    clicks(cx, ("let a", 3), 2);
    assert_eq!(held(cx), [" "]);
    // A double click on what is lit selects its word anew.
    clicks(cx, ("alpha", 2), 3);
    clicks(cx, ("beta", 0), 2);
    assert_eq!(held(cx), ["beta"]);
}

#[gpui::test]
fn a_drag_after_a_double_click_goes_by_words(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = draw(cx, dir.path());
    let (alpha, beta) = (glyph(cx, "alpha", 2), glyph(cx, "beta", 1));
    click(cx, ("alpha", 2), 1, false);
    let down = MouseDownEvent {
        position: alpha,
        modifiers: Modifiers::none(),
        button: MouseButton::Left,
        click_count: 2,
        first_mouse: false,
    };
    cx.simulate_event(down);
    cx.simulate_mouse_move(beta, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(held(cx), ["alpha beta"]);
    // Back over the first word: its own ends, whatever the pointer's byte.
    let back = glyph(cx, "alpha", 1);
    cx.simulate_mouse_move(back, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(held(cx), ["alpha"]);
    cx.simulate_mouse_up(alpha, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
}

#[gpui::test]
fn shift_with_a_click_extends_in_the_selections_unit(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = draw(cx, dir.path());
    // With nothing selected it places a caret; then the focus moves by characters.
    click(cx, ("alpha", 1), 1, true);
    assert!(held(cx).is_empty());
    click(cx, ("beta", 1), 1, true);
    assert_eq!(held(cx), ["pha be"]);
    // After a double click it moves by words, and back over the anchor from the word's end.
    clicks(cx, ("beta", 2), 2);
    click(cx, ("alpha", 3), 1, true);
    assert_eq!(held(cx), ["alpha beta"]);
    // Shift with a double click on a range changes nothing.
    click(cx, ("second", 1), 2, true);
    assert_eq!(held(cx), ["alpha beta"]);
}

/// A press of `clicks` with Shift on byte `at` of the run that holds `needle`, held down.
fn press(cx: &mut VisualTestContext, (needle, at): (&str, usize), clicks: usize, shift: bool) {
    let position = glyph(cx, needle, at);
    cx.simulate_event(MouseDownEvent {
        position,
        modifiers: Modifiers {
            shift,
            ..Modifiers::none()
        },
        button: MouseButton::Left,
        click_count: clicks,
        first_mouse: false,
    });
    cx.run_until_parked();
}

fn drag(cx: &mut VisualTestContext, to: (&str, usize)) {
    let to = glyph(cx, to.0, to.1);
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
}

#[gpui::test]
fn shift_with_more_clicks_goes_as_blink_dispatches_them(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = show(cx, dir.path(), "alpha beta\n\nsecond words\n\nRight");
    // `logs/review/9.6k2-r1-review-blink.log`: a triple click selects whatever Shift holds.
    clicks(cx, ("beta", 1), 2);
    (1..=3).for_each(|n| click(cx, ("second", 1), n, true));
    assert_eq!(held(cx), ["second words"]);
    // A double click on a range keeps it, and its drag extends by words.
    clicks(cx, ("beta", 1), 2);
    click(cx, ("second", 1), 1, true);
    press(cx, ("second", 1), 2, true);
    drag(cx, ("Right", 1));
    assert_eq!(held(cx), ["beta", "second words", "Right"]);
}

#[gpui::test]
fn shift_on_a_link_places_a_caret(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = show(
        cx,
        dir.path(),
        "alpha beta\n\nLeft [Link](https://e.test/x) Right",
    );
    clicks(cx, ("beta", 1), 2);
    press(cx, ("Link", 1), 1, true);
    assert!(held(cx).is_empty(), "{:?}", held(cx));
}

#[gpui::test]
fn a_word_drag_goes_back_through_a_chip(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = show(cx, dir.path(), "Aa [Bb](cindy://session/abc123) Cc");
    // The paragraph's runs stand on both sides of the chip's label.
    click(cx, ("Cc", 0), 1, false);
    press(cx, ("Cc", 0), 2, false);
    drag(cx, ("Bb", 0));
    assert_eq!(held(cx), ["Bb", " Cc"]);
}

#[gpui::test]
fn mod_a_selects_the_whole_stream(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = show(cx, dir.path(), "one\n\ntwo");
    click(cx, ("two", 1), 1, false);
    cx.simulate_keystrokes(super::super::SELECT_ALL);
    let all = held(cx);
    assert_eq!(all.first().map(String::as_str), Some("hi"));
    assert!(all.ends_with(&["one".to_owned(), "two".to_owned()]));
}

/// A run of block `id` from byte `start`, on row `row`.
fn run(id: &str, start: usize, text: &str, row: usize) -> Run {
    let top = px(20.0 * row as f32);
    Run {
        id: SharedString::from(id.to_owned()),
        start,
        text: SharedString::from(text.to_owned()),
        bounds: Bounds::new(point(px(0.0), top), size(px(100.0), px(20.0))),
        line: top..top + px(20.0),
        scope: None,
        cells: Vec::new(),
        picked: None,
        card: None,
        item: false,
    }
}

/// A selection of `blocks`, each a row of one run: `(id, text, cuts)`.
fn laid(blocks: &[(&str, &str, &[usize])]) -> Selection {
    let mut selection = Selection::default();
    for (n, (id, text, cuts)) in blocks.iter().enumerate() {
        selection.runs.push(run(id, 0, text, n));
        selection.blocks.push(Block::new(id, text, cuts.to_vec()));
    }
    selection
}

/// The word a hit on byte `at` of block `id` selects, on the row of the first run holding it.
fn word(selection: &Selection, id: &str, at: usize) -> (usize, usize) {
    let id = SharedString::from(id.to_owned());
    let holds = |run: &&Run| run.id == id && (run.start..=run.start + run.text.len()).contains(&at);
    let row = selection
        .runs
        .iter()
        .find(holds)
        .expect("a run holds the hit");
    let (start, end) = selection.word(&Place { id, at }, row.line.start + px(1.0));
    (start.at, end.at)
}

#[test]
fn a_word_is_icus_and_a_paragraphs_end_its_break() {
    let text = "end. (1.5 kg; don't x_y)\nnext";
    let selection = laid(&[("a", text, &[25]), ("b", "Last here.", &[])]);
    // `logs/review/9.6k2-blink-clicks.log`: "1.5", "don't" and "x_y" are words.
    assert_eq!(word(&selection, "a", 7), (6, 9));
    assert_eq!(word(&selection, "a", 15), (14, 19));
    assert_eq!(word(&selection, "a", 21), (20, 23));
    // A space is a word; a click before the line feed selects it.
    assert_eq!(word(&selection, "a", 4), (4, 5));
    assert_eq!(word(&selection, "a", 24), (24, 25));
    // The end of all the text takes the word before it.
    assert_eq!(word(&selection, "b", 10), (9, 10));
    assert_eq!(word(&selection, "b", 0), (0, 4));
}

#[gpui::test]
fn a_links_menu_anchor_and_a_break_end_a_paragraph(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let text = "Aa [Bb](https://e.test/x) Cc\n\nDd Ee  \nFf";
    let cx = show(cx, dir.path(), text);
    // `logs/review/9.6k2-blink-kinds.log`: the anchor after a web link is a block.
    clicks(cx, ("Aa", 0), 3);
    assert_eq!(held(cx), ["Aa ", "Bb"]);
    clicks(cx, ("Cc", 1), 3);
    assert_eq!(held(cx), [" Cc"]);
    // A `br` ends its paragraph; the next starts after it.
    clicks(cx, ("Dd", 0), 3);
    assert_eq!(held(cx), ["Dd Ee"]);
    clicks(cx, ("Ff", 0), 3);
    assert_eq!(held(cx), ["Ff"]);
}

#[test]
fn a_wrapped_rows_end_takes_the_word_before_it() {
    let mut selection = laid(&[("b", "z", &[])]);
    // The block's first row ends after "two"; the wrap dropped the space.
    selection
        .runs
        .splice(0..0, [run("a", 0, "one two", 0), run("a", 8, "three", 1)]);
    selection.runs[2] = run("b", 0, "z", 2);
    selection
        .blocks
        .push(Block::new("a", "one two three", Vec::new()));
    assert_eq!(word(&selection, "a", 7), (4, 7));
    assert_eq!(word(&selection, "a", 4), (4, 7));
}

#[test]
fn a_new_rows_first_glyph_takes_its_own_word() {
    let mut selection = laid(&[("a", "中国日本美国英国", &[]), ("b", "last", &[])]);
    let mut second = run("a", 12, "美国英国", 1);
    let glyphs = vec![(0, 0.0), (3, 16.0), (6, 32.0), (9, 48.0)];
    second.cells = super::super::cells(glyphs, "美国英国", 64.0);
    selection.runs = vec![run("a", 0, "中国日本", 0), second, run("b", 0, "last", 2)];
    // `logs/review/9.6k2-r1-review-blink.log`: the row's left quarter of 美 selects 美国.
    selection.down(point(px(4.0), px(25.0)), 2, false);
    let ends = |selection: &Selection| (selection.anchor.clone(), selection.focus.clone());
    let place = |at| Place {
        id: SharedString::from("a"),
        at,
    };
    assert_eq!(ends(&selection), (Some(place(12)), Some(place(18))));
    // The same byte hit from the row above ends that row's word.
    assert_eq!(selection.word(&place(12), px(5.0)), (place(6), place(12)));
}

#[test]
fn a_drag_by_paragraphs_keeps_the_first_whole() {
    let mut selection = laid(&[("a", "one\ntwo\nthree", &[4, 8]), ("b", "last", &[])]);
    let place = |id: &str, at| Place {
        id: SharedString::from(id.to_owned()),
        at,
    };
    selection.pick(place("a", 5), px(0.0), super::super::Unit::Paragraph);
    selection.extend(Some(place("b", 2)), px(0.0));
    let ends = |selection: &Selection| (selection.anchor.clone(), selection.focus.clone());
    assert_eq!(ends(&selection), (Some(place("a", 4)), Some(place("b", 4))));
    selection.extend(Some(place("a", 1)), px(0.0));
    assert_eq!(ends(&selection), (Some(place("a", 8)), Some(place("a", 0))));
}

#[test]
fn windows_trailing_spaces_stop_at_a_line_feed_and_take_a_no_break_space() {
    use super::super::granular::skip_spaces;
    assert_eq!(skip_spaces("one  two", 3), 5);
    assert_eq!(skip_spaces("one \u{a0}\u{3000}two", 3), 9);
    assert_eq!(skip_spaces("one \nnext", 3), 4);
    assert_eq!(skip_spaces("one", 3), 3);
}

#[gpui::test]
fn an_orca_line_is_one_paragraph(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let content = r#"{"orcaSource":"lead","content":"do it now\nsecond line"}"#;
    let cx = crate::session_view::user_row::tests::open(cx, dir.path(), 2400.0, &[(content, None)]);
    let header = crate::session_view::user_row::tests::at(cx, "msg-c0-orca-head").unwrap();
    cx.simulate_click(header.center(), Modifiers::none());
    crate::session_view::user_row::tests::settle(cx);
    clicks(cx, ("do it now", 1), 3);
    assert_eq!(held(cx), ["do it now", "\n"]);
}

#[gpui::test]
fn a_user_chip_stops_the_paragraph(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = crate::session_view::user_row::tests::open_roles(
        cx,
        dir.path(),
        2400.0,
        &[("user", "Aa cindy://session/abc123 Cc")],
    );
    clicks(cx, ("Aa", 0), 3);
    assert_eq!(held(cx), ["Aa "]);
    clicks(cx, ("Cc", 0), 3);
    assert_eq!(held(cx), [" Cc"]);
}

#[gpui::test]
fn a_press_on_an_image_button_keeps_the_selection(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    crate::session_view::images::tests::put(dir.path(), "wide.webp", (2000, 300));
    let cx = show(
        cx,
        dir.path(),
        "alpha beta\n\none ![](xdt-image://s1/wide.webp) two\n\nlast",
    );
    let image = crate::session_view::user_row::tests::at(cx, "msg-c1-g0-img").unwrap();
    let button = crate::session_view::user_row::tests::at(cx, "msg-c1-g0").unwrap();
    // `logs/review/9.6k2-r3-blink-blank.log`: the blank and the image alike.
    let blank = point(image.right() + px(20.0), image.center().y);
    assert!(button.contains(&blank));
    for (position, shift) in [(blank, false), (blank, true), (image.center(), true)] {
        clicks(cx, ("beta", 1), 2);
        cx.simulate_event(MouseDownEvent {
            position,
            modifiers: Modifiers {
                shift,
                ..Modifiers::none()
            },
            button: MouseButton::Left,
            click_count: 1,
            first_mouse: false,
        });
        cx.run_until_parked();
        assert_eq!(held(cx), ["beta"], "{position:?} shift {shift}");
    }
}

#[gpui::test]
fn mod_a_reaches_the_offscreen_tail(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let rows: Vec<_> = (0..80).map(|nth| format!("para{nth:03}")).collect();
    let cx = show(cx, dir.path(), &rows.join("\n\n"));
    cx.simulate_keystrokes(super::super::SELECT_ALL);
    assert_eq!(held(cx).last().map(String::as_str), Some("para079"));
}

#[gpui::test]
fn the_last_user_words_end_takes_that_word(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = crate::session_view::user_row::tests::open_roles(
        cx,
        dir.path(),
        2400.0,
        &[("user", "hello")],
    );
    clicks(cx, ("hello", 4), 2);
    assert_eq!(held(cx), ["hello"]);
}

#[gpui::test]
fn mod_a_ends_at_the_last_user_text(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = crate::session_view::user_row::tests::open_roles(
        cx,
        dir.path(),
        2400.0,
        &[("user", "hello")],
    );
    cx.simulate_keystrokes(super::super::SELECT_ALL);
    assert_eq!(held(cx), ["hello"]);
}

#[gpui::test]
fn a_triple_click_ends_at_the_last_user_text(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = crate::session_view::user_row::tests::open_roles(
        cx,
        dir.path(),
        2400.0,
        &[("user", "hello")],
    );
    clicks(cx, ("hello", 1), 3);
    assert_eq!(held(cx), ["hello"]);
}

/// A press and release of each count at `position`.
fn tap(cx: &mut VisualTestContext, position: Point<Pixels>, count: usize) {
    for clicks in 1..=count {
        let (modifiers, button) = (Modifiers::none(), MouseButton::Left);
        cx.simulate_event(MouseDownEvent {
            position,
            modifiers,
            button,
            click_count: clicks,
            first_mouse: false,
        });
        cx.simulate_event(MouseUpEvent {
            position,
            modifiers,
            button,
            click_count: clicks,
        });
    }
    cx.run_until_parked();
}

/// Under a user text that ends with a blank line.
fn below_blank_line<'a>(
    cx: &'a mut TestAppContext,
    dir: &std::path::Path,
) -> (&'a mut VisualTestContext, Point<Pixels>) {
    let cx =
        crate::session_view::user_row::tests::open_roles(cx, dir, 2400.0, &[("user", "hello\n\n")]);
    let body = crate::session_view::user_row::tests::at(cx, "msg-c0-text").unwrap();
    (cx, point(body.left() + px(10.0), body.bottom() + px(20.0)))
}

#[gpui::test]
fn below_a_closing_blank_line_a_double_click_takes_its_feed(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (cx, below) = below_blank_line(cx, dir.path());
    // `logs/review/9.6k2-r3-review-blink-ends.log`: `PreviousPositionOf` the content's end.
    tap(cx, below, 2);
    assert_eq!(held(cx), ["\n"]);
}

#[gpui::test]
fn below_a_closing_blank_line_a_triple_click_takes_the_paragraph_before(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (cx, below) = below_blank_line(cx, dir.path());
    tap(cx, below, 3);
    assert_eq!(held(cx), ["hello", "\n"]);
}

#[gpui::test]
fn mod_a_leaves_a_focused_field_its_own(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = show(cx, dir.path(), "alpha beta");
    let title = cx.debug_bounds("session-header-title").unwrap().center();
    tap(cx, title, 2);
    assert!(cx.debug_bounds("task-rename").is_some());
    cx.simulate_input("draft");
    cx.simulate_keystrokes("left");
    cx.simulate_keystrokes(super::super::SELECT_ALL);
    assert!(held(cx).is_empty());
    cx.simulate_input("X");
    cx.simulate_keystrokes("enter");
    let saved = cx.update(|_, cx| crate::session_view::header::task(cx).unwrap().title.clone());
    assert_eq!(saved, "X");
}

#[gpui::test]
fn below_a_closing_chip_a_double_click_takes_its_last_word(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = crate::session_view::user_row::tests::open_roles(
        cx,
        dir.path(),
        2400.0,
        &[("user", "Aa cindy://session/abc123")],
    );
    let body = crate::session_view::user_row::tests::at(cx, "msg-c0-text").unwrap();
    let below = point(body.left() + px(10.0), body.bottom() + px(20.0));
    // `logs/review/9.6k2-r4-review-blink-chips.log`: the label's last word, then the label.
    tap(cx, below, 2);
    assert_eq!(held(cx), ["session"]);
    tap(cx, below, 3);
    assert_eq!(held(cx), ["A session"]);
}
