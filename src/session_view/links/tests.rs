//! The links of a user message: where they lie, what a click, a key and the menu do.

use gpui::{
    KeyDownEvent, KeyUpEvent, Keystroke, Modifiers, MouseButton, TestAppContext, VisualTestContext,
    point, px,
};

use super::super::user_row::tests::{at, lines, open, settle};
use crate::{open_external::tests as opener, toast::Toasts};

const WEB: &str = "https://example.com/a";
const LOCAL: &str = "http://localhost:3000/";
const FIRST: &str = "msg-c0-link-0-0";

fn toasts(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|window, cx| {
        let items = cx
            .global::<Toasts>()
            .items(window.window_handle().window_id());
        items.iter().map(|item| item.message.to_string()).collect()
    })
}

fn click(cx: &mut VisualTestContext, selector: &str, modifiers: Modifiers) {
    let centre = at(cx, selector).unwrap().center();
    cx.simulate_click(centre, modifiers);
    settle(cx);
}

fn press(cx: &mut VisualTestContext, selector: &str, button: MouseButton) {
    let centre = at(cx, selector).unwrap().center();
    cx.simulate_mouse_down(centre, button, Modifiers::none());
    cx.simulate_mouse_up(centre, button, Modifiers::none());
    settle(cx);
}

/// A key down and up, where gpui's keyboard click would land.
fn key(cx: &mut VisualTestContext, name: &'static str) {
    let keystroke = Keystroke::parse(name).unwrap();
    cx.simulate_event(KeyDownEvent {
        keystroke: keystroke.clone(),
        is_held: false,
        prefer_character_input: false,
    });
    cx.simulate_event(KeyUpEvent { keystroke });
    settle(cx);
}

/// A press that ends off the link: it takes focus and clicks nothing.
fn focus(cx: &mut VisualTestContext, selector: &str) {
    let link = at(cx, selector).unwrap();
    cx.simulate_mouse_down(link.center(), MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(
        point(px(2.0), px(300.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    settle(cx);
    assert_eq!(opener::take(), Vec::<String>::new());
}

#[gpui::test]
fn a_link_lies_over_its_text(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let body = format!("see {WEB} now\r\n{LOCAL}");
    let cx = open(cx, dir.path(), 2400.0, &[(&body, None), ("no link", None)]);
    let text = at(cx, "msg-c0-text").unwrap();
    let (first, second) = (at(cx, FIRST).unwrap(), at(cx, "msg-c0-link-1-0").unwrap());
    assert!(first.left() > text.left(), "after `see `");
    assert_eq!(first.top(), text.top());
    assert_eq!(first.size.height, px(24.0));
    assert!(first.right() < text.right(), "before ` now`");
    assert_eq!(second.left(), text.left(), "the CR moves no link");
    assert_eq!(second.top(), text.top() + px(24.0));
    assert_eq!(second.right(), text.left() + second.size.width);
    assert!(second.size.width > px(100.0));
    assert!(at(cx, "msg-c0-link-0-1").is_none(), "one line, one box");
    assert!(at(cx, "msg-c1-link-0-0").is_none());
}

#[gpui::test]
fn a_link_past_a_hung_space_keeps_its_place(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let body = format!("{} {WEB}", "a ".repeat(60));
    let cx = open(cx, dir.path(), 2400.0, &[(&body, None), (WEB, None)]);
    let (moved, alone) = (at(cx, FIRST).unwrap(), at(cx, "msg-c1-link-0-0").unwrap());
    assert_eq!(moved.size, alone.size, "the same text, the same box");
    let text = at(cx, "msg-c0-text").unwrap();
    assert!(moved.top() > text.top(), "on a later line");
    assert!(moved.bottom() <= text.bottom());
}

#[gpui::test]
fn a_wrapped_link_has_a_box_a_line(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    // Blink breaks an address after its `?`, not after a `/`.
    let body = format!("go {WEB}?{}", "path".repeat(60));
    let cx = open(cx, dir.path(), 2400.0, &[(&body, None)]);
    let text = at(cx, "msg-c0-text").unwrap();
    let (first, second) = (at(cx, FIRST).unwrap(), at(cx, "msg-c0-link-0-1").unwrap());
    assert!(first.left() > text.left());
    assert_eq!(second.left(), text.left());
    assert_eq!(second.top(), first.bottom());
    assert!(first.right() <= text.right() && second.right() <= text.right());
    // Either line's box opens the one address, and a press on either focuses the link.
    click(cx, "msg-c0-link-0-1", Modifiers::none());
    assert_eq!(opener::take().len(), 1);
    cx.update(|window, cx| window.blur(cx));
    focus(cx, "msg-c0-link-0-1");
    key(cx, "space");
    assert_eq!(opener::take().len(), 1);
}

#[gpui::test]
fn a_link_alone_fills_the_text(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), 2400.0, &[(WEB, None)]);
    assert_eq!(at(cx, FIRST), at(cx, "msg-c0-text"));
}

#[gpui::test]
fn a_folded_message_draws_no_link(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let body = format!("{WEB}\n{}", lines(15));
    let cx = open(cx, dir.path(), 2400.0, &[(&body, None)]);
    assert!(at(cx, FIRST).is_none());
    click(cx, "msg-c0-fold", Modifiers::none());
    assert!(at(cx, FIRST).is_some());
}

#[gpui::test]
fn a_click_opens_by_the_kind_of_address(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let body = format!("{WEB} {LOCAL}");
    let cx = open(cx, dir.path(), 2400.0, &[(&body, None)]);
    click(cx, FIRST, Modifiers::none());
    assert_eq!(opener::take(), [WEB]);
    assert_eq!(toasts(cx), Vec::<String>::new());
    // A local page goes to the sidebar browser, which 17.0 brings.
    click(cx, "msg-c0-link-1-0", Modifiers::none());
    assert_eq!(opener::take(), Vec::<String>::new());
    assert_eq!(toasts(cx), ["Failed to open in sidebar browser"]);
}

#[gpui::test]
fn a_modifier_or_the_middle_button_opens_the_browser(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), 2400.0, &[(LOCAL, None)]);
    let held = [
        Modifiers::shift(),
        Modifiers::control(),
        Modifiers::command(),
    ];
    for modifiers in held {
        click(cx, FIRST, modifiers);
        assert_eq!(opener::take(), [LOCAL], "{modifiers:?}");
    }
    press(cx, FIRST, MouseButton::Middle);
    assert_eq!(opener::take(), [LOCAL]);
    assert_eq!(toasts(cx), Vec::<String>::new());
}

