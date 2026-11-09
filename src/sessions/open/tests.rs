use gpui::{
    Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, TestAppContext, VisualTestContext,
};

use crate::{
    router,
    sessions::{
        Sessions,
        view_mode::{self, Main, Pinned},
        view_tests::open,
    },
};

fn press(cx: &mut VisualTestContext, id: &str, modifiers: Modifiers, count: usize) {
    let selector: &'static str = Box::leak(format!("task-{id}").into_boxed_str());
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} not drawn"));
    // The status icon's spot, clear of the title and the rename field.
    let at = gpui::point(bounds.origin.x + gpui::px(20.0), bounds.center().y);
    for click_count in 1..=count {
        cx.simulate_event(MouseDownEvent {
            position: at,
            modifiers,
            button: MouseButton::Left,
            click_count,
            first_mouse: false,
        });
        cx.simulate_event(MouseUpEvent {
            position: at,
            modifiers,
            button: MouseButton::Left,
            click_count,
        });
    }
    cx.run_until_parked();
}

fn here(cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| router::path(cx).to_owned())
}

fn ringed(cx: &mut VisualTestContext, id: &str) -> bool {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.debug_bounds(Box::leak(format!("task-active-{id}").into_boxed_str()))
        .is_some()
}

#[gpui::test]
fn a_click_opens_the_task_and_draws_it_active(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    assert_eq!(here(cx), "/cc-agent/a0", "the index opens the first task");
    assert!(ringed(cx, "a0"));
    press(cx, "a2", Modifiers::none(), 1);
    assert_eq!(here(cx), "/cc-agent/a2");
    assert!(ringed(cx, "a2") && !ringed(cx, "a0"), "one active row");
}

#[gpui::test]
fn a_modifier_click_does_not_open(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    for modifiers in [
        Modifiers::shift(),
        Modifiers::control(),
        Modifiers::command(),
    ] {
        press(cx, "a2", modifiers, 1);
        assert_eq!(here(cx), "/cc-agent/a0", "{modifiers:?}");
    }
}

#[gpui::test]
fn a_double_click_opens_then_renames(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    press(cx, "a2", Modifiers::none(), 2);
    assert_eq!(here(cx), "/cc-agent/a2");
    let editing = cx.update(|_, cx| {
        cx.global::<Sessions>()
            .editing
            .as_ref()
            .map(|e| e.id.clone())
    });
    assert_eq!(editing.as_deref(), Some("a2"));
}

#[gpui::test]
fn a_click_beside_the_rename_field_commits_then_opens(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    press(cx, "a2", Modifiers::none(), 2);
    cx.update(|_, cx| router::navigate("/cc-agent/a1", cx));
    cx.run_until_parked();
    press(cx, "a2", Modifiers::none(), 1);
    let editing = cx.update(|_, cx| cx.global::<Sessions>().editing.is_some());
    assert!(!editing, "the press outside the field committed it");
    assert_eq!(here(cx), "/cc-agent/a2");
}

#[gpui::test]
fn a_list_row_opens_and_draws_it_active(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    cx.update(|_, cx| view_mode::set_main(Main::List, cx));
    cx.run_until_parked();
    press(cx, "a1", Modifiers::none(), 1);
    assert_eq!(here(cx), "/cc-agent/a1");
    assert!(ringed(cx, "a1"));
}

#[gpui::test]
fn a_pinned_card_opens_and_draws_it_active(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    cx.update(|window, cx| {
        let loaded = cx.global_mut::<Sessions>().loaded.as_mut().unwrap();
        loaded.sessions[2].pinned_at = Some(1);
        view_mode::set_pinned(Pinned::Card, cx);
        window.refresh();
    });
    cx.run_until_parked();
    press(cx, "a2", Modifiers::none(), 1);
    assert_eq!(here(cx), "/cc-agent/a2");
    assert!(cx.update(|_, cx| super::is_active("a2", cx)));
}
