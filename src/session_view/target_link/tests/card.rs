//! A session-card link in an assistant message: `SessionHandoffCard` among its lines.

use gpui::{
    FontWeight, KeyDownEvent, KeyUpEvent, Keystroke, Modifiers, MouseButton, TestAppContext,
    VisualTestContext,
};

use super::{bounds, draw};
use crate::{
    markdown::hast::{self, Urls},
    router,
    session_view::{
        link_chip,
        links::tests::{click, focus, key, press, toasts},
        user_row::tests::{at, settle},
    },
};

const Q: &str = "?wake=resumed&title=Plan&last_active=null";
const CARD: &str = "msg-c1-card-a0";

fn card(id: &str) -> String {
    format!("[x](cindy://session-card/{id}{Q})")
}

/// The top and height of `selector` under the message's Markdown.
fn span(cx: &mut VisualTestContext, selector: &str) -> (f32, f32) {
    let root = bounds(cx, "msg-c1-markdown");
    let found = bounds(cx, &format!("msg-c1-{selector}"));
    let top = f32::from(found.top() - root.top());
    (top, f32::from(found.size.height))
}

fn path(cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| router::path(cx).to_owned())
}

// The places below are Blink's (`logs/review/9.6h5b-blink-card.log`, `-list.log`).

#[gpui::test]
fn a_card_alone_keeps_its_margins_in_the_bubble(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = draw(cx, dir.path(), &card("a"));
    assert_eq!(span(cx, "card-a0"), (8.0, 74.0));
    assert_eq!(bounds(cx, "msg-c1-markdown").size.height, gpui::px(90.0));
    assert_eq!(
        bounds(cx, CARD).size.width,
        bounds(cx, "msg-c1-markdown").size.width
    );
}

#[gpui::test]
fn a_card_s_margins_collapse_with_its_paragraph_s(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let text = format!("before\n\n{} {}\n\nafter\n", card("a"), card("b"));
    let cx = draw(cx, dir.path(), &text);
    assert_eq!(span(cx, "md-0"), (0.0, 24.0));
    assert_eq!(span(cx, "card-a0"), (32.0, 74.0));
    assert_eq!(span(cx, "card-a1"), (114.0, 74.0));
    assert_eq!(span(cx, "md-2"), (196.0, 24.0));
}

#[gpui::test]
fn a_card_parts_the_lines_about_it(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let text = format!("before\n\n**one {} two**\n\nafter\n", card("a"));
    let cx = draw(cx, dir.path(), &text);
    assert_eq!(span(cx, "md-1"), (32.0, 24.0));
    assert_eq!(span(cx, "card-a0"), (64.0, 74.0));
    assert_eq!(span(cx, "md-1-p1"), (146.0, 24.0));
    assert_eq!(span(cx, "md-2"), (178.0, 24.0));
}

#[gpui::test]
fn a_break_after_a_card_opens_a_line(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let text = format!("one  \n{}  \ntwo\n\nafter\n", card("a"));
    let cx = draw(cx, dir.path(), &text);
    // The break before the card ends its line; the one after it is a line of its own.
    assert_eq!(span(cx, "md-0"), (0.0, 24.0));
    assert_eq!(span(cx, "card-a0"), (32.0, 74.0));
    assert_eq!(span(cx, "md-0-p1"), (114.0, 48.0));
    assert_eq!(span(cx, "md-1"), (170.0, 24.0));
}

#[gpui::test]
fn a_heading_s_card_stands_among_its_lines(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let text = format!("before\n\n## one {} two\n\nafter\n", card("a"));
    let cx = draw(cx, dir.path(), &text);
    assert_eq!(span(cx, "md-1"), (36.0, 28.0));
    assert_eq!(span(cx, "card-a0"), (72.0, 74.0));
    assert_eq!(span(cx, "md-1-p1"), (154.0, 28.0));
    assert_eq!(span(cx, "md-2"), (194.0, 24.0));
}

