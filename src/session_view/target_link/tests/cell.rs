//! A handoff card in a table's cell: a block among the cell's lines, counted in its column.

use gpui::{FontWeight, Modifiers, TestAppContext, VisualTestContext, px};

use super::{bounds, draw};
use crate::{
    handoff_card,
    i18n::Translator,
    markdown::hast::{self, Urls},
    router,
    session_view::{handoff, links::tests::click},
};

const Q: &str = "?wake=resumed&title=Plan&last_active=null";
const CELL: &str = "msg-c1-md-0-r1c0";

fn card(id: &str) -> String {
    format!("[x](cindy://session-card/{id}{Q})")
}

fn near(got: gpui::Pixels, want: f32) {
    assert!((f32::from(got) - want).abs() < 1.0, "{got} for {want}");
}

// Blink's places: `logs/review/9.6h5b-blink-table.log`, `9.6h5b2-blink-cell.log`.

#[gpui::test]
fn a_cell_s_card_stands_among_its_lines(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let text = format!("| a |\n| - |\n| one {} two |\n", card("old"));
    let cx = draw(cx, dir.path(), &text);
    let (cell, card) = (bounds(cx, CELL), bounds(cx, "msg-c1-card-a0"));
    // `py-2`, a line of 22.4px and `my-2` on either side; the cell draws the border under it.
    assert_eq!(card.size.height, px(74.0));
    near(cell.size.height, 151.8);
    near(card.top() - cell.top(), 38.4);
    // The card fills the cell's content box: `px-3`, and the border after the cell.
    assert_eq!(card.left() - cell.left(), px(12.0));
    assert_eq!(cell.right() - card.right(), px(13.0));
    let one = bounds(cx, &format!("{CELL}-lines"));
    let two = bounds(cx, &format!("{CELL}-p1-lines"));
    assert_eq!(card.top() - one.bottom(), px(8.0));
    assert_eq!(two.top() - card.bottom(), px(8.0));
    click(cx, "msg-c1-card-a0", Modifiers::none());
    assert_eq!(
        cx.update(|_, cx| router::path(cx).to_owned()),
        "/cc-agent/old"
    );
}

#[gpui::test]
fn two_cards_of_a_cell_share_a_margin(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let text = format!("| a |\n| - |\n| {} {} |\n", card("a"), card("b"));
    let cx = draw(cx, dir.path(), &text);
    let (first, second) = (bounds(cx, "msg-c1-card-a0"), bounds(cx, "msg-c1-card-a1"));
    assert_eq!(second.top() - first.bottom(), px(8.0));
    assert_eq!(bounds(cx, CELL).size.height, px(189.0));
}

/// The min and max content of the card `id`, as the column counts them.
fn content(cx: &mut VisualTestContext, id: &str) -> (f32, f32) {
    cx.update(|window, cx| {
        let never = handoff_card::never(cx.global::<Translator>());
        let short = handoff_card::short(id);
        let words = ("Plan", "RESUMED", (never.as_str(), false), short.as_str());
        handoff::content(words, (true, FontWeight::NORMAL), window, cx)
    })
}

#[gpui::test]
fn a_column_is_no_narrower_than_its_card_s_min_content(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let id = "83639512aaaaaaaa7ed0";
    // The other cell's max content takes all but a hair of the room past the mins.
    let long = "word ".repeat(4000);
    let text = format!(
        "{}\n\n| a | b |\n| - | - |\n| {} | {long} |\n",
        card(id),
        card(id)
    );
    let cx = draw(cx, dir.path(), &text);
    let (least, most) = content(cx, id);
    // The wide card shows its second line whole: that line, and what stands about the lines.
    let part = |cx: &mut VisualTestContext, n: usize, name: &str| {
        bounds(cx, &format!("msg-c1-card-a{n}-{name}"))
    };
    let (title, short) = (part(cx, 0, "title"), part(cx, 0, "short"));
    // gpui lays each of the line's three texts out to the device pixel.
    let over = f32::from(short.right() - title.left()) - (most - 122.0);
    assert!((0.0..3.0).contains(&over), "{over}");
    // `overflow-wrap: anywhere` breaks the short id at each letter: the min keeps one of 13.
    let tail = f32::from(short.size.width) * 12.0 / 13.0;
    assert!((most - least - tail).abs() < 1.0, "{least} {most} {tail}");
    // The squeezed column is the card's min content and the cell's 25px.
    let (cell, narrow) = (bounds(cx, "msg-c1-md-1-r1c0"), bounds(cx, "msg-c1-card-a1"));
    near(narrow.size.width, least);
    near(cell.size.width, least + 25.0);
    // The short id keeps its width there: the words before it are cut.
    let (short, arrow) = (part(cx, 1, "short"), part(cx, 1, "arrow"));
    near(arrow.left() - short.right(), 14.0);
}

#[gpui::test]
fn a_title_s_white_space_collapses_before_it_is_measured(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let long = "word ".repeat(400);
    let card = |title: &str| {
        format!("[x](cindy://session-card/a?wake=resumed&title={title}&last_active=null)")
    };
    let row = |title: &str| format!("| a | b |\n| - | - |\n| {} | {long} |\n", card(title));
    let spaces = "%20".repeat(40);
    let text = [
        row("Review%20the%20project%20plan%20before%20deployment"),
        row(&format!(
            "Review%20the%20project{spaces}plan%20before%20deployment"
        )),
        row("Review%0Athe%20project%09%09plan%20before%20deployment"),
    ];
    let cx = draw(cx, dir.path(), &text.join("\n"));
    let wide = |cx: &mut VisualTestContext, n: usize| {
        bounds(cx, &format!("msg-c1-md-{n}-r1c0")).size.width
    };
    assert_eq!(wide(cx, 1), wide(cx, 0));
    assert_eq!(wide(cx, 2), wide(cx, 0));
}

#[test]
fn a_head_cell_hands_its_weight_on() {
    let text = format!(
        "| {} |\n| - |\n| {} **{}** |\n",
        card("a"),
        card("b"),
        card("c")
    );
    let tree = hast::render(&text, true, Urls::Trusted);
    let (mut all, mut codes) = (Vec::new(), Vec::new());
    super::super::collect(&tree, &mut all, &mut codes);
    let mut about = std::collections::HashMap::new();
    super::super::around(&tree, (false, FontWeight::NORMAL), &mut about);
    let weights: Vec<f32> = (all.iter())
        .map(|a| about[&(*a as *const hast::Element)].1.0)
        .collect();
    assert_eq!(weights, [500.0, 400.0, 700.0]);
}
