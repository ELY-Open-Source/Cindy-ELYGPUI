use gpui::{
    InteractiveElement as _, IntoElement, KeyUpEvent, Keystroke, Modifiers, MouseButton,
    MouseDownEvent, MouseUpEvent, ParentElement as _, Styled as _, TestAppContext,
    VisualTestContext, Window, div,
};

use crate::{layout_root::screen_tests, router, sessions};

/// The content area with `s1` listed and the database served; `s1` opens.
fn open<'a>(cx: &'a mut TestAppContext, dir: &std::path::Path) -> &'a mut VisualTestContext {
    let cx = screen_tests::open(cx, dir, screen_tests::tree(0.6, 0.4), 1200.0);
    cx.update(|_, cx| {
        crate::localdb::serve(super::super::tests::db(), cx);
        sessions::load_for_tests(&[("s1", "active", "desktop")], cx);
    });
    cx.run_until_parked();
    cx
}

fn sql(cx: &mut VisualTestContext, sql: &'static str) {
    let done = cx.update(|_, cx| {
        let crate::localdb::State::Ready(db) = &cx.global::<crate::localdb::LocalDb>().state else {
            panic!("served")
        };
        db.call(move |conn| conn.execute(sql, []))
    });
    assert!(futures::executor::block_on(done).unwrap().is_ok());
}

fn read(cx: &mut VisualTestContext, sql: &'static str) -> Option<String> {
    let done = cx.update(|_, cx| {
        let crate::localdb::State::Ready(db) = &cx.global::<crate::localdb::LocalDb>().state else {
            panic!("served")
        };
        db.call(move |conn| conn.query_row(sql, [], |r| r.get::<_, Option<String>>(0)))
    });
    futures::executor::block_on(done).unwrap().unwrap()
}

fn go(cx: &mut VisualTestContext, to: &str) {
    cx.update(|_, cx| router::navigate(to, cx));
    cx.run_until_parked();
}

fn press(cx: &mut VisualTestContext, selector: &'static str, count: usize) {
    let at = cx.debug_bounds(selector).expect(selector).center();
    let (modifiers, button) = (Modifiers::none(), MouseButton::Left);
    cx.simulate_event(MouseDownEvent {
        position: at,
        modifiers,
        button,
        click_count: count,
        first_mouse: false,
    });
    cx.simulate_event(MouseUpEvent {
        position: at,
        modifiers,
        button,
        click_count: count,
    });
    cx.run_until_parked();
}

fn drawn(cx: &mut VisualTestContext, selector: &'static str) -> bool {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.debug_bounds(selector).is_some()
}

fn shown_title(cx: &mut VisualTestContext) -> Option<String> {
    cx.update(|_, cx| super::task(cx).map(|s| s.title.clone()))
}

#[gpui::test]
fn the_header_shows_the_routed_task(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    assert!(drawn(cx, "session-header-title"));
    // The listed row wins over the read one.
    assert_eq!(shown_title(cx).as_deref(), Some("s1"));
    assert!(!drawn(cx, "session-header-pin"));
    sql(cx, "UPDATE sessions SET pinned_at = 5 WHERE id = 'old'");
    go(cx, "/cc-agent/old");
    // An archived task comes from the content's read, pin and all.
    assert_eq!(shown_title(cx).as_deref(), Some("task old"));
    assert!(drawn(cx, "session-header-pin"));
    // A listed archived row is not the view's: the read row stays.
    let all = [("old", "archived", "desktop")];
    cx.update(|_, cx| sessions::load_buckets_for_tests(&[("s1", "active", "desktop")], &all, cx));
    assert_eq!(shown_title(cx).as_deref(), Some("task old"));
    go(cx, "/cc-agent/nope");
    assert!(!drawn(cx, "session-header"), "no task, no header");
}

#[gpui::test]
fn a_double_click_renames_and_enter_saves(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    go(cx, "/cc-agent/old");
    press(cx, "session-header-title", 1);
    assert!(!drawn(cx, "task-rename"), "one click does not rename");
    press(cx, "session-header-title", 2);
    assert!(drawn(cx, "task-rename"));
    assert!(
        !drawn(cx, "session-header-more"),
        "More hides while editing"
    );
    // A list read does not end the header's edit of an unlisted task.
    cx.update(|_, cx| sessions::load_for_tests(&[("s1", "active", "desktop")], cx));
    assert!(drawn(cx, "task-rename"));
    cx.simulate_input("Renamed");
    cx.simulate_keystrokes("enter");
    assert!(!drawn(cx, "task-rename"));
    let title = read(cx, "SELECT title FROM sessions WHERE id = 'old'");
    assert_eq!(title.as_deref(), Some("Renamed"));
}

