//! A Markdown link in an assistant message: its face, and what a click, a key and its menu do.

use gpui::{Bounds, Modifiers, MouseButton, Pixels, TestAppContext, VisualTestContext, px};

use super::{Face, Go, face};
use crate::{
    markdown_target::{Local, Place, Target, link},
    open_external::tests as opener,
    session_view::{
        links::tests::{click, focus, press, toasts},
        user_row::tests::{at, open_in, open_roles, settle},
    },
};

/// The content area with a user message and then this assistant text, `msg-c1`.
fn draw<'a>(
    cx: &'a mut TestAppContext,
    dir: &std::path::Path,
    text: &str,
) -> &'a mut VisualTestContext {
    open_roles(cx, dir, 2400.0, &[("user", "hi"), ("assistant", text)])
}

/// Piece `n` of the first row of block `block`.
pub(super) fn piece(block: usize, n: usize) -> String {
    format!("msg-c1-md-{block}-r0-{n}")
}

pub(super) fn bounds(cx: &mut VisualTestContext, selector: &str) -> Bounds<Pixels> {
    at(cx, selector).unwrap_or_else(|| panic!("no {selector}"))
}

const OPEN: &str = "menu-row-Open link";
const COPY: &str = "menu-row-Copy link";
const BROWSER: &str = "menu-row-Open in browser";

#[test]
fn a_target_draws_as_cindys_renderer_draws_it() {
    let of = |href: &str, bare: bool| face(link(Some(href), &[]), bare, "/work", "label");
    assert!(matches!(of("https://x.y/a", false), Face::External(url) if url == "https://x.y/a"));
    assert!(matches!(of("#a%20b", false), Face::Inline(Go::Heading(id)) if id == "a b"));
    assert!(matches!(
        of("xdt-image://a.png", false),
        Face::Inline(Go::Image)
    ));
    assert!(matches!(
        of("src/app.ts:4", false),
        Face::Inline(Go::Local(Target::Candidate { .. }, dir)) if dir == "/work"
    ));
    // A path the text held bare stays text until it resolves.
    assert!(matches!(of("src/app.ts", true), Face::Span));
    for plain in ["", "mailto:a@b.c", "xdt-audio://a.mp3", "ftp://x.y/a"] {
        assert!(matches!(of(plain, false), Face::Span), "{plain}");
    }
    // A resolved target: a chip when its label reads as a file, a link for prose or a bare path.
    let lit = |kind: Local| Target::Resolved {
        href: "src/a.ts".into(),
        path: "/work/src/a.ts".into(),
        kind,
        at: Place::default(),
    };
    let drawn = |kind: Local, bare: bool, label: &str| match face(lit(kind), bare, "/work", label) {
        Face::File(face, title, _) if title == "src/a.ts" => (face.boxed, face.icon.is_some()),
        _ => panic!("a resolved target is a file's face"),
    };
    assert_eq!(drawn(Local::Text, false, "a.ts"), (true, true));
    assert_eq!(drawn(Local::Text, true, "a.ts"), (false, true));
    assert_eq!(drawn(Local::Text, false, "the code"), (false, true));
    assert_eq!(
        drawn(Local::Text, false, " "),
        (false, false),
        "no label, no icon"
    );
    assert_eq!(drawn(Local::Directory, false, "a.ts"), (true, false));
    assert_eq!(drawn(Local::Directory, false, "a folder"), (false, false));
}

#[test]
fn a_label_reads_as_a_file_as_cindys_does() {
    let all = include_str!("../../../tests/fixtures/smart-paths/label.json");
    let all: Vec<(serde_json::Value, String, bool)> = serde_json::from_str(all).unwrap();
    for (target, label, expected) in &all {
        let number = |key: &str| target[key].as_u64().map(|n| n as u32);
        let at = Place {
            line: number("line"),
            column: number("column"),
        };
        let href = target["href"].as_str().unwrap();
        assert_eq!(
            super::file::code_label(href, at, label),
            *expected,
            "{target} {label:?}"
        );
    }
    assert!(all.len() > 300);
}

