//! A resolved file's menu and activation, on the handlers' test recorders.

use gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext};

use crate::{
    copy_media::tests as copies,
    open_external::tests as opener,
    open_path::tests as paths,
    session_view::{
        links::tests::{click, focus, press, toasts},
        target_link::tests::{bounds, in_folder, piece},
        user_row::tests::{at, settle},
    },
    show_item::tests as shows,
};

const CHIP: &str = "msg-c1-md-0-file-0-r0";

fn row(name: &str) -> String {
    format!("menu-row-{name}")
}

fn rows(cx: &mut VisualTestContext, names: &[&str]) -> Vec<bool> {
    names
        .iter()
        .map(|name| at(cx, &row(name)).is_some())
        .collect()
}

/// The menu asked on `selector`, then its row `name` clicked.
fn choose(cx: &mut VisualTestContext, selector: &str, name: &str) {
    press(cx, selector, MouseButton::Right);
    let centre = bounds(cx, &row(name)).center();
    cx.simulate_click(centre, Modifiers::none());
    settle(cx);
}

fn copied(cx: &mut VisualTestContext) -> Option<String> {
    cx.read_from_clipboard().and_then(|item| item.text())
}

const ALL: [&str; 9] = [
    "Open in sidebar",
    "Open in browser",
    "View source",
    "Locate in file list",
    "Open with default app",
    "Show in folder",
    "Copy",
    "Copy path",
    "Copy Location",
];

#[gpui::test]
fn a_files_menu_holds_its_file_actions(cx: &mut TestAppContext) {
    let (dir, root) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let cx = in_folder(
        cx,
        dir.path(),
        root.path(),
        &["src/app.ts"],
        "`src/app.ts:42`\n",
    );
    let path = root.path().join("src/app.ts");
    let path = path.to_str().unwrap();
    press(cx, CHIP, MouseButton::Right);
    let shown = rows(cx, &ALL);
    assert_eq!(
        shown,
        [false, false, false, true, true, true, true, true, true]
    );
    let order = [
        "Locate in file list",
        "Open with default app",
        "Show in folder",
        "Copy",
    ];
    let tops = order.map(|name| bounds(cx, &row(name)).top());
    assert!(tops.is_sorted(), "{tops:?}");
    choose(cx, CHIP, "Locate in file list");
    choose(cx, CHIP, "Copy Location");
    assert_eq!(copied(cx).as_deref(), Some("src/app.ts:42"));
    choose(cx, CHIP, "Copy path");
    assert_eq!(copied(cx).as_deref(), Some(path));
    // The board shows three toasts, the last first.
    let said = [
        "Path copied",
        "Location copied",
        "Failed to open in sidebar file browser",
    ];
    assert_eq!(toasts(cx), said);
    choose(cx, CHIP, "Copy");
    assert_eq!(copies::take(), [path]);
    choose(cx, CHIP, "Show in folder");
    assert_eq!(shows::take(), [path]);
    choose(cx, CHIP, "Open with default app");
    assert_eq!(paths::take(), [std::path::PathBuf::from(path)]);
    assert!(opener::take().is_empty());
}

#[gpui::test]
fn a_separate_activation_opens_the_file_with_its_app(cx: &mut TestAppContext) {
    let (dir, root) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let cx = in_folder(
        cx,
        dir.path(),
        root.path(),
        &["src/app.ts"],
        "`src/app.ts`\n",
    );
    let path = root.path().join("src/app.ts");
    click(cx, CHIP, Modifiers::none());
    assert!(paths::take().is_empty(), "the lightbox comes with 9.13");
    click(cx, CHIP, Modifiers::shift());
    press(cx, CHIP, MouseButton::Middle);
    focus(cx, CHIP);
    cx.simulate_keystrokes("ctrl-enter");
    settle(cx);
    assert_eq!(paths::take(), [path.clone(), path.clone(), path]);
    // A refusal of the handler is the toast.
    paths::fail(Some("no app opens it"));
    click(cx, CHIP, Modifiers::shift());
    paths::fail(None);
    assert_eq!(toasts(cx), ["no app opens it"]);
}

