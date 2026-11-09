//! The project header's keys, More, menu and rename, driven by real events.

use gpui::{
    Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, TestAppContext, VisualTestContext,
};

use super::deep_link;
use crate::sessions::{
    Sessions,
    view_tests::{click, open},
};

const HEADER: &str = "project:local:/work/app-header";
const MORE: &str = "project-more-local:/work/app";

fn press(cx: &mut VisualTestContext, selector: &'static str, button: MouseButton, count: usize) {
    let at = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} not drawn"))
        .center();
    let modifiers = Modifiers::none();
    for click_count in 1..=count {
        cx.simulate_event(MouseDownEvent {
            position: at,
            modifiers,
            button,
            click_count,
            first_mouse: false,
        });
        cx.simulate_event(MouseUpEvent {
            position: at,
            modifiers,
            button,
            click_count,
        });
    }
    cx.run_until_parked();
}

fn drawn(cx: &mut VisualTestContext, selector: &'static str) -> bool {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.debug_bounds(selector).is_some()
}

fn toasts(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|window, cx| {
        cx.global::<crate::toast::Toasts>()
            .items(window.window_handle().window_id())
            .iter()
            .map(|item| item.message.to_string())
            .collect()
    })
}

fn folded(cx: &mut VisualTestContext) -> bool {
    cx.update(|_, cx| {
        cx.global::<Sessions>()
            .collapsed
            .contains("project:local:/work/app")
    })
}

fn aliases(cx: &mut VisualTestContext) -> Vec<(String, String)> {
    cx.update(|_, cx| {
        cx.global::<Sessions>()
            .loaded
            .as_ref()
            .unwrap()
            .aliases
            .clone()
    })
}

#[test]
fn the_link_escapes_what_encode_uri_component_keeps() {
    assert_eq!(
        deep_link("/w/a b/(x)!*'~"),
        "cindy://project/%2Fw%2Fa%20b%2F%28x%29%21%2A%27~"
    );
    assert_eq!(
        deep_link("/w/项目"),
        "cindy://project/%2Fw%2F%E9%A1%B9%E7%9B%AE"
    );
}

#[gpui::test]
fn a_right_click_opens_the_menu_and_copies_the_link(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    press(cx, HEADER, MouseButton::Right, 1);
    assert!(!folded(cx), "a right click does not fold");
    for row in ["menu-row-Rename project", "menu-row-Pin project"] {
        assert!(drawn(cx, row), "{row}");
    }
    click(cx, "menu-row-Copy Project Link");
    let copied = cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(copied.as_deref(), Some("cindy://project/%2Fwork%2Fapp"));
    assert_eq!(toasts(cx), ["Link copied"]);
}

#[gpui::test]
fn more_shows_on_hover_and_opens_the_menu_under_it(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    let at = cx.debug_bounds(HEADER).unwrap().center();
    cx.simulate_mouse_move(at, None, Modifiers::none());
    click(cx, MORE);
    assert!(!folded(cx), "More does not fold the project");
    let more = cx.debug_bounds(MORE).unwrap();
    let row = cx
        .debug_bounds("menu-row-Rename project")
        .expect("menu open");
    assert!(
        row.origin.y > more.origin.y + more.size.height,
        "under More"
    );
}

#[gpui::test]
fn a_double_click_renames_and_escape_drops_it(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    press(cx, HEADER, MouseButton::Left, 1);
    assert!(folded(cx) && !drawn(cx, "project-rename"), "a click folds");
    press(cx, HEADER, MouseButton::Left, 2);
    assert!(folded(cx), "two clicks fold twice");
    assert!(drawn(cx, "project-rename"));
    cx.simulate_input("Other");
    cx.simulate_keystrokes("enter");
    assert!(!drawn(cx, "project-rename"));
    assert!(aliases(cx).is_empty(), "rolled back: no database");
    assert_eq!(toasts(cx), ["Project rename failed"]);
    press(cx, HEADER, MouseButton::Left, 2);
    cx.simulate_input("Again");
    cx.simulate_keystrokes("escape");
    assert!(!drawn(cx, "project-rename"));
    assert_eq!(toasts(cx).len(), 1, "Escape writes nothing");
}

