//! A session or project link in an assistant message: its chip, or its words.

use gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext, px};

use super::{bounds, draw, piece};
use crate::{
    markdown::hast::{self, Urls},
    router,
    session_view::{
        links::tests::{click, press, toasts},
        user_row::tests::{at, settle},
    },
};

const HERE: &str = "menu-row-Open in current window";
const LOCATE: &str = "menu-row-Locate project";
const COPY: &str = "menu-row-Copy link";

fn path(cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| router::path(cx).to_owned())
}

fn chip(n: usize) -> String {
    format!("msg-c1-chip-a{n}")
}

/// A chip's width for `letters` of 7.2 pixels and an icon, as the row snaps it.
fn wide(cx: &mut VisualTestContext, n: usize, letters: f32) {
    let width = f32::from(bounds(cx, &chip(n)).size.width);
    let over = width - (letters * 7.2 + 38.0);
    assert!((0.0..1.0).contains(&over), "chip {n}: {width}");
}

#[gpui::test]
fn a_deep_link_is_its_chip_in_the_row(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let text = "see [mine](cindy://session/old) cindy://session/old \
        [w](cindy://project/%2Fhome%2Fme%2Fwork) end\n\nplain\n\n\
        [q](cindy://session/s1?message=c2)\n";
    let cx = draw(cx, dir.path(), text);
    // The author's label, the task's title, the author's label for a folder.
    wide(cx, 0, 4.0);
    wide(cx, 1, 8.0);
    wide(cx, 2, 1.0);
    // The chips stand in the row, in the order written, the words between them.
    let (first, second) = (bounds(cx, &chip(0)), bounds(cx, &chip(1)));
    assert!(first.right() < second.left());
    assert_eq!(first.top(), second.top());
    // The row grows about the chip's middle, as a user message's row does.
    let (row, line) = (bounds(cx, "msg-c1-md-0"), bounds(cx, "msg-c1-md-1"));
    assert_eq!((row.size.height, line.size.height), (px(30.0), px(24.0)));
    assert_eq!(first.top() - row.top(), px(4.0));
    // A message link's quote is not lifted, so its row grows a pixel more.
    let (quote, row) = (bounds(cx, &chip(3)), bounds(cx, "msg-c1-md-2"));
    assert_eq!(row.size.height, px(31.0));
    assert_eq!(quote.top() - row.top(), px(5.0));
}

#[gpui::test]
fn a_deep_link_that_names_nothing_is_its_words(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let text = "[gone](cindy://session/) [](cindy://project/) [  ](cindy://session/)\n";
    let cx = draw(cx, dir.path(), text);
    for n in 0..3 {
        assert!(at(cx, &chip(n)).is_none(), "chip {n}");
    }
    // `label ?? href`: the label, else the address, as plain text of one row.
    let row = bounds(cx, "msg-c1-md-0");
    assert_eq!(row.size.height, px(24.0));
    for n in 0..4 {
        if let Some(found) = at(cx, &piece(0, n)) {
            cx.simulate_click(found.center(), Modifiers::none());
            cx.simulate_mouse_down(found.center(), MouseButton::Right, Modifiers::none());
            cx.simulate_mouse_up(found.center(), MouseButton::Right, Modifiers::none());
            settle(cx);
        }
    }
    assert_eq!(path(cx), "/cc-agent/s1");
    assert_eq!(toasts(cx), Vec::<String>::new());
    assert!(at(cx, COPY).is_none());
}

#[gpui::test]
fn a_chip_opens_its_task_and_its_menu(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let text = "[w](cindy://project/%2Fhome%2Fme%2Fwork) and [mine](cindy://session/old)\n";
    let cx = draw(cx, dir.path(), text);
    press(cx, &chip(0), MouseButton::Right);
    assert!(at(cx, LOCATE).is_some());
    let copy = bounds(cx, COPY).center();
    cx.simulate_click(copy, Modifiers::none());
    settle(cx);
    let copied = cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(
        copied.as_deref(),
        Some("cindy://project/%2Fhome%2Fme%2Fwork")
    );
    press(cx, &chip(1), MouseButton::Right);
    assert!(at(cx, HERE).is_some() && at(cx, LOCATE).is_none());
    cx.simulate_keystrokes("escape");
    settle(cx);
    assert_eq!(path(cx), "/cc-agent/s1");
    click(cx, &chip(1), Modifiers::none());
    assert_eq!(path(cx), "/cc-agent/old");
}

#[gpui::test]
fn a_chip_stands_in_a_table_s_cell(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let text = "| a | b |\n| - | - |\n| [mine](cindy://session/old) | x |\n";
    let cx = draw(cx, dir.path(), text);
    wide(cx, 0, 4.0);
    click(cx, &chip(0), Modifiers::none());
    assert_eq!(path(cx), "/cc-agent/old");
}

#[gpui::test]
fn a_chip_s_label_follows_the_ui_font_size(cx: &mut TestAppContext) {
    use ely_gpui_component::theme::Theme;
    let dir = tempfile::tempdir().unwrap();
    let text = "[mine](cindy://session/old) [q](cindy://session/s1?message=c2)\n";
    let cx = draw(cx, dir.path(), text);
    let widths = |cx: &mut VisualTestContext, size: f32| {
        cx.update(|window, cx| {
            Theme::update(cx, |theme| theme.font_scale = size / 14.0);
            window.refresh();
        });
        settle(cx);
        [0, 1].map(|n| f32::from(bounds(cx, &chip(n)).size.width))
    };
    // `--text-12` is 12, 15 and 10 pixels at the UI sizes 14, 18 and 12.
    let (usual, large, small) = (widths(cx, 14.0), widths(cx, 18.0), widths(cx, 12.0));
    for n in 0..2 {
        assert!(small[n] < usual[n] && usual[n] < large[n], "chip {n}");
    }
}

#[test]
fn an_em_about_a_link_slants_it() {
    let text = "*[a](cindy://session/a)* [*b*](cindy://session/b) \
        ***[c](cindy://project/c)***\n\n| x |\n| - |\n| *[d](cindy://session/d)* |\n";
    let tree = hast::render(text, true, Urls::Trusted);
    let (mut all, mut codes, mut slant) = (Vec::new(), Vec::new(), Vec::new());
    super::super::collect(&tree, &mut all, &mut codes);
    super::super::slanted(&tree, false, &mut slant);
    let slants = |a: &&hast::Element| slant.contains(&(*a as *const hast::Element));
    // An `em` inside the label is flattened with the label.
    assert_eq!(
        all.iter().map(slants).collect::<Vec<_>>(),
        [true, false, true, true]
    );
}

#[gpui::test]
fn a_slanted_chip_stands_where_an_upright_one_does(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let text = "*[mine](cindy://session/old)*\n\n[mine](cindy://session/old)\n";
    let cx = draw(cx, dir.path(), text);
    let lift = |cx: &mut VisualTestContext, n: usize| {
        let row = bounds(cx, &format!("msg-c1-md-{n}"));
        (bounds(cx, &chip(n)).top() - row.top(), row.size.height)
    };
    assert_eq!(lift(cx, 0), lift(cx, 1));
    assert_eq!(lift(cx, 0), (px(4.0), px(30.0)));
}