#[gpui::test]
fn an_item_s_marker_hangs_by_its_card(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let text = format!(
        "- {}\n- text\n- x {}\n- {} y\n",
        card("a"),
        card("b"),
        card("c")
    );
    let cx = draw(cx, dir.path(), &text);
    assert_eq!(span(cx, "md-0-0"), (8.0, 74.0));
    assert_eq!(span(cx, "md-0-1"), (90.0, 24.0));
    assert_eq!(span(cx, "md-0-2"), (118.0, 106.0));
    assert_eq!(span(cx, "card-a1"), (150.0, 74.0));
    assert_eq!(span(cx, "md-0-3"), (232.0, 106.0));
    assert_eq!(span(cx, "card-a2"), (232.0, 74.0));
    // A marker hangs over the card's first baseline, 46px down, as over a line's.
    let hung = |cx: &mut VisualTestContext, item: usize| {
        let (top, tall) = span(cx, &format!("md-0-{item}-marker"));
        (top - span(cx, &format!("md-0-{item}")).0, tall)
    };
    let (by_card, by_line) = (hung(cx, 0), hung(cx, 1));
    assert!(
        by_card.0 + by_card.1 <= 46.0 && by_card.0 > 46.0 - 24.0,
        "{by_card:?}"
    );
    assert_eq!(hung(cx, 3), by_card);
    assert_eq!(hung(cx, 2), by_line);
}

#[gpui::test]
fn an_outlined_badge_makes_the_card_taller(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let text =
        "[x](cindy://session-card/a?wake=already-active&title=&last_active=2026-01-02T03:04:05Z)";
    let cx = draw(cx, dir.path(), text);
    assert_eq!(span(cx, "card-a0"), (8.0, 76.0));
}

#[gpui::test]
fn a_link_that_parses_as_no_card_is_a_link(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = draw(
        cx,
        dir.path(),
        "[x](cindy://session-card/a?wake=asleep&title=&last_active=null)",
    );
    assert!(at(cx, CARD).is_none());
    assert_eq!(span(cx, "md-0"), (0.0, 24.0));
}

#[gpui::test]
fn a_click_or_a_key_opens_the_card_s_task(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let text = format!("{}\n\n{}\n", card("old"), card("nowhere"));
    let cx = draw(cx, dir.path(), &text);
    assert_eq!(path(cx), "/cc-agent/s1");
    // A button takes any modifier, and no middle or right press.
    press(cx, CARD, MouseButton::Middle);
    press(cx, CARD, MouseButton::Right);
    assert_eq!(path(cx), "/cc-agent/s1");
    assert!(at(cx, "menu-row-Copy link").is_none());
    click(cx, CARD, Modifiers::shift());
    assert_eq!(path(cx), "/cc-agent/old");
    assert_eq!(toasts(cx), Vec::<String>::new());
}

#[test]
fn a_strong_or_a_heading_hands_its_weight_on() {
    let text = format!(
        "{} **{}** *{}*\n\n## {} **{}**\n\n#### {}\n",
        card("a"),
        card("b"),
        card("c"),
        card("d"),
        card("e"),
        card("f")
    );
    let tree = hast::render(&text, true, Urls::Trusted);
    let (mut all, mut codes) = (Vec::new(), Vec::new());
    super::super::collect(&tree, &mut all, &mut codes);
    let mut about = std::collections::HashMap::new();
    super::super::around(&tree, (false, FontWeight::NORMAL), &mut about);
    let weights: Vec<f32> = (all.iter())
        .map(|a| about[&(*a as *const hast::Element)].1.0)
        .collect();
    assert_eq!(weights, [400.0, 700.0, 400.0, 500.0, 700.0, 400.0]);
}

/// The route after `name`'s key down, and after its key up.
fn keyed(cx: &mut VisualTestContext, name: &'static str) -> (String, String) {
    let keystroke = Keystroke::parse(name).unwrap();
    cx.simulate_event(KeyDownEvent {
        keystroke: keystroke.clone(),
        is_held: false,
        prefer_character_input: false,
    });
    settle(cx);
    let down = path(cx);
    cx.simulate_event(KeyUpEvent { keystroke });
    settle(cx);
    (down, path(cx))
}