#[gpui::test]
fn a_web_link_opens_as_a_message_link_opens(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = draw(cx, dir.path(), "see [the **site**](https://x.y/a) now\n");
    // `see `, `the `, `site`, ` now`: either stretch of the link opens it.
    click(cx, &piece(0, 1), Modifiers::none());
    assert_eq!(opener::take(), ["https://x.y/a"]);
    click(cx, &piece(0, 2), Modifiers::shift());
    assert_eq!(opener::take(), ["https://x.y/a"]);
    press(cx, &piece(0, 2), MouseButton::Middle);
    assert_eq!(opener::take(), ["https://x.y/a"]);
    click(cx, &piece(0, 0), Modifiers::none());
    click(cx, &piece(0, 3), Modifiers::none());
    assert_eq!(opener::take(), Vec::<String>::new(), "text about it");
    // The link is one Tab stop, and its menu is the open-with menu.
    focus(cx, &piece(0, 2));
    cx.simulate_keystrokes("enter");
    settle(cx);
    assert_eq!(opener::take(), ["https://x.y/a"]);
    cx.simulate_keystrokes("shift-f10");
    settle(cx);
    let (first, row) = (bounds(cx, &piece(0, 1)), bounds(cx, BROWSER));
    assert!(row.left() >= first.left() && row.left() < first.left() + px(16.0));
    assert!(row.top() >= first.bottom());
    assert!(at(cx, OPEN).is_none());
    cx.simulate_click(row.center(), Modifiers::none());
    settle(cx);
    assert_eq!(opener::take(), ["https://x.y/a"]);
    assert_eq!(toasts(cx), Vec::<String>::new());
}

#[gpui::test]
fn text_that_is_no_target_is_no_link(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let text = "a [mail](mailto:a@b.c) b [none]() c src/none.ts d\n";
    let cx = draw(cx, dir.path(), text);
    for n in 0..7 {
        if let Some(found) = at(cx, &piece(0, n)) {
            cx.simulate_click(found.center(), Modifiers::none());
            cx.simulate_mouse_down(found.center(), MouseButton::Right, Modifiers::none());
            cx.simulate_mouse_up(found.center(), MouseButton::Right, Modifiers::none());
            settle(cx);
        }
    }
    assert_eq!(opener::take(), Vec::<String>::new());
    assert_eq!(toasts(cx), Vec::<String>::new());
    assert!(at(cx, COPY).is_none());
}

/// How far the block's top lies under the scroller's.
fn under(cx: &mut VisualTestContext, block: &str) -> f32 {
    let (block, port) = (bounds(cx, block), bounds(cx, "session-scroll"));
    (block.top() - port.top()).into()
}

fn frame(cx: &mut VisualTestContext, ms: u64) {
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(ms));
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();
}

#[gpui::test]
fn an_anchor_scrolls_its_heading_to_the_top(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let filler = vec!["para"; 60].join("\n\n");
    let text = format!(
        "[go](#the-tail) [lost](#nowhere) [empty](#) note[^n]\n\n{filler}\n\n\
         ## The tail\n\n{filler}\n\n[^n]: the note\n"
    );
    let cx = draw(cx, dir.path(), &text);
    // Block 61 is the heading, after the first paragraph and sixty more.
    let heading = "msg-c1-md-61";
    let start = under(cx, heading);
    assert!(start > 1000.0, "{start}");
    click(cx, &piece(0, 0), Modifiers::none());
    assert_eq!(under(cx, heading), start, "the scroll is smooth");
    frame(cx, 100);
    let part = under(cx, heading);
    assert!(part > 0.0 && part < start, "{part}");
    frame(cx, 2000);
    assert_eq!(under(cx, heading), 0.0);
    assert_eq!(toasts(cx), Vec::<String>::new());
    // An id no element has, and an empty one.
    cx.update(|window, cx| super::open(&Go::Heading(String::new()), window, cx));
    assert_eq!(toasts(cx), Vec::<String>::new());
    cx.update(|window, cx| super::open(&Go::Heading("nowhere".into()), window, cx));
    settle(cx);
    assert_eq!(toasts(cx), ["Heading not found"]);
    assert_eq!(under(cx, heading), 0.0);
    // The footnote's call goes to its item, and the item's link back to the call.
    cx.update(|window, cx| super::open(&Go::Heading("user-content-fn-n".into()), window, cx));
    frame(cx, 2000);
    let down = under(cx, heading);
    assert!(down < -1000.0, "{down}");
    cx.update(|window, cx| super::open(&Go::Heading("user-content-fnref-n".into()), window, cx));
    frame(cx, 2000);
    // `go`, `lost`, `empty` and ` note` with the spaces between them, then the call.
    let call = under(cx, &piece(0, 6));
    assert!(call.abs() < 0.01, "{call}");
    assert!(under(cx, heading) > 1000.0);
}