#[gpui::test]
fn enter_and_space_fold_and_the_menu_key_opens_the_menu(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    press(cx, HEADER, MouseButton::Left, 1);
    cx.simulate_keystrokes("enter");
    assert!(!folded(cx), "Enter unfolds");
    cx.simulate_keystrokes("space");
    assert!(folded(cx), "Space folds");
    cx.simulate_keystrokes("shift-f10");
    assert!(drawn(cx, "menu-row-Rename project"));
}

#[gpui::test]
fn pin_and_unpin_save_the_pinned_order(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let owner = dir.path().join("owner");
    let cx = open(cx, dir.path());
    cx.update(|_, cx| cx.global_mut::<Sessions>().owner_dir = Some(owner.clone()));
    press(cx, HEADER, MouseButton::Right, 1);
    click(cx, "menu-row-Pin project");
    let saved = || std::fs::read_to_string(owner.join("sidebar-settings.json")).unwrap();
    assert!(
        saved().contains("\"project:local:/work/app\""),
        "{}",
        saved()
    );
    press(cx, "project:local:/work/app-header", MouseButton::Right, 1);
    click(cx, "menu-row-Unpin project");
    assert!(!saved().contains("project:local"), "{}", saved());
    assert!(toasts(cx).is_empty());
}

#[gpui::test]
fn a_rename_writes_the_alias_cindy_reads(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    let file = dir.path().join("cindy-local-v1.db");
    let db = crate::sessions::tests::db();
    db.execute_batch(&format!("VACUUM INTO '{}';", file.display()))
        .unwrap();
    let db = rusqlite::Connection::open(&file).unwrap();
    db.execute_batch(
        "CREATE TABLE project_aliases (project_key text PRIMARY KEY, alias text NOT NULL,
           updated_at integer NOT NULL);
         INSERT INTO sessions (id, created_at, updated_at, user_send_at, working_dir)
           VALUES ('t', 1, 1, 1, '/work/app');",
    )
    .unwrap();
    cx.update(|_, cx| crate::localdb::serve(db, cx));
    cx.run_until_parked();
    press(cx, HEADER, MouseButton::Left, 2);
    cx.simulate_input("  Renamed ");
    cx.simulate_keystrokes("enter");
    assert_eq!(aliases(cx), [("local:/work/app".into(), "Renamed".into())]);
    assert!(toasts(cx).is_empty(), "{:?}", toasts(cx));
    let reader = rusqlite::Connection::open(&file).unwrap();
    let stored: (String, String) = reader
        .query_row("SELECT project_key, alias FROM project_aliases", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(stored, ("local:/work/app".into(), "Renamed".into()));
}

#[gpui::test]
fn the_menu_key_on_more_opens_the_menu_under_the_header(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    press(cx, HEADER, MouseButton::Left, 1);
    cx.simulate_keystrokes("tab");
    assert!(drawn(cx, MORE));
    cx.simulate_keystrokes("enter");
    assert!(folded(cx), "Enter on More does not fold");
    cx.simulate_keystrokes("escape shift-f10");
    assert!(drawn(cx, "menu-row-Rename project"));
}

fn slot_width(cx: &mut VisualTestContext) -> f32 {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let slot = cx.debug_bounds("project-slot-local:/work/app").unwrap();
    f32::from(slot.size.width)
}

#[gpui::test]
fn a_key_press_keeps_the_hovered_header_lit(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    assert_eq!(slot_width(cx), 0.0, "More waits for a hover");
    press(cx, HEADER, MouseButton::Left, 1);
    cx.simulate_keystrokes("enter");
    assert!(!folded(cx));
    assert_eq!(
        slot_width(cx),
        20.0,
        "the pointer still rests on the header"
    );
    let away = cx.debug_bounds(HEADER).unwrap().bottom_left()
        + gpui::point(gpui::px(0.0), gpui::px(300.0));
    cx.simulate_mouse_move(away, None, Modifiers::none());
    assert_eq!(slot_width(cx), 0.0);
}

#[gpui::test]
fn a_key_press_keeps_the_hovered_more_lit(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    press(cx, HEADER, MouseButton::Left, 1);
    let more = cx.debug_bounds(MORE).unwrap().center();
    cx.simulate_mouse_move(more, None, Modifiers::none());
    assert!(drawn(cx, "project-more-hover-local:/work/app"));
    cx.simulate_keystrokes("enter");
    assert!(!folded(cx), "the header took the key");
    assert!(drawn(cx, "project-more-hover-local:/work/app"), "still lit");
}
