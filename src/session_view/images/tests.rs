use std::{path::Path, time::Duration};

use gpui::{Modifiers, TestAppContext, VisualTestContext, px, size};
use image::{ExtendedColorType, ImageEncoder as _, codecs::webp::WebPEncoder};

use super::{Load, fitted, held_mut, inline, long};
use crate::session_view::user_row::tests::{at, open_json, settle};

/// A WebP of this size in the task's image cache.
pub(in crate::session_view) fn put(dir: &Path, name: &str, (width, height): (u32, u32)) {
    let folder = dir.join("cc-agent/images/s1");
    std::fs::create_dir_all(&folder).unwrap();
    let pixels = vec![200u8; (width * height * 4) as usize];
    let mut bytes = Vec::new();
    let encoder = WebPEncoder::new_lossless(&mut bytes);
    encoder
        .write_image(&pixels, width, height, ExtendedColorType::Rgba8)
        .unwrap();
    std::fs::write(folder.join(name), bytes).unwrap();
}

/// A user row's content: its text and its images by file name.
fn content(text: &str, names: &[&str]) -> String {
    let image = |name: &&str| {
        let url = format!("xdt-image://s1/{name}");
        serde_json::json!({ "url": url, "mimeType": "image/webp", "originalName": name })
    };
    let images: Vec<serde_json::Value> = names.iter().map(image).collect();
    serde_json::json!({ "text": text, "images": images }).to_string()
}

/// `s1` open on one message with these images, 2400 wide.
fn open<'a>(cx: &'a mut TestAppContext, dir: &Path, names: &[&str]) -> &'a mut VisualTestContext {
    open_json(cx, dir, 2400.0, &[&content("hi", names)])
}

#[test]
fn an_image_keeps_its_ratio_inside_280_by_180() {
    assert_eq!(fitted((40, 30)), (40.0, 30.0));
    assert_eq!(fitted((280, 180)), (280.0, 180.0));
    assert_eq!(fitted((560, 180)), (280.0, 90.0));
    assert_eq!(fitted((280, 360)), (140.0, 180.0));
    assert_eq!(fitted((1120, 1440)), (140.0, 180.0));
    assert_eq!(fitted((100, 300)), (60.0, 180.0));
}

#[test]
fn a_long_image_is_over_three_times_as_tall() {
    assert!(!long((100, 300)));
    assert!(long((100, 301)));
    assert!(!long((0, 10)));
}

#[gpui::test]
fn images_stand_right_over_the_bubble(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    put(dir.path(), "wide.webp", (560, 180));
    put(dir.path(), "small.webp", (40, 30));
    let cx = open(cx, dir.path(), &["wide.webp", "small.webp"]);
    let (wide, small, bubble) = (
        at(cx, "msg-c0-image-0").unwrap(),
        at(cx, "msg-c0-image-1").unwrap(),
        at(cx, "msg-c0-bubble").unwrap(),
    );
    assert_eq!(wide.size, size(px(280.0), px(90.0)));
    assert_eq!(small.size, size(px(40.0), px(30.0)));
    // Each is the column's child: set right, `gap-2` apart, the bubble last.
    assert_eq!(
        (wide.right(), small.right()),
        (bubble.right(), bubble.right())
    );
    assert_eq!(wide.bottom() + px(8.0), small.top());
    assert_eq!(small.bottom() + px(8.0), bubble.top());
    assert!(at(cx, "msg-c0-image-0-hint").is_none());
}

#[gpui::test]
fn images_stand_over_the_files(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    put(dir.path(), "a.webp", (40, 30));
    let url = "xdt-image://s1/a.webp";
    let row = serde_json::json!({
        "text": "",
        "images": [{ "url": url, "mimeType": "image/webp", "originalName": "a.webp" }],
        "files": [{ "name": "a.txt", "path": "/tmp/a.txt" }],
    });
    let cx = open_json(cx, dir.path(), 2400.0, &[&row.to_string()]);
    let (image, files) = (at(cx, "msg-c0-image-0"), at(cx, "msg-c0-files"));
    assert_eq!(image.unwrap().bottom() + px(8.0), files.unwrap().top());
    assert!(at(cx, "msg-c0-bubble").is_none());
}