#[gpui::test]
fn the_stored_title_typed_back_saves_nothing(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    sql(
        cx,
        "UPDATE sessions SET title = '[Schedule] Weekly' WHERE id = 'old'",
    );
    go(cx, "/cc-agent/old");
    press(cx, "session-header-title", 2);
    cx.simulate_input("[Schedule] Weekly");
    cx.simulate_keystrokes("enter");
    assert!(!drawn(cx, "task-rename"));
    let title = read(cx, "SELECT title FROM sessions WHERE id = 'old'");
    assert_eq!(title.as_deref(), Some("[Schedule] Weekly"));
    // Nothing was written: with the database gone, no rename fails.
    cx.update(|_, cx| crate::localdb::install(None, cx));
    press(cx, "session-header-title", 2);
    cx.simulate_input("[Schedule] Weekly");
    cx.simulate_keystrokes("enter");
    assert!(toasts(cx).is_empty());
}

fn toasts(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|window, cx| {
        let id = window.window_handle().window_id();
        let toasts = cx.global::<crate::toast::Toasts>().items(id);
        toasts.iter().map(|item| item.message.to_string()).collect()
    })
}

#[gpui::test]
fn an_untitled_task_shows_the_header_fallback(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    sql(cx, "UPDATE sessions SET title = '   ' WHERE id = 'old'");
    go(cx, "/cc-agent/old");
    let shown = cx.update(|_, cx| {
        let t = cx.global::<crate::i18n::Translator>();
        super::task(cx).map(|s| crate::sessions::title::header(s, t))
    });
    assert_eq!(shown.as_deref(), Some("Untitled session"));
    // The fallback typed back is no change.
    press(cx, "session-header-title", 2);
    cx.simulate_keystrokes("enter");
    let title = read(cx, "SELECT title FROM sessions WHERE id = 'old'");
    assert_eq!(title.as_deref(), Some("   "));
    // The field opens on the fallback, so an addition keeps it.
    press(cx, "session-header-title", 2);
    cx.simulate_keystrokes("end");
    cx.simulate_input("!");
    cx.simulate_keystrokes("enter");
    let title = read(cx, "SELECT title FROM sessions WHERE id = 'old'");
    assert_eq!(title.as_deref(), Some("Untitled session!"));
}

#[gpui::test]
fn more_pins_without_moving_the_pinned_order(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    press(cx, "session-header-more", 1);
    press(cx, "menu-row-Pin", 1);
    let pinned = read(
        cx,
        "SELECT CAST(pinned_at AS TEXT) FROM sessions WHERE id = 's1'",
    );
    assert!(pinned.is_some(), "pinnedAt was written");
    let order = cx.update(|_, cx| sessions::pinned_order_for_tests(cx));
    assert!(order.is_empty(), "the header leaves pinnedOrder alone");
    assert!(toasts(cx).is_empty(), "no pinned order save ran or failed");
    press(cx, "session-header-more", 1);
    press(cx, "menu-row-Rename", 1);
    assert!(drawn(cx, "task-rename"), "Rename opens the header's field");
}

fn down(cx: &mut VisualTestContext, at: gpui::Point<gpui::Pixels>) {
    cx.simulate_event(MouseDownEvent {
        position: at,
        modifiers: Modifiers::none(),
        button: MouseButton::Left,
        click_count: 1,
        first_mouse: false,
    });
}

/// Down on the title, then a move `dx` to the right with the button held.
fn drag(cx: &mut VisualTestContext, dx: f32) {
    let at = cx
        .debug_bounds("session-header-title")
        .expect("title")
        .center();
    down(cx, at);
    let to = gpui::point(at.x + gpui::px(dx), at.y);
    cx.simulate_mouse_move(to, Some(MouseButton::Left), Modifiers::none());
    cx.run_until_parked();
}

/// The test window panics on a window move: neither the title nor the header's drag moved it.
#[gpui::test]
fn a_press_inside_the_dead_zone_does_not_move_the_window(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    drag(cx, 3.0);
}

#[gpui::test]
#[should_panic(expected = "not implemented")]
fn a_press_past_the_dead_zone_moves_the_window(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    drag(cx, 5.0);
}

#[gpui::test]
#[should_panic(expected = "not implemented")]
fn a_drag_that_leaves_the_title_still_moves_the_window(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    let at = cx.debug_bounds("session-header-title").unwrap().center();
    down(cx, at);
    let outside = gpui::point(at.x, at.y + gpui::px(120.0));
    cx.simulate_mouse_move(outside, Some(MouseButton::Left), Modifiers::none());
    cx.run_until_parked();
}

#[gpui::test]
fn a_release_outside_the_title_ends_the_press(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    let title = cx.debug_bounds("session-header-title").unwrap();
    let start = gpui::point(title.left() + gpui::px(1.0), title.center().y);
    let hover = gpui::point(title.right() - gpui::px(1.0), title.center().y);
    assert!(hover.x - start.x > gpui::px(4.0));
    down(cx, start);
    cx.simulate_event(MouseUpEvent {
        position: gpui::point(start.x, start.y + gpui::px(120.0)),
        modifiers: Modifiers::none(),
        button: MouseButton::Left,
        click_count: 1,
    });
    // A later press that began elsewhere drags over the title.
    cx.simulate_mouse_move(hover, Some(MouseButton::Left), Modifiers::none());
    cx.run_until_parked();
}