#[gpui::test]
fn reduced_motion_jumps_to_the_anchor(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let filler = vec!["para"; 60].join("\n\n");
    let cx = draw(
        cx,
        dir.path(),
        &format!("[go](#tail)\n\n{filler}\n\n# Tail\n\n{filler}\n"),
    );
    cx.update(|_, cx| cx.set_global(crate::motion::ReducedMotion(true)));
    click(cx, &piece(0, 0), Modifiers::none());
    frame(cx, 0);
    assert_eq!(under(cx, "msg-c1-md-61"), 0.0);
}

#[gpui::test]
fn a_footnote_call_is_raised_and_small(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = draw(cx, dir.path(), "xx[^n]\n\n[1](#a)x\n\n[^n]: the note\n");
    let (text, call) = (bounds(cx, &piece(0, 0)), bounds(cx, &piece(0, 1)));
    let plain = bounds(cx, &piece(1, 0));
    assert_eq!(
        bounds(cx, "msg-c1-md-0").size.height,
        px(24.0),
        "`line-height: 0`"
    );
    // 75% of 15px, lifted half of that.
    assert_eq!(call.left(), text.right());
    assert!(call.top() < text.top(), "{call:?}");
    let ratio = call.size.width / plain.size.width;
    assert!((0.70..0.80).contains(&ratio), "{ratio}");
}

/// A task folder with `files`, and the assistant text shown in it.
pub(super) fn in_folder<'a>(
    cx: &'a mut TestAppContext,
    dir: &std::path::Path,
    root: &std::path::Path,
    files: &[&str],
    text: &str,
) -> &'a mut VisualTestContext {
    for file in files {
        let path = root.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "").unwrap();
    }
    let rows = [("user", "hi"), ("assistant", text)];
    let cx = open_in(cx, dir, root.to_str().unwrap(), &rows);
    settle(cx);
    cx
}

#[gpui::test]
fn an_authored_local_link_resolves_again_on_a_click(cx: &mut TestAppContext) {
    let (dir, root) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let text = "[gone](lost.ts) [twice](same.ts) [here](src/app.ts)\n";
    let files = ["a/same.ts", "b/same.ts", "src/app.ts"];
    let cx = in_folder(cx, dir.path(), root.path(), &files, text);
    click(cx, &piece(0, 0), Modifiers::none());
    assert_eq!(
        toasts(cx),
        ["Can't find or access the file. It may have been moved or deleted."]
    );
    click(cx, &piece(0, 2), Modifiers::none());
    let twice = "Found 2 files with the same name, please use an absolute path";
    assert!(
        toasts(cx).iter().any(|toast| toast == twice),
        "{:?}",
        toasts(cx)
    );
    // The file that moved in since the first lookup is found: the click reads past the cache.
    std::fs::write(root.path().join("lost.ts"), "").unwrap();
    click(cx, &piece(0, 0), Modifiers::none());
    assert_eq!(toasts(cx).len(), 2);
    // A link that resolved is a resolved target's: its icon, then its label.
    assert!(at(cx, "msg-c1-md-0-icon-0").is_some());
    press(cx, &piece(0, 5), MouseButton::Right);
    assert!(at(cx, COPY).is_none());
    click(cx, &piece(0, 5), Modifiers::none());
    assert_eq!(toasts(cx).len(), 2, "what it opens comes with 9.13");
    assert_eq!(opener::take(), Vec::<String>::new());
}

