//! An image in a line of an assistant message, by `logs/review/9.6i3a-blink-image.log`.

use std::path::Path;

use gpui::{Bounds, Modifiers, Pixels, TestAppContext, VisualTestContext, px, size};

use super::fit;
use crate::{
    session_view::{
        images::{self, tests::put},
        links::tests::click,
        user_row::tests::{at, open_in, open_roles, settle},
    },
    workspace_media,
};

fn draw<'a>(cx: &'a mut TestAppContext, dir: &Path, text: &str) -> &'a mut VisualTestContext {
    open_roles(cx, dir, 2400.0, &[("user", "hi"), ("assistant", text)])
}

fn bounds(cx: &mut VisualTestContext, selector: &str) -> Bounds<Pixels> {
    at(cx, selector).unwrap_or_else(|| panic!("no {selector}"))
}

/// The message's Markdown, whose height is its blocks'.
const ROOT: &str = "msg-c1-markdown";

#[test]
fn the_button_and_its_image_fit_as_blinks() {
    // A 1200 by 900 viewport: `min(50vw, 480px)` and `min(40vh, 420px)`.
    let big = (480.0, 360.0);
    assert_eq!(fit((100, 50), None, big, 914.0), [100.0, 100.0, 50.0]);
    assert_eq!(fit((1000, 500), None, big, 914.0), [720.0, 480.0, 240.0]);
    assert_eq!(fit((300, 2000), None, big, 914.0), [54.0, 54.0, 360.0]);
    assert_eq!(fit((2000, 300), None, big, 914.0), [914.0, 480.0, 72.0]);
    assert_eq!(fit((1000, 500), None, big, 200.0), [200.0, 200.0, 100.0]);
    // The `width` attribute sets the width; `max-height` then cuts the height alone.
    assert_eq!(
        fit((1000, 500), Some(200.0), big, 914.0),
        [200.0, 200.0, 100.0]
    );
    assert_eq!(
        fit((100, 50), Some(1000.0), big, 914.0),
        [914.0, 480.0, 240.0]
    );
    assert_eq!(
        fit((300, 2000), Some(200.0), big, 914.0),
        [200.0, 200.0, 360.0]
    );
    assert_eq!(fit((100, 50), Some(0.0), big, 914.0), [0.0, 0.0, 0.0]);
    // A 700 by 500 viewport.
    let small = (350.0, 200.0);
    assert_eq!(fit((1000, 500), None, small, 600.0), [400.0, 350.0, 175.0]);
    assert_eq!(fit((300, 2000), None, small, 600.0), [30.0, 30.0, 200.0]);
    assert_eq!(fit((2000, 300), None, small, 600.0), [600.0, 350.0, 52.5]);
    assert_eq!(fit((0, 0), None, small, 600.0), [0.0, 0.0, 0.0]);
}

#[gpui::test]
fn a_loaded_image_stands_on_its_line_s_baseline(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    put(dir.path(), "a.webp", (100, 50));
    let cx = draw(
        cx,
        dir.path(),
        "before\n\none ![](xdt-image://s1/a.webp) two\n\nafter",
    );
    let (root, button) = (bounds(cx, ROOT), bounds(cx, "msg-c1-g0"));
    assert_eq!(button.size, size(px(100.0), px(50.0)));
    assert_eq!(bounds(cx, "msg-c1-g0-img").size, button.size);
    // A 24px line, `mb-2`, then the button's 18px margin; the row is the image and both margins.
    assert_eq!(button.top() - root.top(), px(24.0 + 8.0 + 18.0));
    assert_eq!(root.size.height, px(24.0 + 8.0 + 86.0 + 8.0 + 24.0));
    // The words stand left and right of the button, their baseline at its bottom.
    let (one, two) = (
        bounds(cx, "msg-c1-md-1-r0-0"),
        bounds(cx, "msg-c1-md-1-r0-2"),
    );
    assert!(one.right() <= button.left() && button.right() <= two.left());
    assert!(one.top() < button.bottom() && button.bottom() < one.bottom());
}