#[gpui::test]
fn a_move_with_no_button_held_ends_a_press_whose_release_was_lost(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    let at = cx.debug_bounds("session-header-title").unwrap().center();
    down(cx, at);
    let far = gpui::point(at.x + gpui::px(40.0), at.y);
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.simulate_mouse_move(far, Some(MouseButton::Left), Modifiers::none());
    cx.run_until_parked();
}

/// The header between two Tab stops.
struct Between(gpui::FocusHandle, gpui::FocusHandle);

impl gpui::Render for Between {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .size_full()
            .child(div().id("before").track_focus(&self.0).child("before"))
            .children(super::element(window, cx))
            .child(div().id("after").track_focus(&self.1).child("after"))
    }
}

#[gpui::test]
fn more_is_a_tab_stop_that_enter_opens(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    let (before, after) = cx.update(|window, cx| {
        let stops = (
            cx.focus_handle().tab_stop(true),
            cx.focus_handle().tab_stop(true),
        );
        let (before, after) = stops.clone();
        window.replace_root(cx, |_, _| Between(before, after));
        stops
    });
    assert!(drawn(cx, "session-header-more"));
    let on_more = cx.update(|window, cx| {
        window.focus(&before, cx);
        window.focus_next(cx);
        !before.is_focused(window) && !after.is_focused(window)
    });
    assert!(on_more, "Tab lands on More, between the two stops");
    cx.simulate_keystrokes("enter");
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").unwrap(),
    });
    assert!(drawn(cx, "menu-row-Pin"), "Enter opens the task menu");
}

#[gpui::test]
fn a_press_focuses_more(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    press(cx, "session-header-more", 1);
    cx.simulate_keystrokes("escape");
    assert!(!drawn(cx, "menu-row-Pin"));
    // The menu gave focus back to its trigger, so Enter opens it again.
    cx.simulate_keystrokes("enter");
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").unwrap(),
    });
    assert!(drawn(cx, "menu-row-Pin"));
}

#[gpui::test]
fn a_route_change_keeps_the_field_and_commits_to_the_task_shown(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    go(cx, "/cc-agent/old");
    press(cx, "session-header-title", 2);
    cx.simulate_input("Unsubmitted edit");
    go(cx, "/cc-agent/s1");
    assert!(drawn(cx, "task-rename"), "the field outlives the switch");
    assert!(!drawn(cx, "session-header-more"));
    cx.simulate_keystrokes("enter");
    assert!(!drawn(cx, "task-rename"));
    let old = read(cx, "SELECT title FROM sessions WHERE id = 'old'");
    assert_eq!(old.as_deref(), Some("task old"), "the opener is untouched");
    let s1 = read(cx, "SELECT title FROM sessions WHERE id = 's1'");
    assert_eq!(s1.as_deref(), Some("Unsubmitted edit"));
}

#[gpui::test]
fn the_shown_tasks_own_title_typed_after_a_switch_saves_nothing(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    go(cx, "/cc-agent/old");
    press(cx, "session-header-title", 2);
    cx.simulate_input("s1");
    go(cx, "/cc-agent/s1");
    cx.update(|_, cx| crate::localdb::install(None, cx));
    cx.simulate_keystrokes("enter");
    assert!(!drawn(cx, "task-rename"));
    assert!(toasts(cx).is_empty(), "no write ran");
}

#[gpui::test]
fn a_header_that_goes_takes_its_edit_with_it(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    press(cx, "session-header-title", 2);
    cx.simulate_input("Lost");
    go(cx, "/cc-agent/nope");
    assert!(!drawn(cx, "session-header"));
    go(cx, "/cc-agent/s1");
    assert!(!drawn(cx, "task-rename"), "the edit went with the header");
    let s1 = read(cx, "SELECT title FROM sessions WHERE id = 's1'");
    assert_eq!(s1.as_deref(), Some("task s1"));
}

/// `old` listed archived under a stale title while the database holds a fresh one.
fn rename_past_a_stale_archived_row(cx: &mut TestAppContext, through_menu: bool) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    sql(cx, "UPDATE sessions SET title = 'Fresh' WHERE id = 'old'");
    let all = [("old", "archived", "desktop")];
    cx.update(|_, cx| sessions::load_buckets_for_tests(&[("s1", "active", "desktop")], &all, cx));
    go(cx, "/cc-agent/old");
    match through_menu {
        true => {
            press(cx, "session-header-more", 1);
            press(cx, "menu-row-Rename", 1);
        }
        false => press(cx, "session-header-title", 2),
    }
    cx.simulate_keystrokes("end");
    cx.simulate_input("!");
    cx.simulate_keystrokes("enter");
    let title = read(cx, "SELECT title FROM sessions WHERE id = 'old'");
    assert_eq!(
        title.as_deref(),
        Some("Fresh!"),
        "the field opened on the title shown"
    );
}

#[gpui::test]
fn a_double_click_opens_on_the_title_the_header_shows(cx: &mut TestAppContext) {
    rename_past_a_stale_archived_row(cx, false);
}

#[gpui::test]
fn mores_rename_opens_on_the_title_the_header_shows(cx: &mut TestAppContext) {
    rename_past_a_stale_archived_row(cx, true);
}