#[gpui::test]
fn a_long_image_shows_its_square_top_and_a_hint(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    put(dir.path(), "long.webp", (100, 400));
    let cx = open(cx, dir.path(), &["long.webp"]);
    let (frame, hint) = (
        at(cx, "msg-c0-image-0").unwrap(),
        at(cx, "msg-c0-image-0-hint").unwrap(),
    );
    assert_eq!(frame.size, size(px(240.0), px(240.0)));
    // `bottom-2 left-2`, never wider than the frame less 16.
    assert_eq!(hint.left(), frame.left() + px(8.0));
    assert_eq!(hint.bottom(), frame.bottom() - px(8.0));
    assert!(hint.size.width <= px(224.0));
    let top = cx.update(
        |_, cx| match &held_mut(cx).unwrap().0["xdt-image://s1/long.webp"] {
            Load::Ready(shown) => shown.top.as_ref().unwrap().size(0),
            _ => panic!("the image loaded"),
        },
    );
    assert_eq!((top.width.0, top.height.0), (100, 100));
}

#[gpui::test]
fn a_missing_image_and_an_unreadable_one_say_so(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    put(dir.path(), "ok.webp", (40, 30));
    std::fs::write(dir.path().join("cc-agent/images/s1/bad.webp"), b"nope").unwrap();
    let cx = open(cx, dir.path(), &["gone.webp", "bad.webp"]);
    let state = |cx: &mut VisualTestContext, name: &str| {
        let url = format!("xdt-image://s1/{name}");
        cx.update(|_, cx| held_mut(cx).unwrap().0[&url].clone())
    };
    assert!(matches!(state(cx, "gone.webp"), Load::Missing));
    assert!(matches!(state(cx, "bad.webp"), Load::Undecoded));
    let (gone, bad) = (
        at(cx, "msg-c0-image-0-failed").unwrap(),
        at(cx, "msg-c0-image-1-failed").unwrap(),
    );
    // `minHeight: 140`; the label sets the width: `Image missing`, `Image Unavailable`.
    assert_eq!((gone.size.height, bad.size.height), (px(140.0), px(140.0)));
    assert!(gone.size.width < bad.size.width);
    assert_eq!(gone.right(), at(cx, "msg-c0-bubble").unwrap().right());
    assert!(at(cx, "msg-c0-image-0").is_none());
}

#[gpui::test]
fn a_failed_image_never_outgrows_280_and_tips_its_name(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let name = format!("{}.webp", "n".repeat(120));
    let cx = open(cx, dir.path(), &[&name]);
    let failed = at(cx, "msg-c0-image-0-failed").unwrap();
    eprintln!("{failed:?}");
    assert_eq!(failed.size.width, px(280.0));
    // The name keeps to the box's `p-4`.
    let name = at(cx, "msg-c0-image-0-name").unwrap();
    assert!(name.size.width <= px(246.0) && name.size.width > px(230.0));
    let point = name.center();
    cx.simulate_mouse_move(point, None, Modifiers::none());
    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    assert!(at(cx, "tip-mono").is_some());
}

#[gpui::test]
fn a_failed_images_name_draws_on_one_line(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("cc-agent/images/s1")).unwrap();
    std::fs::write(dir.path().join("cc-agent/images/s1/bad.png"), b"nope").unwrap();
    let image = |url: &str, name: &str| serde_json::json!({ "url": url, "mimeType": "image/png", "originalName": name });
    let (gone, bad) = ("xdt-image://s1/gone.png", "xdt-image://s1/bad.png");
    let images = [
        image(gone, "a\n\tb  c.png"),
        image(gone, "a b c.png"),
        image(bad, "a\r\nb.png"),
    ];
    let row = serde_json::json!({ "text": "", "images": images });
    let cx = open_json(cx, dir.path(), 2400.0, &[&row.to_string()]);
    let name = |cx: &mut VisualTestContext, n| at(cx, &format!("msg-c0-image-{n}-name")).unwrap();
    // White space collapses as `nowrap` draws it: both names take one line of one width.
    assert_eq!(name(cx, 0).size, name(cx, 1).size);
    assert_eq!(name(cx, 2).size.height, name(cx, 1).size.height);
    assert!(at(cx, "msg-c0-image-2-failed").is_some());
}

