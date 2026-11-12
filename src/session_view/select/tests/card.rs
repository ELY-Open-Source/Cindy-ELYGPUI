//! A link chip's label and a handoff card's lines in the selection.

use gpui::{Bounds, Pixels, Point, TestAppContext, VisualTestContext, point, px};

use super::{corner, drag, held, lift, press, show, spot};
use crate::session_view::user_row::tests::at;

const TEXT: &str = "Before [task](cindy://session/abc123) after.\n\n\
    [x](cindy://session-card/abc?wake=resumed&title=Plan&last_active=2026-01-02T03:04:05Z)\n\n\
    Middle words.";
const CARD: &str = "msg-c1-card-a1";

fn part(cx: &mut VisualTestContext, name: &str) -> Bounds<Pixels> {
    at(cx, &format!("{CARD}{name}")).unwrap_or_else(|| panic!("no part {name}"))
}

/// What a drag from the first block's start to `to` holds, its runs joined.
fn to(cx: &mut VisualTestContext, to: Point<Pixels>) -> String {
    let start = spot(cx, "0", (0.0, 0.2));
    drag(cx, start, to);
    held(cx).concat()
}

// The places below are Blink's (`logs/review/9.6k1b-blink-{grid,right,chip}.log`).

#[gpui::test]
fn a_chips_label_is_text_with_its_own_line(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = show(cx, dir.path(), TEXT);
    let chip = at(cx, "msg-c1-chip-a0").unwrap();
    // The label's line is 20px in the chip's 26; its right end is 9px in.
    let (top, right) = (chip.top() + px(3.0), chip.right() - px(9.0));
    let end = spot(cx, "0", (1.0, 0.5));
    assert_eq!(to(cx, end), "Before task after.");
    assert_eq!(
        to(cx, point(right - px(1.0), top + px(10.0))),
        "Before task"
    );
    // The icon and the padding before the label are its start, the padding after it its end.
    assert_eq!(
        to(cx, point(chip.left() + px(12.0), top + px(10.0))),
        "Before "
    );
    assert_eq!(
        to(cx, point(chip.right() - px(3.0), top + px(10.0))),
        "Before task"
    );
    // Over the label's line is its start and under it its end, wherever the pointer stands.
    assert_eq!(to(cx, point(right - px(1.0), top - px(1.5))), "Before ");
    assert_eq!(
        to(cx, point(right - px(20.0), top + px(21.0))),
        "Before task"
    );
}

#[gpui::test]
fn a_cards_lines_select_as_a_flex_rows_items(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = show(cx, dir.path(), TEXT);
    let (card, title, badge) = (part(cx, ""), part(cx, "-title"), part(cx, "-badge"));
    let (short, arrow) = (part(cx, "-short"), part(cx, "-arrow"));
    let before = "Before task after.";
    let mid = |bounds: Bounds<Pixels>| bounds.center();
    // Left of the lines, at any height, is the card's start; the arrow is its end.
    for y in [
        card.top() + px(3.0),
        title.center().y,
        short.bottom() + px(8.0),
    ] {
        assert_eq!(to(cx, point(card.left() + px(5.0), y)), before);
        assert_eq!(to(cx, point(title.left() - px(20.0), y)), before);
        let all = to(cx, point(arrow.left() + px(1.0), y));
        assert!(
            all.starts_with("Before task after.PlanRESUMEDlast active "),
            "{all}"
        );
        assert!(all.ends_with("ago\u{b7}abc"), "{all}");
    }
    // In a line the nearest item: in its box its glyph, over it its start, under it its end.
    assert_eq!(to(cx, point(title.left() + px(1.0), mid(title).y)), before);
    assert_eq!(
        to(cx, point(title.right(), mid(title).y)),
        format!("{before}Plan")
    );
    assert_eq!(
        to(cx, point(badge.right() + px(40.0), mid(badge).y)),
        format!("{before}PlanRESUMED")
    );
    assert_eq!(
        to(cx, point(mid(badge).x, card.top() + px(4.0))),
        format!("{before}Plan")
    );
    assert_eq!(
        to(cx, point(mid(badge).x, badge.bottom() + px(1.0))),
        format!("{before}PlanRESUMED")
    );
    assert_eq!(
        to(cx, point(mid(title).x, title.bottom() + px(1.0))),
        format!("{before}Plan")
    );
    // Half way down the gap is the second line: over the short id, its start.
    let gap = title.bottom() + px(3.0);
    let lower = to(cx, point(mid(short).x, gap));
    assert!(lower.ends_with("ago\u{b7}"), "{lower}");
    // The history icon is a place before the words; under the words is their end.
    let history = part(cx, "-history");
    assert_eq!(
        to(cx, point(mid(history).x, short.bottom() + px(6.0))),
        format!("{before}PlanRESUMED")
    );
    let words = to(
        cx,
        point(history.right() + px(30.0), short.bottom() + px(6.0)),
    );
    assert!(words.ends_with(" ago"), "{words}");
    // Above the card the same columns count; under it the next block's start.
    assert_eq!(
        to(cx, point(mid(badge).x, card.top() - px(3.0))),
        format!("{before}Plan")
    );
    assert!(to(cx, point(mid(badge).x, card.bottom() + px(3.0))).ends_with("abc"));
}

#[gpui::test]
fn a_press_on_a_cards_bare_face_keeps_the_selection(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = show(cx, dir.path(), TEXT);
    let (card, title, arrow) = (part(cx, ""), part(cx, "-title"), part(cx, "-arrow"));
    let (end, away) = (spot(cx, "0", (1.0, 0.5)), corner(cx, (2.0, 2.0)));
    assert_eq!(to(cx, end), "Before task after.");
    for bare in [
        point(card.left() + px(5.0), card.top() + px(5.0)),
        point(title.left() - px(6.0), title.center().y),
        point(title.center().x, card.bottom() - px(5.0)),
        point(arrow.right() + px(6.0), arrow.center().y),
    ] {
        // Lifted off the card, so no click opens its task.
        press(cx, bare);
        lift(cx, away);
        assert_eq!(held(cx).concat(), "Before task after.", "{bare:?}");
    }
    // The circle places a caret, which drops it; a drag from there starts at the title.
    let disc = point(card.left() + px(30.0), card.center().y);
    press(cx, disc);
    assert!(held(cx).is_empty());
    cx.simulate_mouse_move(
        point(title.right(), title.center().y),
        gpui::MouseButton::Left,
        gpui::Modifiers::none(),
    );
    cx.run_until_parked();
    assert_eq!(held(cx).concat(), "Plan");
    lift(cx, away);
    // The arrow is the card's end.
    press(cx, arrow.center());
    cx.simulate_mouse_move(disc, gpui::MouseButton::Left, gpui::Modifiers::none());
    cx.run_until_parked();
    assert!(held(cx).concat().starts_with("PlanRESUMEDlast active "));
    lift(cx, away);
}
