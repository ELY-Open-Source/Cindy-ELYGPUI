use gpui::{
    Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, ScrollDelta, ScrollEvent, TestAppContext,
    TouchPhase, VisualTestContext, point, px, size,
};
use rusqlite::Connection;

use crate::{
    localdb::search::conversations::tests::task,
    sessions::{
        reveal::Reveal,
        search::{
            Search,
            run::tests::{search, wait},
        },
        view_tests::click,
    },
};

fn offset(cx: &mut VisualTestContext) -> f32 {
    cx.update(|_, cx| f32::from(cx.global::<Reveal>().handle.offset().y))
}

fn scroll(cx: &mut VisualTestContext, by: f32) {
    let list = cx.debug_bounds("main-list").unwrap();
    cx.simulate_event(ScrollEvent {
        position: list.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-by))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
        ..Default::default()
    });
    cx.run_until_parked();
}

fn plans(db: &Connection) {
    for i in 0..30 {
        task(db, &format!("p{i}"), &format!("plan {i}"), i);
    }
}

/// The search over a short window, the list scrolled down 60.
fn scrolled<'a>(cx: &'a mut TestAppContext, dir: &std::path::Path) -> &'a mut VisualTestContext {
    let cx = search(cx, dir, Some(plans));
    cx.simulate_resize(size(px(400.0), px(200.0)));
    cx.run_until_parked();
    scroll(cx, 60.0);
    assert_eq!(offset(cx), -60.0);
    cx
}

#[gpui::test]
fn a_search_starts_at_the_top_and_clear_puts_the_list_back(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = scrolled(cx, dir.path());
    cx.simulate_input("plan");
    assert_eq!(offset(cx), 0.0);
    click(cx, "nav-search-clear");
    assert_eq!(offset(cx), -60.0);
}

#[gpui::test]
fn an_option_change_scrolls_the_results_back_up(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = scrolled(cx, dir.path());
    cx.simulate_input("plan");
    wait(cx, 250);
    scroll(cx, 40.0);
    assert_eq!(offset(cx), -40.0);
    cx.update(|_, cx| {
        cx.global_mut::<Search>().options.status = "archived";
        crate::sessions::search::run::sync(cx);
        cx.refresh_windows();
    });
    cx.run_until_parked();
    assert_eq!(offset(cx), 0.0);
}

#[gpui::test]
fn a_press_that_clears_restores_once_the_pointer_lifts(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = scrolled(cx, dir.path());
    cx.simulate_input("plan");
    let list = cx.debug_bounds("main-list").unwrap();
    let at = point(list.center().x, list.bottom() - px(4.0));
    cx.simulate_event(MouseDownEvent {
        position: at,
        button: MouseButton::Left,
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    cx.run_until_parked();
    assert_eq!(
        cx.update(|_, cx| crate::sessions::search::query(cx).to_owned()),
        ""
    );
    assert_eq!(offset(cx), 0.0, "held while the pointer is down");
    cx.simulate_event(MouseUpEvent {
        position: at,
        button: MouseButton::Left,
        modifiers: Modifiers::none(),
        click_count: 1,
    });
    cx.run_until_parked();
    assert_eq!(offset(cx), -60.0);
}