#[gpui::test]
fn a_failed_open_says_so(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), 2400.0, &[(WEB, None)]);
    opener::fail(true);
    click(cx, FIRST, Modifiers::none());
    opener::fail(false);
    assert_eq!(opener::take(), [WEB]);
    assert_eq!(toasts(cx), ["Failed to open link"]);
}

#[gpui::test]
fn the_stored_choice_sends_a_web_page_to_the_sidebar(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), 2400.0, &[(WEB, None)]);
    cx.update(|_, cx| {
        let prefs = cx.global_mut::<crate::preferences::Preferences>();
        prefs.set("chat.webLinkOpenPreference", "sidebar").unwrap();
    });
    click(cx, FIRST, Modifiers::none());
    assert_eq!(opener::take(), Vec::<String>::new());
    assert_eq!(toasts(cx).len(), 1);
}

#[gpui::test]
fn enter_and_space_open_with_their_modifiers(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), 2400.0, &[(LOCAL, None)]);
    focus(cx, FIRST);
    cx.simulate_keystrokes("shift-enter");
    settle(cx);
    assert_eq!(opener::take(), [LOCAL]);
    cx.simulate_keystrokes("ctrl-space");
    settle(cx);
    assert_eq!(opener::take(), [LOCAL], "one open a key, none on key up");
    key(cx, "enter");
    assert_eq!(opener::take(), Vec::<String>::new());
    assert_eq!(
        toasts(cx).len(),
        1,
        "a plain key follows the preference, once"
    );
    cx.simulate_keystrokes("a");
    settle(cx);
    assert_eq!(toasts(cx).len(), 1);
}

const COPY: &str = "menu-row-Copy link";
const BROWSER: &str = "menu-row-Open in browser";
const SIDEBAR: &str = "menu-row-Open in sidebar";

