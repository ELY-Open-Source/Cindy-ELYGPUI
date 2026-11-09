use gpui::{KeyUpEvent, Keystroke, Modifiers, TestAppContext};

use crate::{router, sidebar::tests::open};

#[gpui::test]
fn new_opens_the_draft(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (_row, cx) = open(cx, dir.path(), "{}");
    let new = cx.debug_bounds("nav-new").expect("New drawn");
    cx.simulate_click(new.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        cx.update(|_, cx| router::path(cx).to_owned()),
        "/cc-agent/new"
    );
}

#[gpui::test]
fn new_keeps_off_the_rail(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (_row, cx) = open(cx, dir.path(), r#"{"sidebar-rail":"true"}"#);
    assert!(cx.debug_bounds("nav-new").is_none());
}

#[gpui::test]
fn enter_on_new_opens_the_draft(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (_row, cx) = open(cx, dir.path(), "{}");
    // The first tab stop, as Tab from the window root reaches it.
    cx.update(|window, cx| window.focus_next(cx));
    cx.simulate_keystrokes("enter");
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").unwrap(),
    });
    cx.run_until_parked();
    assert_eq!(
        cx.update(|_, cx| router::path(cx).to_owned()),
        "/cc-agent/new"
    );
}