#[gpui::test]
fn a_page_opens_in_a_browser(cx: &mut TestAppContext) {
    let (dir, root) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let files = ["site/index.html"];
    let text = "[the page](site/index.html:3)\n";
    let cx = in_folder(cx, dir.path(), root.path(), &files, text);
    let link = piece(0, 1);
    let url = format!("file://{}/site/index.html", root.path().to_str().unwrap());
    press(cx, &link, MouseButton::Right);
    let shown = rows(cx, &ALL);
    assert_eq!(
        shown,
        [true, true, true, false, false, true, false, true, true]
    );
    choose(cx, &link, "Open in browser");
    click(cx, &link, Modifiers::shift());
    assert_eq!(opener::take(), [url.clone(), url.clone()]);
    // A plain click follows the preference, the sidebar; so does the menu's first row.
    click(cx, &link, Modifiers::none());
    choose(cx, &link, "Open in sidebar");
    assert!(opener::take().is_empty());
    let sidebar = "Failed to open in sidebar browser";
    assert_eq!(toasts(cx), [sidebar, sidebar]);
    // Both opens failing is the handler's code, named in the renderer's words.
    opener::fail(true);
    paths::fail(Some("refused"));
    choose(cx, &link, "Open in browser");
    opener::fail(false);
    paths::fail(None);
    assert_eq!(toasts(cx)[0], "Failed to open in browser");
    choose(cx, &link, "Copy Location");
    assert_eq!(copied(cx).as_deref(), Some("site/index.html:3"));
}

#[gpui::test]
fn a_folders_menu_has_no_file_rows(cx: &mut TestAppContext) {
    let (dir, root) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let cx = in_folder(
        cx,
        dir.path(),
        root.path(),
        &["docs/guide.md"],
        "[docs](docs/)\n",
    );
    // A separate activation still hands the folder to the system.
    press(cx, CHIP, MouseButton::Middle);
    assert_eq!(paths::take().len(), 1);
    press(cx, CHIP, MouseButton::Right);
    let shown = rows(cx, &ALL);
    let want = [false, false, false, true, false, true, true, true, false];
    assert_eq!(shown, want);
}

#[gpui::test]
fn the_menu_key_asks_under_the_chip(cx: &mut TestAppContext) {
    let (dir, root) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let cx = in_folder(
        cx,
        dir.path(),
        root.path(),
        &["src/app.ts"],
        "`src/app.ts`\n",
    );
    focus(cx, CHIP);
    cx.simulate_keystrokes("shift-f10");
    settle(cx);
    let (chip, first) = (bounds(cx, CHIP), bounds(cx, &row("Locate in file list")));
    assert!(
        first.top() >= chip.bottom() && first.left() >= chip.left(),
        "{first:?}"
    );
    assert!(at(cx, &row("Copy Location")).is_none(), "no line was named");
}

#[gpui::test]
fn an_authored_link_to_a_page_opens_it_once_found(cx: &mut TestAppContext) {
    let (dir, root) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let cx = in_folder(
        cx,
        dir.path(),
        root.path(),
        &[],
        "[late](<late page.html>)\n",
    );
    std::fs::write(root.path().join("late page.html"), "").unwrap();
    click(cx, &piece(0, 0), Modifiers::none());
    assert_eq!(toasts(cx), ["Failed to open in sidebar browser"]);
    assert!(opener::take().is_empty());
}

#[gpui::test]
fn a_key_on_a_chip_reaches_the_target_about_it(cx: &mut TestAppContext) {
    let (dir, root) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let files = ["inner.ts", "outer.ts"];
    let text = "[look at `inner.ts`](outer.ts) and [see `inner.ts`](https://example.com/a)\n";
    let cx = in_folder(cx, dir.path(), root.path(), &files, text);
    let (inner, outer) = (root.path().join("inner.ts"), root.path().join("outer.ts"));
    // A click stays with the chip; the key bubbles to the link, as Cindy's keydown does.
    click(cx, CHIP, Modifiers::shift());
    assert_eq!(paths::take(), std::slice::from_ref(&inner));
    focus(cx, CHIP);
    cx.simulate_keystrokes("shift-enter");
    settle(cx);
    assert_eq!(paths::take(), [inner.clone(), outer]);
    focus(cx, "msg-c1-md-0-file-1-r0");
    cx.simulate_keystrokes("shift-space");
    settle(cx);
    assert_eq!(paths::take(), [inner]);
    assert_eq!(opener::take(), ["https://example.com/a"]);
    // The menu key stops at the chip.
    cx.simulate_keystrokes("shift-f10");
    settle(cx);
    assert!(at(cx, &row("Copy path")).is_some() && at(cx, &row("Copy link")).is_none());
}