#[gpui::test]
fn a_wide_image_takes_the_button_s_left(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    put(dir.path(), "wide.webp", (2000, 300));
    let cx = draw(cx, dir.path(), "one ![](xdt-image://s1/wide.webp) two");
    let (root, button) = (bounds(cx, ROOT), bounds(cx, "msg-c1-g0"));
    let picture = bounds(cx, "msg-c1-g0-img");
    // The button fills the line, so it takes a row of its own between the words' rows.
    assert_eq!(button.size.width, root.size.width);
    assert_eq!(picture.left(), button.left());
    assert_eq!(picture.size.width, px(480.0));
    assert_eq!(picture.size.height, px(72.0));
    assert_eq!(root.size.height, px(24.0 + 72.0 + 36.0 + 24.0));
}

#[gpui::test]
fn an_image_with_no_address_keeps_its_frame(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = draw(cx, dir.path(), "one ![]() two");
    let (root, frame) = (bounds(cx, ROOT), bounds(cx, "msg-c1-g0"));
    assert_eq!(frame.size, size(px(280.0), px(180.0)));
    assert_eq!(bounds(cx, "msg-c1-g0-loading").size, frame.size);
    assert_eq!(root.size.height, px(216.0));
    assert_eq!(frame.top() - root.top(), px(18.0));
    // The words' baseline is the bottom of the frame's centred spinner, 98px down.
    let (one, two) = (
        bounds(cx, "msg-c1-md-0-r0-0"),
        bounds(cx, "msg-c1-md-0-r0-2"),
    );
    let middle = frame.top() + px(98.0);
    assert_eq!(one.top(), two.top());
    assert!(one.top() < middle && middle < one.bottom());
    assert!(middle - one.top() > one.bottom() - middle);
}

#[gpui::test]
fn a_missing_image_is_a_card_between_the_lines(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let text = "before\n\none ![shot](xdt-image://s1/gone.webp) two\n\nafter";
    let cx = draw(cx, dir.path(), text);
    let (root, card) = (bounds(cx, ROOT), bounds(cx, "msg-c1-g0-failed"));
    assert_eq!(card.size, size(px(280.0), px(140.0)));
    assert_eq!(card.left(), root.left());
    // `before`, `mb-2`, `one`, the card, `two`, `mt-2`, `after`.
    assert_eq!(card.top() - root.top(), px(24.0 + 8.0 + 24.0));
    assert_eq!(
        root.size.height,
        px(24.0 + 8.0 + 24.0 + 140.0 + 24.0 + 8.0 + 24.0)
    );
    assert!(at(cx, "msg-c1-g0").is_none());
    assert!(at(cx, "msg-c1-g0-retry").is_none());
}

#[gpui::test]
fn a_card_alone_is_its_paragraph(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = draw(cx, dir.path(), "![](xdt-image://s1/gone%20far.webp)");
    assert_eq!(bounds(cx, ROOT).size.height, px(140.0));
    // The name is the address's last segment, decoded.
    assert!(at(cx, "msg-c1-g0-name").is_some());
}

#[gpui::test]
fn bytes_that_do_not_decode_offer_a_retry_and_a_focus_tries_again(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("cc-agent/images/s1");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("a.webp"), b"nope").unwrap();
    let cx = draw(cx, dir.path(), "![](xdt-image://s1/a.webp)");
    // The card grows by `gap-2` and the button's 28px.
    let tall = f32::from(bounds(cx, "msg-c1-g0-failed").size.height);
    assert!((tall - 151.6).abs() < 0.5, "{tall}");
    assert!(at(cx, "msg-c1-g0-retry").is_some());
    // A focus loads the bytes again, and they fail again.
    cx.update(|_, cx| images::focused(cx));
    settle(cx);
    settle(cx);
    assert!(at(cx, "msg-c1-g0-retry").is_some());
    put(dir.path(), "a.webp", (100, 50));
    click(cx, "msg-c1-g0-retry", Modifiers::default());
    settle(cx);
    settle(cx);
    assert!(at(cx, "msg-c1-g0-failed").is_none());
    assert_eq!(bounds(cx, "msg-c1-g0").size, size(px(100.0), px(50.0)));
}

#[gpui::test]
fn a_focus_tries_a_missing_image_again(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = draw(cx, dir.path(), "![](xdt-image://s1/a.webp)");
    assert!(at(cx, "msg-c1-g0-failed").is_some());
    put(dir.path(), "a.webp", (100, 50));
    cx.update(|_, cx| images::focused(cx));
    settle(cx);
    settle(cx);
    assert_eq!(bounds(cx, "msg-c1-g0").size, size(px(100.0), px(50.0)));
}