#[gpui::test]
fn the_keyboard_s_focus_draws_the_ring(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let text = format!("{}\n\n{}\n", card("old"), card("nowhere"));
    let cx = draw(cx, dir.path(), &text);
    focus(cx, "msg-c1-card-a1");
    assert!(
        at(cx, "msg-c1-card-a1-ring").is_none(),
        "a press draws no ring"
    );
    key(cx, "shift");
    assert!(at(cx, "msg-c1-card-a1-ring").is_some());
    assert!(at(cx, "msg-c1-card-a0-ring").is_none());
    assert_eq!(path(cx), "/cc-agent/s1");
}

// A native button's keys, whatever they hold (`logs/review/9.6h5b1-r1-review-blink.log`).
macro_rules! keys {
    ($($test:ident: $name:literal => $down:literal;)*) => {$(
        #[gpui::test]
        fn $test(cx: &mut TestAppContext) {
            let dir = tempfile::tempdir().unwrap();
            let cx = draw(cx, dir.path(), &card("old"));
            focus(cx, CARD);
            let (down, up) = keyed(cx, $name);
            assert_eq!((down.as_str(), up.as_str()), ($down, "/cc-agent/old"));
        }
    )*};
}
keys! {
    enter_clicks_on_key_down: "enter" => "/cc-agent/old";
    shift_enter_clicks_on_key_down: "shift-enter" => "/cc-agent/old";
    ctrl_enter_clicks_on_key_down: "ctrl-enter" => "/cc-agent/old";
    alt_enter_clicks_on_key_down: "alt-enter" => "/cc-agent/old";
    space_clicks_on_key_up: "space" => "/cc-agent/s1";
    shift_space_clicks_on_key_up: "shift-space" => "/cc-agent/s1";
    ctrl_space_clicks_on_key_up: "ctrl-space" => "/cc-agent/s1";
    alt_space_clicks_on_key_up: "alt-space" => "/cc-agent/s1";
}

/// The opens the card asked after `between` came while Space was held, and after Space lifted.
fn held(cx: &mut TestAppContext, between: impl FnOnce(&mut VisualTestContext)) -> (bool, bool) {
    let dir = tempfile::tempdir().unwrap();
    // The card names the task shown, so it stays mounted through its own opens.
    let text = format!("{}\n\n{}\n", card("s1"), card("s1"));
    let cx = draw(cx, dir.path(), &text);
    focus(cx, CARD);
    let asked = |cx: &mut VisualTestContext| cx.update(|_, cx| link_chip::asked(CARD, cx));
    let space = Keystroke::parse("space").unwrap();
    cx.simulate_event(KeyDownEvent {
        keystroke: space.clone(),
        is_held: false,
        prefer_character_input: false,
    });
    settle(cx);
    between(cx);
    settle(cx);
    let before = asked(cx);
    cx.simulate_event(KeyUpEvent { keystroke: space });
    settle(cx);
    let after = asked(cx);
    (before.is_some(), after != before)
}