#[gpui::test]
fn a_right_click_opens_the_menu_at_the_pointer(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), 2400.0, &[(LOCAL, None)]);
    let link = at(cx, FIRST).unwrap();
    press(cx, FIRST, MouseButton::Right);
    assert_eq!(
        opener::take(),
        Vec::<String>::new(),
        "a right click opens no page"
    );
    let (sidebar, browser, copy) = (
        at(cx, SIDEBAR).unwrap(),
        at(cx, BROWSER).unwrap(),
        at(cx, COPY).unwrap(),
    );
    assert!(sidebar.top() >= link.center().y && sidebar.left() >= link.center().x);
    assert!(sidebar.top() < browser.top() && browser.top() < copy.top());
    cx.simulate_click(browser.center(), Modifiers::none());
    settle(cx);
    assert_eq!(opener::take(), [LOCAL]);
    assert!(at(cx, COPY).is_none(), "a choice closes the menu");
}

#[gpui::test]
fn the_menu_copies_the_address(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let body = format!("{WEB} {LOCAL}");
    let cx = open(cx, dir.path(), 2400.0, &[(&body, None)]);
    press(cx, "msg-c0-link-1-0", MouseButton::Right);
    let copy = at(cx, COPY).unwrap().center();
    cx.simulate_click(copy, Modifiers::none());
    settle(cx);
    let copied = cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(copied.as_deref(), Some(LOCAL));
    assert_eq!(toasts(cx), ["Link copied"]);
    press(cx, FIRST, MouseButton::Right);
    let sidebar = at(cx, SIDEBAR).unwrap().center();
    cx.simulate_click(sidebar, Modifiers::none());
    settle(cx);
    assert_eq!(toasts(cx).len(), 2, "the sidebar browser comes with 17.0");
    assert_eq!(opener::take(), Vec::<String>::new());
}

#[gpui::test]
fn the_menu_key_opens_the_menu_under_the_link(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let body = format!("see {WEB}");
    let cx = open(cx, dir.path(), 2400.0, &[(&body, None)]);
    focus(cx, FIRST);
    cx.simulate_keystrokes("shift-f10");
    settle(cx);
    let (link, row) = (at(cx, FIRST).unwrap(), at(cx, SIDEBAR).unwrap());
    assert!(row.top() >= link.bottom() && row.top() < link.bottom() + px(16.0));
    assert!(row.left() >= link.left() && row.left() < link.left() + px(16.0));
    cx.simulate_keystrokes("escape");
    settle(cx);
    assert!(at(cx, SIDEBAR).is_none());
    // Focus is back on the link: a key opens it.
    cx.simulate_keystrokes("enter");
    settle(cx);
    assert_eq!(opener::take(), [WEB]);
}

#[gpui::test]
fn the_menu_key_reads_every_row_of_a_wrapped_link(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let body = format!("go {WEB}?{}", "path".repeat(60));
    let cx = open(cx, dir.path(), 2400.0, &[(&body, None)]);
    focus(cx, FIRST);
    cx.simulate_keystrokes("shift-f10");
    settle(cx);
    let (text, first) = (at(cx, "msg-c0-text").unwrap(), at(cx, FIRST).unwrap());
    let row = at(cx, SIDEBAR).unwrap();
    // The link's left is its later rows', its bottom its last row's.
    assert!(row.left() < first.left(), "{:?}", row.left());
    assert!(row.left() >= text.left() && row.top() >= text.bottom());
}

/// A shaped line of these glyphs, each a byte index and its x.
fn shaped(glyphs: &[(usize, f32)], width: f32) -> gpui::LineLayout {
    let glyph = |(index, x): &(usize, f32)| gpui::ShapedGlyph {
        id: gpui::GlyphId(0),
        position: point(px(*x), px(0.0)),
        index: *index,
        is_emoji: false,
    };
    let run = gpui::ShapedRun {
        font_id: gpui::FontId(0),
        glyphs: glyphs.iter().map(glyph).collect(),
    };
    gpui::LineLayout {
        width: px(width),
        runs: vec![run],
        len: glyphs.len(),
        ..Default::default()
    }
}

type Flat = Vec<[f32; 4]>;

fn boxes(line: &gpui::LineLayout, range: std::ops::Range<usize>) -> Flat {
    let found = super::boxes(line, range, px(20.0));
    let flat = |b: &gpui::Bounds<gpui::Pixels>| {
        let (x, y, w, h) = (b.left(), b.top(), b.size.width, b.size.height);
        [x, y, w, h].map(|n| (f32::from(n) * 100.0).round() / 100.0)
    };
    found.iter().map(flat).collect()
}