#[gpui::test]
fn an_address_with_no_reader_yet_is_unavailable(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = draw(cx, dir.path(), "![a](https://x.test/a.png)");
    assert_eq!(
        bounds(cx, "msg-c1-g0-failed").size,
        size(px(280.0), px(140.0))
    );
}

#[gpui::test]
fn images_in_a_list_and_a_cell_keep_their_boxes(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    put(dir.path(), "a.webp", (100, 50));
    let text = "- ![](xdt-image://s1/a.webp)\n- ![](xdt-image://s1/gone.webp)\n\n\
                | h | k |\n| - | - |\n| ![](xdt-image://s1/a.webp) | ![](xdt-image://s1/gone.webp) |";
    let cx = draw(cx, dir.path(), text);
    assert_eq!(bounds(cx, "msg-c1-g0").size, size(px(100.0), px(50.0)));
    assert_eq!(
        bounds(cx, "msg-c1-g1-failed").size,
        size(px(280.0), px(140.0))
    );
    assert_eq!(bounds(cx, "msg-c1-g2").size, size(px(100.0), px(50.0)));
    assert_eq!(
        bounds(cx, "msg-c1-g3-failed").size,
        size(px(280.0), px(140.0))
    );
}

#[gpui::test]
fn a_retry_loads_its_own_image_alone(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("cc-agent/images/s1");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("a.webp"), b"nope").unwrap();
    let text = "![one](xdt-image://s1/a.webp)\n\n![two](xdt-image://s1/a.webp)";
    let cx = draw(cx, dir.path(), text);
    put(dir.path(), "a.webp", (100, 50));
    click(cx, "msg-c1-g0-retry", Modifiers::default());
    settle(cx);
    settle(cx);
    assert_eq!(bounds(cx, "msg-c1-g0").size, size(px(100.0), px(50.0)));
    // The load is this image's while it shows this address, and no other's.
    let own = |cx: &mut VisualTestContext, id: &str, url: &str| {
        cx.update(|_, cx| images::own(id, url, cx).is_some())
    };
    assert!(own(cx, "msg-c1-g0", "xdt-image://s1/a.webp"));
    assert!(!own(cx, "msg-c1-g0", "xdt-image://s1/b.webp"));
    assert!(!own(cx, "msg-c1-g1", "xdt-image://s1/a.webp"));
    // The other image of the same address keeps its failure and its Retry.
    assert!(at(cx, "msg-c1-g1-failed").is_some());
    assert!(at(cx, "msg-c1-g1-retry").is_some());
    cx.update(|_, cx| images::focused(cx));
    settle(cx);
    settle(cx);
    assert_eq!(bounds(cx, "msg-c1-g1").size, size(px(100.0), px(50.0)));
}

// `logs/review/9.6i3a-blink-cells.log`: a card asks its icon and its name at least.
#[gpui::test]
fn four_cards_share_a_narrow_table(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let gone = "![x](xdt-image://s1/gone.webp)";
    let text =
        format!("| a | b | c | d |\n| - | - | - | - |\n| {gone} | {gone} | {gone} | {gone} |");
    let cx = open_roles(
        cx,
        dir.path(),
        1100.0,
        &[("user", "hi"), ("assistant", &text)],
    );
    let root = bounds(cx, ROOT);
    let cards: Vec<_> = (0..4)
        .map(|n| bounds(cx, &format!("msg-c1-g{n}-failed")))
        .collect();
    // Each column is a quarter of the table; a card is its cell less the padding and a border.
    let column = f32::from(root.size.width) / 4.0;
    assert!(column < 280.0, "{column}");
    for card in &cards {
        assert!(
            (f32::from(card.size.width) - (column - 25.0)).abs() < 1.0,
            "{card:?}"
        );
        assert!(card.right() <= root.right());
        assert!(card.size.height >= px(140.0));
    }
}