#[gpui::test]
fn an_image_shows_its_frame_while_it_loads(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    put(dir.path(), "a.webp", (40, 30));
    let cx = open(cx, dir.path(), &["a.webp"]);
    let url = "xdt-image://s1/a.webp";
    cx.update(|_, cx| *held_mut(cx).unwrap().0.get_mut(url).unwrap() = Load::Loading);
    settle(cx);
    let (frame, bubble) = (
        at(cx, "msg-c0-image-0-loading").unwrap(),
        at(cx, "msg-c0-bubble").unwrap(),
    );
    assert_eq!(frame.size, size(px(280.0), px(180.0)));
    assert!(at(cx, "msg-c0-image-0").is_none());
    assert_eq!(frame.bottom() + px(8.0), bubble.top());
}

#[gpui::test]
fn the_loading_frame_stands_at_the_columns_start(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    put(dir.path(), "a.webp", (40, 30));
    let row = content(&"wide ".repeat(40), &["a.webp"]);
    let cx = open_json(cx, dir.path(), 2400.0, &[&row]);
    let image = at(cx, "msg-c0-image-0").unwrap();
    let url = "xdt-image://s1/a.webp";
    cx.update(|_, cx| *held_mut(cx).unwrap().0.get_mut(url).unwrap() = Load::Loading);
    settle(cx);
    // `self-start` in a column the bubble makes 488 wide; the image itself stands right.
    let (frame, bubble) = (
        at(cx, "msg-c0-image-0-loading"),
        at(cx, "msg-c0-bubble").unwrap(),
    );
    assert_eq!(bubble.size.width, px(488.0));
    assert_eq!(frame.unwrap().left(), bubble.left());
    assert_eq!(image.right(), bubble.right());
}

#[gpui::test]
fn one_address_loads_once(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    put(dir.path(), "a.webp", (40, 30));
    let cx = open(cx, dir.path(), &["a.webp", "a.webp"]);
    assert_eq!(cx.update(|_, cx| held_mut(cx).unwrap().0.len()), 1);
    assert!(at(cx, "msg-c0-image-1").is_some());
    // The file goes; the image held stays, as no new load is asked.
    std::fs::remove_file(dir.path().join("cc-agent/images/s1/a.webp")).unwrap();
    settle(cx);
    assert!(at(cx, "msg-c0-image-0").is_some());
}

#[gpui::test]
fn a_blob_address_reads_the_media_store(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    put(dir.path(), "a.webp", (40, 30));
    let hash = "ab".repeat(32);
    let folder = dir.path().join("cindy-media/blobs/ab");
    std::fs::create_dir_all(&folder).unwrap();
    let from = dir.path().join("cc-agent/images/s1/a.webp");
    std::fs::rename(from, folder.join(format!("{hash}.webp"))).unwrap();
    let url = format!("cindy-media://blobs/{hash}.webp");
    let row = serde_json::json!({
        "text": "hi",
        "images": [{ "url": url, "mimeType": "image/webp", "originalName": "a.webp" }],
    });
    let cx = open_json(cx, dir.path(), 2400.0, &[&row.to_string()]);
    assert_eq!(
        at(cx, "msg-c0-image-0").unwrap().size,
        size(px(40.0), px(30.0))
    );
}

#[test]
fn a_data_address_loads_where_blinks_image_does() {
    let cases = include_str!("../../../tests/fixtures/data-image/blink.json");
    let cases: Vec<(String, Option<(u32, u32)>)> = serde_json::from_str(cases).unwrap();
    assert!(cases.len() > 40);
    for (url, expected) in cases {
        let svg = gpui::SvgRenderer::new(std::sync::Arc::new(()));
        let natural = inline(&url, svg).ok().map(|shown| shown.natural);
        assert_eq!(natural, expected, "{url}");
    }
}
