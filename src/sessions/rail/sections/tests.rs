use std::time::Duration;

use gpui::{KeyUpEvent, Keystroke, Modifiers, TestAppContext, VisualTestContext, point, px};

use super::{Panels, Section};
use crate::sessions::rail::pinned::tests::{center, hover, rail};

fn state(cx: &mut VisualTestContext) -> (Option<Section>, bool) {
    cx.update(|_, cx| {
        let p = cx.default_global::<Panels>();
        (p.open, p.via_keyboard)
    })
}

fn tile(cx: &mut VisualTestContext, id: &'static str) -> gpui::Bounds<gpui::Pixels> {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.debug_bounds(id).unwrap_or_else(|| panic!("{id} drawn"))
}

fn wait(cx: &mut VisualTestContext, ms: u64) {
    cx.executor().advance_clock(Duration::from_millis(ms));
    cx.run_until_parked();
}

fn away(cx: &mut VisualTestContext) {
    hover(cx, point(px(600.0), px(500.0)));
}

#[gpui::test]
fn hover_opens_at_the_tile_and_leaving_closes_after_the_grace(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    let projects = tile(cx, "rail-projects");
    hover(cx, projects.center());
    assert_eq!(state(cx), (Some(Section::Projects), false));
    let anchor = cx.update(|_, cx| cx.global::<Panels>().anchor);
    assert_eq!(anchor, Some(projects));
    away(cx);
    wait(cx, 119);
    assert_eq!(state(cx).0, Some(Section::Projects), "within the grace");
    wait(cx, 1);
    assert_eq!(state(cx).0, None);
}

#[gpui::test]
fn coming_back_within_the_grace_keeps_it_and_the_other_tile_switches(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    let projects = tile(cx, "rail-projects").center();
    hover(cx, projects);
    away(cx);
    wait(cx, 60);
    hover(cx, projects);
    wait(cx, 200);
    assert_eq!(
        state(cx).0,
        Some(Section::Projects),
        "the close was cancelled"
    );
    let dialogues = tile(cx, "rail-dialogues");
    hover(cx, dialogues.center());
    assert_eq!(state(cx).0, Some(Section::Dialogues));
    let anchor = cx.update(|_, cx| cx.global::<Panels>().anchor);
    assert_eq!(anchor, Some(dialogues));
}

fn enter(cx: &mut VisualTestContext) {
    cx.simulate_keystrokes("enter");
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").unwrap(),
    });
    cx.run_until_parked();
}

/// A click focuses the tile; Enter then clicks it from the keyboard.
fn focus_projects(cx: &mut VisualTestContext) {
    let at = tile(cx, "rail-projects").center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(state(cx), (Some(Section::Projects), false), "a mouse click");
}

#[gpui::test]
fn a_keyboard_open_takes_no_hover_close(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    focus_projects(cx);
    enter(cx);
    assert_eq!(state(cx), (Some(Section::Projects), true));
    away(cx);
    wait(cx, 500);
    assert_eq!(state(cx).0, Some(Section::Projects));
}

#[gpui::test]
fn an_open_hides_the_pin_preview_and_a_pin_click_closes_it(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    focus_projects(cx);
    let pin = center(cx, "a0");
    hover(cx, pin);
    wait(cx, 120);
    assert_eq!(state(cx).0, None, "the pointer left the tile");
    assert!(cx.debug_bounds("rail-pin-preview").is_some());
    enter(cx);
    assert_eq!(state(cx), (Some(Section::Projects), true));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("rail-pin-preview").is_none(),
        "openSectionAt clears it"
    );
    cx.simulate_click(pin, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(state(cx).0, None, "handlePinnedClick closes all");
}