// A held Space on a native button (`logs/review/9.6h5b1-r2-review-key-reference.log`).
macro_rules! spaces {
    ($($test:ident: $want:expr, $between:expr;)*) => {$(
        #[gpui::test]
        fn $test(cx: &mut TestAppContext) {
            assert_eq!(held(cx, $between), $want);
        }
    )*};
}
spaces! {
    a_held_space_clicks_as_it_lifts: (false, true), |_| {};
    another_key_s_release_keeps_the_space: (false, true), |cx| {
        cx.simulate_event(KeyUpEvent { keystroke: Keystroke::parse("a").unwrap() })
    };
    another_key_s_press_keeps_the_space: (false, true), |cx| key(cx, "a");
    a_repeated_space_clicks_once: (false, true), |cx| {
        cx.simulate_event(KeyDownEvent {
            keystroke: Keystroke::parse("space").unwrap(),
            is_held: true,
            prefer_character_input: false,
        })
    };
    enter_ends_the_held_space: (true, false), |cx| key(cx, "enter");
    a_click_ends_the_held_space: (true, false), |cx| click(cx, CARD, Modifiers::none());
    a_blur_ends_the_held_space: (false, false), |cx| {
        cx.update(|window, cx| {
            let card = window.focused(cx).unwrap();
            window.blur(cx);
            window.focus(&card, cx);
        })
    };
    a_focus_gone_and_back_ends_the_held_space: (false, false), |cx| {
        cx.update(|window, cx| {
            let card = window.focused(cx).unwrap();
            window.focus_next(cx);
            window.focus(&card, cx);
        })
    };
    a_focus_on_another_card_ends_the_held_space: (false, false), |cx| {
        focus(cx, "msg-c1-card-a1")
    };
}

#[gpui::test]
fn an_empty_last_active_is_none(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let link = |last: &str| {
        format!("[x](cindy://session-card/a?wake=resumed&title=Plan&last_active={last})")
    };
    let text = [link(""), link("null"), link("soon"), link("%20")].join("\n\n");
    let cx = draw(cx, dir.path(), &text);
    let history = |cx: &mut VisualTestContext, n: usize| {
        at(cx, &format!("msg-c1-card-a{n}-history")).is_some()
    };
    assert_eq!(
        [0, 1, 2, 3].map(|n| history(cx, n)),
        [false, false, true, true]
    );
}

#[gpui::test]
fn the_card_lays_its_parts_out_as_blink_does(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = draw(cx, dir.path(), &card("83639512-aaaa-bbbb-cccc-00007ed0"));
    let whole = bounds(cx, CARD);
    let part = |cx: &mut VisualTestContext, name: &str| {
        let found = bounds(cx, &format!("{CARD}-{name}"));
        let at = found.origin - whole.origin;
        (
            f32::from(at.x),
            f32::from(at.y),
            f32::from(found.size.height),
        )
    };
    // The lines start past the border, `p-4`, the 36px circle and `gap-[14px]`.
    assert_eq!(part(cx, "title"), (67.0, 17.0, 20.0));
    let badge = part(cx, "badge");
    assert_eq!((badge.1, badge.2), (17.0, 20.0));
    let short = part(cx, "short");
    assert_eq!((short.1, short.2), (41.0, 16.0));
    let arrow = bounds(cx, &format!("{CARD}-arrow"));
    assert_eq!(whole.right() - arrow.right(), gpui::px(17.0));
    assert_eq!(arrow.top() - whole.top(), gpui::px(25.0));
    // The badge follows the title by `gap-2`; its letters stand 0.4px apart.
    let title = bounds(cx, &format!("{CARD}-title"));
    let badge = bounds(cx, &format!("{CARD}-badge"));
    assert_eq!(badge.left() - title.right(), gpui::px(8.0));
    // Seven letters of 6px, each 0.4px more; gpui lays the box out to the device pixel.
    let over = f32::from(badge.size.width) - (16.0 + 7.0 * 6.4);
    assert!((0.0..1.0).contains(&over), "{over}");
}

#[gpui::test]
fn a_narrow_card_cuts_its_title_and_keeps_its_badge(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let title = "long ".repeat(80);
    let text = format!("[x](cindy://session-card/a?wake=created&title={title}&last_active=null)");
    let cx = draw(cx, dir.path(), &text.replace(' ', "%20"));
    let (whole, badge) = (bounds(cx, CARD), bounds(cx, &format!("{CARD}-badge")));
    assert_eq!(whole.size.width, bounds(cx, "msg-c1-markdown").size.width);
    assert_eq!(whole.size.height, gpui::px(74.0));
    let arrow = bounds(cx, &format!("{CARD}-arrow"));
    assert_eq!(arrow.left() - badge.right(), gpui::px(14.0));
}