/// Glyphs 10px each in this order from the left.
fn even(order: &[usize]) -> gpui::LineLayout {
    let glyphs: Vec<(usize, f32)> = order
        .iter()
        .copied()
        .zip((0u8..).map(|n| f32::from(n) * 10.0))
        .collect();
    shaped(&glyphs, 10.0 * order.len() as f32)
}

#[test]
fn a_box_covers_each_stretch_of_a_link() {
    let plain = even(&[0, 1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(boxes(&plain, 2..7), [[20.0, 0.0, 50.0, 20.0]]);
    assert_eq!(boxes(&plain, 5..8), [[50.0, 0.0, 30.0, 20.0]]);
    assert_eq!(boxes(&plain, 8..9), Flat::new());
    // A link cut in two by its own direction change gets a box a stretch.
    let split = even(&[0, 1, 4, 5, 2, 3, 6]);
    assert_eq!(
        boxes(&split, 1..4),
        [[10.0, 0.0, 10.0, 20.0], [40.0, 0.0, 20.0, 20.0]]
    );
}

/// cosmic-text 0.19's glyphs for these lines at 15px, as `.scratch/9.5g/bidi` printed them.
#[test]
fn a_link_among_right_to_left_words_keeps_its_box() {
    // "שלום https://e.co/a שלום": the link is bytes 9..23.
    let hebrew = [
        (0, 152.83),
        (2, 145.57),
        (4, 141.77),
        (6, 132.4),
        (8, 128.5),
        (22, 120.17),
        (21, 114.67),
        (20, 105.64),
        (19, 98.46),
        (18, 94.52),
        (17, 86.1),
        (16, 80.6),
        (15, 75.1),
        (14, 71.15),
        (13, 64.01),
        (12, 54.83),
        (11, 49.48),
        (10, 44.14),
        (9, 34.94),
        (23, 31.04),
        (24, 20.43),
        (26, 13.17),
        (28, 9.37),
        (30, 0.0),
    ];
    assert_eq!(
        boxes(&shaped(&hebrew, 163.44), 9..23),
        [[34.94, 0.0, 93.56, 20.0]]
    );
    // "مرحبا https://e.co/a مرحبا": bytes 11..25, with a zero-width mark on a glyph's x.
    let arabic = [
        (0, 156.6),
        (2, 151.26),
        (4, 142.43),
        (6, 137.28),
        (6, 137.28),
        (8, 132.92),
        (10, 129.02),
        (24, 120.69),
        (23, 115.19),
        (22, 106.16),
        (21, 98.98),
        (20, 95.04),
        (19, 86.62),
        (18, 81.12),
        (17, 75.62),
        (16, 71.67),
        (15, 64.53),
        (14, 55.35),
        (13, 50.0),
        (12, 44.66),
        (11, 35.46),
        (25, 31.56),
        (26, 23.68),
        (28, 18.34),
        (30, 9.51),
        (32, 4.36),
        (32, 4.36),
        (34, 0.0),
    ];
    assert_eq!(
        boxes(&shaped(&arabic, 164.48), 11..25),
        [[35.46, 0.0, 93.56, 20.0]]
    );
    // "שלום https://e.co/שלום/a שלום": bytes 9..32, Hebrew inside the address.
    let mixed = [
        (0, 189.38),
        (2, 182.11),
        (4, 178.31),
        (6, 168.94),
        (8, 165.04),
        (20, 156.02),
        (19, 148.84),
        (18, 144.9),
        (17, 136.47),
        (16, 130.97),
        (15, 125.47),
        (14, 121.53),
        (13, 114.38),
        (12, 105.21),
        (11, 99.86),
        (10, 94.51),
        (9, 85.31),
        (21, 79.81),
        (22, 69.21),
        (24, 61.94),
        (26, 58.14),
        (28, 48.77),
        (30, 43.27),
        (31, 34.94),
        (32, 31.04),
        (33, 20.43),
        (35, 13.17),
        (37, 9.37),
        (39, 0.0),
    ];
    assert_eq!(
        boxes(&shaped(&mixed, 199.98), 9..32),
        [[34.94, 0.0, 130.1, 20.0]]
    );
}