/// The mono text's width at the chip's size.
fn mono(cx: &mut VisualTestContext, text: &str) -> f32 {
    let text = text.to_owned();
    cx.update(|window, cx| {
        use ely_gpui_component::theme::ActiveTheme as _;
        let mut style = window.text_style();
        style.font_family = cx.theme().mono_family.clone();
        style.font_size = crate::units::css(14.0).into();
        crate::session_view::flow::draw::shape(&text, &style, window)
            .width
            .into()
    })
}

fn hover(cx: &mut VisualTestContext, selector: &str) {
    let centre = bounds(cx, selector).center();
    cx.simulate_mouse_move(centre, None, Modifiers::none());
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(1000));
    cx.run_until_parked();
}

#[gpui::test]
fn a_label_that_names_the_file_is_a_chip(cx: &mut TestAppContext) {
    let (dir, root) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let text = "a [src/app.ts](src/app.ts) b\n\n[docs](docs/) [read the app](src/app.ts)\n";
    let files = ["src/app.ts", "docs/guide.md"];
    let cx = in_folder(cx, dir.path(), root.path(), &files, text);
    // `px-1` about the icon with its `mr-1` and the label, in the code font at 14px.
    let (chip, icon) = (
        bounds(cx, "msg-c1-md-0-file-0-r0"),
        bounds(cx, "msg-c1-md-0-icon-0"),
    );
    let label = mono(cx, "src/app.ts");
    assert!(
        (f32::from(chip.size.width) - label - 8.0 - 18.0).abs() < 1.01,
        "{chip:?}"
    );
    assert_eq!(icon.left(), chip.left() + px(4.0));
    assert_eq!(icon.size.width, px(18.0));
    let before = bounds(cx, &piece(0, 0));
    assert_eq!(chip.left(), before.right());
    assert_eq!(
        f32::from(bounds(cx, "msg-c1-md-0").size.height).floor(),
        24.0
    );
    // A folder's chip has no icon; a label of prose is a link with the file's icon.
    let folder = bounds(cx, "msg-c1-md-1-file-0-r0");
    assert!((f32::from(folder.size.width) - mono(cx, "docs") - 8.0).abs() < 1.01);
    let only = bounds(cx, "msg-c1-md-1-icon-0");
    assert!(
        only.left() > folder.right(),
        "the link's icon, not the folder's"
    );
    assert!(at(cx, "msg-c1-md-1-icon-1").is_none());
    assert!(at(cx, "msg-c1-md-1-file-1-r0").is_none());
    // The chip's title is the link's address, and a click opens nothing before 9.13.
    hover(cx, "msg-c1-md-0-file-0-r0");
    assert!(at(cx, "tip").is_some());
    click(cx, "msg-c1-md-0-file-0-r0", Modifiers::none());
    press(cx, "msg-c1-md-0-file-0-r0", MouseButton::Middle);
    assert_eq!(toasts(cx), Vec::<String>::new());
    assert_eq!(opener::take(), Vec::<String>::new());
}

