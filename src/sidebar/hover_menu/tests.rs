use std::time::Duration;

use gpui::{App, Global, TestAppContext, VisualTestContext};

use super::{HoverMenu, close, enter_content, enter_trigger, leave, open};

#[derive(Default)]
struct Held(HoverMenu);

impl Global for Held {}

fn menu(cx: &mut App) -> &mut HoverMenu {
    &mut cx.global_mut::<Held>().0
}

fn at(cx: &mut VisualTestContext, ms: u64) {
    cx.executor().advance_clock(Duration::from_millis(ms));
    cx.run_until_parked();
}

fn shown(cx: &mut VisualTestContext) -> bool {
    cx.update(|_, cx| cx.global::<Held>().0.is_open())
}

fn start(cx: &mut TestAppContext) -> &mut VisualTestContext {
    cx.update(|cx| cx.set_global(Held::default()));
    cx.add_empty_window()
}

#[gpui::test]
fn hover_opens_after_90ms_without_a_focus_to_return(cx: &mut TestAppContext) {
    let cx = start(cx);
    cx.update(|window, cx| enter_trigger(menu, window, cx));
    at(cx, 89);
    assert!(!shown(cx));
    at(cx, 1);
    assert!(shown(cx));
    assert!(cx.update(|_, cx| cx.global::<Held>().0.back.is_none()));
}

#[gpui::test]
fn a_pass_over_the_trigger_opens_nothing(cx: &mut TestAppContext) {
    let cx = start(cx);
    cx.update(|window, cx| {
        enter_trigger(menu, window, cx);
        leave(menu, window, cx);
    });
    at(cx, 200);
    assert!(!shown(cx));
}

#[gpui::test]
fn leaving_closes_after_160ms_unless_the_content_is_reached(cx: &mut TestAppContext) {
    let cx = start(cx);
    cx.update(|window, cx| enter_trigger(menu, window, cx));
    at(cx, 90);
    cx.update(|window, cx| leave(menu, window, cx));
    at(cx, 100);
    cx.update(|_, cx| enter_content(menu, cx));
    at(cx, 200);
    assert!(shown(cx), "the gap to the menu was crossed");
    cx.update(|window, cx| leave(menu, window, cx));
    at(cx, 159);
    assert!(shown(cx));
    at(cx, 1);
    assert!(!shown(cx));
}

#[gpui::test]
fn a_click_open_returns_focus_on_any_close_but_a_press_outside(cx: &mut TestAppContext) {
    let cx = start(cx);
    let trigger = cx.update(|_, cx| cx.focus_handle());
    let elsewhere = cx.update(|_, cx| cx.focus_handle());
    cx.update(|window, cx| {
        open(menu, trigger.clone(), window, cx);
        window.focus(&elsewhere, cx);
        leave(menu, window, cx);
    });
    at(cx, 160);
    assert!(!shown(cx));
    assert!(
        cx.update(|window, _| trigger.is_focused(window)),
        "the timed close focused it"
    );
    cx.update(|window, cx| {
        open(menu, trigger.clone(), window, cx);
        window.focus(&elsewhere, cx);
        close(menu, true, window, cx);
    });
    assert!(cx.update(|window, _| elsewhere.is_focused(window)));
}
