//! `SessionHistoryButtons` in the chrome cluster and the history shortcuts.

use gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext, px};

use super::screen_tests::{open, wait};
use crate::router;

fn visit(vcx: &mut VisualTestContext, ids: &[&str]) {
    for id in ids {
        vcx.update(|_, cx| router::navigate(&format!("/cc-agent/{id}"), cx));
        wait(vcx, 0);
    }
}

fn here(vcx: &mut VisualTestContext) -> String {
    vcx.update(|_, cx| router::path(cx).to_owned())
}

fn click(vcx: &mut VisualTestContext, selector: &'static str) {
    let at = vcx.debug_bounds(selector).expect("button painted").center();
    vcx.simulate_click(at, Modifiers::none());
    wait(vcx, 0);
}

#[cfg(not(target_os = "macos"))]
#[gpui::test]
fn the_buttons_follow_the_toggle_and_walk_the_tasks(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let mut vcx = open(cx, dir.path());
    let back = vcx.debug_bounds("history-back").expect("back painted");
    let forward = vcx
        .debug_bounds("history-forward")
        .expect("forward painted");
    assert_eq!((back.origin.x, back.size.width), (px(40.0), px(28.0)));
    assert_eq!(forward.origin.x, px(72.0));
    visit(&mut vcx, &["a", "b"]);
    click(&mut vcx, "history-forward");
    assert_eq!(here(&mut vcx), "/cc-agent/b", "no next task");
    click(&mut vcx, "history-back");
    assert_eq!(here(&mut vcx), "/cc-agent/a");
    click(&mut vcx, "history-back");
    assert_eq!(here(&mut vcx), "/cc-agent/a", "no previous task");
    click(&mut vcx, "history-forward");
    assert_eq!(here(&mut vcx), "/cc-agent/b");
}

#[cfg(not(target_os = "macos"))]
#[gpui::test]
fn the_shortcuts_hop_and_pass_at_the_ends(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let mut vcx = open(cx, dir.path());
    visit(&mut vcx, &["a", "b", "c"]);
    vcx.simulate_keystrokes("ctrl-[ ctrl-[");
    wait(&mut vcx, 0);
    assert_eq!(here(&mut vcx), "/cc-agent/a");
    vcx.simulate_keystrokes("ctrl-]");
    wait(&mut vcx, 0);
    assert_eq!(here(&mut vcx), "/cc-agent/b");
}

fn face_width(vcx: &mut VisualTestContext, selector: &'static str) -> gpui::Pixels {
    vcx.debug_bounds(selector).expect("face painted").size.width
}

#[cfg(not(target_os = "macos"))]
#[gpui::test]
fn a_press_pulls_the_face_in_unless_disabled(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let mut vcx = open(cx, dir.path());
    visit(&mut vcx, &["a", "b"]);
    let none = Modifiers::none();
    for (button, face, inset) in [
        ("history-forward", "history-forward-face", px(0.0)),
        ("history-back", "history-back-face", px(2.0)),
    ] {
        let at = vcx.debug_bounds(button).unwrap().center();
        vcx.simulate_mouse_move(at, None, none);
        vcx.simulate_mouse_down(at, MouseButton::Left, none);
        wait(&mut vcx, 100);
        assert_eq!(face_width(&mut vcx, face), px(28.0) - inset, "{button}");
        vcx.simulate_mouse_up(at, MouseButton::Left, none);
        wait(&mut vcx, 100);
        assert_eq!(face_width(&mut vcx, face), px(28.0), "{button} lets go");
    }
}