#[gpui::test]
fn an_inline_code_that_resolves_is_a_chip(cx: &mut TestAppContext) {
    let (dir, root) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let text = "`app.ts` and `nope.ts` and [`app.ts`](src/app.ts)\n";
    let cx = in_folder(cx, dir.path(), root.path(), &["src/app.ts"], text);
    let (chip, label) = (bounds(cx, "msg-c1-md-0-file-0-r0"), mono(cx, "app.ts"));
    assert!((f32::from(chip.size.width) - label - 26.0).abs() < 1.01);
    // Code that names no file keeps inline code's box: `px-[0.4em]` of 14px.
    let plain = bounds(cx, &piece(0, 3));
    assert!((f32::from(plain.size.width) - mono(cx, "nope.ts") - 11.2).abs() < 1.01);
    // A chip in a chip: the link's box about the code's own.
    let (outer, inner) = (
        bounds(cx, "msg-c1-md-0-file-1-r0"),
        bounds(cx, "msg-c1-md-0-file-2-r0"),
    );
    assert!((f32::from(inner.size.width) - label - 26.0).abs() < 1.01);
    assert_eq!(inner.left(), outer.left() + px(4.0 + 18.0));
    assert_eq!(inner.right() + px(4.0), outer.right());
    // Each is its own Tab stop, with its own title.
    hover(cx, "msg-c1-md-0-file-0-r0");
    assert!(at(cx, "tip").is_some());
}

#[gpui::test]
fn a_long_chip_breaks_anywhere_and_keeps_its_ends(cx: &mut TestAppContext) {
    let (dir, root) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let name = format!("{}.ts", "long".repeat(60));
    let text = format!("`{name}`\n");
    let cx = in_folder(cx, dir.path(), root.path(), &[&name], &text);
    let (first, second) = (
        bounds(cx, "msg-c1-md-0-file-0-r0"),
        bounds(cx, "msg-c1-md-0-file-0-r1"),
    );
    let block = bounds(cx, "msg-c1-md-0");
    assert_eq!(first.left(), block.left());
    assert_eq!(second.left(), block.left());
    assert!(first.right() <= block.right() && second.top() > first.top());
    // The icon and the left padding stand on the first row alone.
    assert!(at(cx, "msg-c1-md-0-icon-0").is_some());
    let text = bounds(cx, &piece(0, 1));
    assert_eq!(text.left(), first.left() + px(22.0));
}

#[gpui::test]
fn an_inline_links_menu_opens_and_copies(cx: &mut TestAppContext) {
    let (dir, root) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let text = "[gone](<my lost.ts>) and [top](#nowhere)\n";
    let cx = in_folder(cx, dir.path(), root.path(), &[], text);
    press(cx, &piece(0, 0), MouseButton::Right);
    let (open, copy) = (bounds(cx, OPEN), bounds(cx, COPY));
    assert!(open.top() < copy.top());
    assert!(at(cx, BROWSER).is_none());
    cx.simulate_click(copy.center(), Modifiers::none());
    settle(cx);
    let copied = cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(copied.as_deref(), Some("my lost.ts"), "as it was written");
    assert_eq!(toasts(cx), ["Link copied"]);
    press(cx, &piece(0, 2), MouseButton::Right);
    let open = bounds(cx, OPEN).center();
    cx.simulate_click(open, Modifiers::none());
    settle(cx);
    assert!(toasts(cx).iter().any(|toast| toast == "Heading not found"));
    // Keys: Space activates, the menu key asks under the link.
    focus(cx, &piece(0, 2));
    cx.simulate_keystrokes("space");
    settle(cx);
    assert_eq!(toasts(cx).len(), 3);
}

#[gpui::test]
fn every_row_of_a_link_opens_it(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let words = "words ".repeat(70);
    let text = format!("[first\\\nsecond](https://x.y/break)\n\n[{words}](https://x.y/long)\n");
    let cx = draw(cx, dir.path(), &text);
    for (row, href) in [
        ("msg-c1-md-0-r0-0", "https://x.y/break"),
        ("msg-c1-md-0-r1-0", "https://x.y/break"),
        ("msg-c1-md-1-r0-0", "https://x.y/long"),
        ("msg-c1-md-1-r1-0", "https://x.y/long"),
        ("msg-c1-md-1-r2-0", "https://x.y/long"),
    ] {
        click(cx, row, Modifiers::none());
        assert_eq!(opener::take(), [href], "{row}");
    }
}