// A card with Retry asks the button's width at least, so the button keeps the card's padding.
#[gpui::test]
fn four_cards_with_retry_keep_their_padding(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("cc-agent/images/s1");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("a.webp"), b"nope").unwrap();
    let bad = "![x](xdt-image://s1/a.webp)";
    let text = format!("| a | b | c | d |\n| - | - | - | - |\n| {bad} | {bad} | {bad} | {bad} |");
    let cx = open_roles(
        cx,
        dir.path(),
        600.0,
        &[("user", "hi"), ("assistant", &text)],
    );
    for n in 0..4 {
        let card = bounds(cx, &format!("msg-c1-g{n}-failed"));
        let retry = bounds(cx, &format!("msg-c1-g{n}-retry"));
        // `p-4` and the border on each side.
        assert!(retry.left() >= card.left() + px(17.0), "{card:?} {retry:?}");
        assert!(
            retry.right() <= card.right() - px(17.0),
            "{card:?} {retry:?}"
        );
    }
}

/// A WebP of this size at `to`, in the task's folder.
fn file(dir: &Path, to: &Path, size: (u32, u32)) {
    put(dir, "made.webp", size);
    std::fs::copy(dir.join("cc-agent/images/s1/made.webp"), to).unwrap();
}

/// `s1` in the folder `work` under `dir`, open on an answer.
fn local<'a>(cx: &'a mut TestAppContext, dir: &Path, text: &str) -> &'a mut VisualTestContext {
    let work = dir.join("work");
    let rows = [("user", "hi"), ("assistant", text)];
    let cx = open_in(cx, dir, work.to_str().unwrap(), &rows);
    for _ in 0..4 {
        settle(cx);
    }
    cx
}

#[gpui::test]
fn a_local_file_draws_by_its_path_in_the_task_s_folder(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path().canonicalize().unwrap();
    let work = dir.join("work");
    std::fs::create_dir_all(work.join("deep/renders")).unwrap();
    file(&dir, &work.join("near.webp"), (100, 50));
    file(&dir, &work.join("deep/renders/far.webp"), (60, 30));
    let vector = r#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"/>"#;
    std::fs::write(work.join("draw.svg"), vector).unwrap();
    std::fs::write(work.join("notes.txt"), "words").unwrap();
    let whole = work.join("near.webp");
    let text = format!(
        "![](near.webp)\n\n![](renders/far.webp)\n\n![]({})\n\n![](draw.svg)\n\n![](notes.txt)",
        whole.display()
    );
    let cx = local(cx, &dir, &text);
    assert_eq!(bounds(cx, "msg-c1-g0").size, size(px(100.0), px(50.0)));
    // A path that is not at the join is the one file of the folder that ends with it.
    assert_eq!(bounds(cx, "msg-c1-g1").size, size(px(60.0), px(30.0)));
    assert_eq!(bounds(cx, "msg-c1-g2").size, size(px(100.0), px(50.0)));
    assert_eq!(bounds(cx, "msg-c1-g3").size, size(px(40.0), px(20.0)));
    // The handler serves no text file: the card stands, with no Retry.
    assert!(at(cx, "msg-c1-g4-failed").is_some());
    assert!(at(cx, "msg-c1-g4-retry").is_none());
}

#[gpui::test]
fn a_file_found_after_its_image_failed_mounts_the_image_anew(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path().canonicalize().unwrap();
    let work = dir.join("work");
    std::fs::create_dir_all(&work).unwrap();
    let cx = local(cx, &dir, "![](late.webp)");
    assert!(at(cx, "msg-c1-g0-failed").is_some());
    assert!(at(cx, "msg-c1-g0-retry").is_none());
    // The file lands at the join, so the address stays; the hook's revision alone remounts it.
    file(&dir, &work.join("late.webp"), (100, 50));
    cx.update(|_, cx| workspace_media::focused(cx));
    for _ in 0..4 {
        settle(cx);
    }
    assert!(at(cx, "msg-c1-g0-failed").is_none());
    assert_eq!(bounds(cx, "msg-c1-g0").size, size(px(100.0), px(50.0)));
}

#[gpui::test]
fn a_data_address_draws_and_a_broken_one_is_unavailable(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let vector = "%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20width%3D%2240%22%20height%3D%2220%22%2F%3E";
    let text = format!("![](data:image/svg+xml,{vector})\n\n![](data:image/png;base64,AAAA)");
    let cx = draw(cx, dir.path(), &text);
    settle(cx);
    assert_eq!(bounds(cx, "msg-c1-g0").size, size(px(40.0), px(20.0)));
    assert!(at(cx, "msg-c1-g1-failed").is_some());
    assert!(at(cx, "msg-c1-g1-retry").is_none());
}