#[gpui::test]
fn a_link_about_an_empty_code_is_a_link(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let text = "[` `](https://x.y/empty)\n\n[` `](#missing)\n";
    let cx = draw(cx, dir.path(), text);
    click(cx, "msg-c1-md-0-pill-0", Modifiers::none());
    assert_eq!(opener::take(), ["https://x.y/empty"]);
    press(cx, "msg-c1-md-1-pill-0", MouseButton::Right);
    let open = bounds(cx, OPEN).center();
    assert!(at(cx, COPY).is_some());
    cx.simulate_click(open, Modifiers::none());
    settle(cx);
    assert_eq!(toasts(cx), ["Heading not found"]);
}

#[gpui::test]
fn an_authored_title_is_the_links_tip(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let text = "[site](https://x.y/ \"Destination note\") [bare](https://x.y/b)\n";
    let cx = draw(cx, dir.path(), text);
    let hover = |cx: &mut VisualTestContext, n: usize| {
        let centre = bounds(cx, &piece(0, n)).center();
        cx.simulate_mouse_move(centre, None, Modifiers::none());
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(1000));
        cx.run_until_parked();
    };
    hover(cx, 2);
    assert!(at(cx, "tip").is_none(), "no title, no tip");
    hover(cx, 0);
    assert!(at(cx, "tip").is_some());
}

#[gpui::test]
fn a_chip_inside_a_link_keeps_its_own_clicks_and_title(cx: &mut TestAppContext) {
    let (dir, root) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let text = "[see `app.ts`](https://x.y/outer \"Outer\") [tail](https://x.y/tail)\n\n`app.ts`\n";
    let cx = in_folder(cx, dir.path(), root.path(), &["app.ts"], text);
    // `see `, then the chip's icon and label: the chip takes its clicks, the link the rest.
    for inner in ["msg-c1-md-0-icon-0", &piece(0, 2)] {
        click(cx, inner, Modifiers::none());
        press(cx, inner, MouseButton::Right);
        assert_eq!(opener::take(), Vec::<String>::new(), "{inner}");
        assert!(at(cx, BROWSER).is_none(), "{inner}");
    }
    click(cx, &piece(0, 0), Modifiers::none());
    assert_eq!(opener::take(), ["https://x.y/outer"]);
    // The chip's title is the file's path, as the same code's outside a link.
    let tip = |cx: &mut VisualTestContext, selector: &str| {
        hover(cx, selector);
        bounds(cx, "tip").size
    };
    let (nested, alone) = (tip(cx, &piece(0, 2)), tip(cx, &piece(1, 1)));
    assert_eq!(nested, alone);
    assert!(
        tip(cx, &piece(0, 0)).width < nested.width,
        "the link's own title"
    );
    // The link is a Tab stop before its chip, and the next link follows the chip.
    focus(cx, &piece(0, 2));
    cx.simulate_keystrokes("enter");
    settle(cx);
    // The chip has the focus; its key bubbles to the link, as Cindy's keydown does.
    assert_eq!(opener::take(), ["https://x.y/outer"]);
    cx.update(|window, cx| window.focus_next(cx));
    settle(cx);
    cx.simulate_keystrokes("enter");
    settle(cx);
    assert_eq!(opener::take(), ["https://x.y/tail"]);
}

#[gpui::test]
fn a_link_is_a_tab_stop_before_the_chips_it_holds(cx: &mut TestAppContext) {
    let (dir, root) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let text = "[before](https://x.y/before) [`app.ts` and `docs/guide.md`](https://x.y/outer) \
                [tail](https://x.y/tail)";
    let files = ["app.ts", "docs/guide.md"];
    let cx = in_folder(cx, dir.path(), root.path(), &files, text);
    focus(cx, &piece(0, 0));
    let mut opened = Vec::new();
    for _ in 0..4 {
        cx.update(|window, cx| window.focus_next(cx));
        settle(cx);
        cx.simulate_keystrokes("enter");
        settle(cx);
        opened.push(opener::take().join(" "));
    }
    // The link, its two chips, whose key bubbles to it, then the next link.
    let outer = "https://x.y/outer";
    assert_eq!(opened, [outer, outer, outer, "https://x.y/tail"]);
}

mod deep;
