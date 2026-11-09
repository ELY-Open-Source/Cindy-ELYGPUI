//! The resume dialog from the database load through the window, with real clicks.

use gpui::{
    Context, IntoElement, Modifiers, ParentElement as _, Render, Styled as _, TestAppContext,
    VisualTestContext, Window, WindowAppearance, div, px, size,
};
use rusqlite::Connection;

use super::{Interrupted, Task, meta};
use crate::{
    i18n::Translator, os_appearance::Initial, sessions::interrupted::tests as data,
    theme::Appearance, theme::Mode, toast::Toasts,
};

struct Host;

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(super::view(window, cx))
    }
}

/// Three tasks a turn left open, `a` oldest; `b` and `c` in the same folder.
fn tasks() -> Connection {
    let db = data::db();
    for (id, started) in [("a", 1_000), ("b", 2_000), ("c", 3_000)] {
        data::task(&db, id, "desktop", started);
        db.execute(
            "UPDATE sessions SET title = ?2, working_dir = ?3 WHERE id = ?1",
            (id, format!("Task {id}"), format!("/code/{id}")),
        )
        .unwrap();
    }
    db
}

fn open(cx: &mut TestAppContext, db: Connection) -> &mut VisualTestContext {
    let owner = tempfile::tempdir().unwrap().keep();
    cx.update(|cx| {
        ely_gpui_component::init_for_tests(cx);
        let initial = Initial {
            os: WindowAppearance::Light,
            gpui_agrees: true,
        };
        cx.set_global(crate::motion::ReducedMotion::default());
        cx.set_global(crate::units::Zoom(1.0));
        cx.set_global(crate::preferences::Preferences::load(&owner).unwrap());
        cx.set_global(Appearance::new(Mode::System, None, initial, None).unwrap());
        cx.set_global(Translator::new("en").unwrap());
        cx.set_global(Toasts::default());
        crate::localdb::install(None, cx);
        crate::sessions::install(owner.clone(), Some(owner), cx);
        crate::router::install(cx);
        crate::localdb::serve(db, cx);
        crate::sessions::interrupted::load(cx);
    });
    let (_, vcx) = cx.add_window_view(|_, _| Host);
    vcx.simulate_resize(size(px(1200.0), px(800.0)));
    vcx.run_until_parked();
    vcx
}

fn click(cx: &mut VisualTestContext, selector: &str) {
    let selector: &'static str = Box::leak(selector.to_owned().into_boxed_str());
    let at = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} not drawn"))
        .center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
}

fn shown(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.debug_bounds("confirm-dialog-panel").is_some()
}

fn toasts(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|window, cx| {
        let window = window.window_handle().window_id();
        let items = cx.global::<Toasts>().items(window);
        items.iter().map(|t| t.message.to_string()).collect()
    })
}

/// Lets the dialog's fade-out finish.
fn settle(cx: &mut VisualTestContext) {
    for _ in 0..2 {
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(200));
        cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.run_until_parked();
    }
}

#[gpui::test]
fn opens_with_the_load_newest_first(cx: &mut TestAppContext) {
    let cx = open(cx, tasks());
    assert!(shown(cx));
    let rows = ["a", "b", "c"]
        .map(|id| cx.debug_bounds(Box::leak(format!("interrupted-row-{id}").into_boxed_str())));
    let [a, b, c] = rows.map(|r| r.unwrap().origin.y);
    assert!(c < b && b < a, "newest first");
}

#[gpui::test]
fn continue_toasts_the_picked_tasks_still_interrupted(cx: &mut TestAppContext) {
    let cx = open(cx, tasks());
    click(cx, "interrupted-row-b");
    // `c` resumed elsewhere before the press.
    cx.update(|_, cx| {
        let crate::localdb::State::Ready(db) = &cx.global::<crate::localdb::LocalDb>().state else {
            panic!("no database");
        };
        let ended = db.call(|db| {
            db.execute(
                "UPDATE sessions SET last_turn_ended_at = 9000 WHERE id = 'c'",
                (),
            )
        });
        drop(ended);
    });
    cx.run_until_parked();
    click(cx, "confirm-dialog-confirm");
    assert_eq!(
        toasts(cx),
        ["1 session couldn't resume. Open it to try again."]
    );
    settle(cx);
    assert!(!shown(cx), "Continue closes it");
}

#[gpui::test]
fn select_all_clears_then_fills_and_an_empty_pick_cannot_continue(cx: &mut TestAppContext) {
    let cx = open(cx, tasks());
    click(cx, "interrupted-select-all");
    click(cx, "confirm-dialog-confirm");
    assert!(shown(cx) && toasts(cx).is_empty(), "nothing picked");
    click(cx, "interrupted-row-a");
    click(cx, "interrupted-select-all");
    click(cx, "confirm-dialog-confirm");
    assert_eq!(
        toasts(cx),
        ["3 sessions couldn't resume. Open them to try again."],
        "a mixed box selects all"
    );
}

#[gpui::test]
fn the_checkbox_toggles_once(cx: &mut TestAppContext) {
    let cx = open(cx, tasks());
    click(cx, "interrupted-box-a");
    click(cx, "confirm-dialog-confirm");
    assert_eq!(
        toasts(cx),
        ["2 sessions couldn't resume. Open them to try again."]
    );
}

#[gpui::test]
fn later_marks_the_owner_handled(cx: &mut TestAppContext) {
    let cx = open(cx, tasks());
    click(cx, "confirm-dialog-cancel");
    settle(cx);
    assert!(!shown(cx));
    assert!(toasts(cx).is_empty(), "Later sends nothing");
    assert!(!shown(cx), "the handled owner stays closed");
    cx.update(|_, cx| {
        let store = cx.global_mut::<Interrupted>();
        store.owner = Some(std::path::PathBuf::from("/elsewhere"));
    });
    assert!(shown(cx), "another owner's load opens it");
    click(cx, "confirm-dialog-cancel");
    settle(cx);
    let owner = cx.update(|_, cx| cx.global::<crate::sessions::Sessions>().owner_dir.clone());
    cx.update(|_, cx| cx.global_mut::<Interrupted>().owner = owner);
    assert!(!shown(cx), "back on the first owner, it stays handled");
}

#[gpui::test]
fn no_interrupted_task_draws_nothing(cx: &mut TestAppContext) {
    let cx = open(cx, data::db());
    assert!(!shown(cx));
}

#[gpui::test]
fn meta_names_the_source_activity_and_time(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let t = Translator::new("en").unwrap();
        let session = crate::sessions::view_tests::task("x", Some("/code/x"), false, 0);
        let mut task = Task {
            session,
            interrupted_at: 1_000_000,
            activity: Some("Ran tests".into()),
            companion: None,
        };
        let now = 1_000_000 + 2 * 60_000;
        assert_eq!(
            meta(&task, Some("x"), now, &t),
            "x · Ran tests · interrupted 2m ago"
        );
        task.companion = Some("Buddy".into());
        task.activity = None;
        assert_eq!(
            meta(&task, Some("x"), 1_000_000 + 59_999, &t),
            "Buddy · interrupted just now"
        );
        let _ = cx;
    });
}

#[gpui::test]
fn space_on_the_select_all_box_toggles_it(cx: &mut TestAppContext) {
    let cx = open(cx, tasks());
    for key in ["tab", "space"] {
        cx.simulate_keystrokes(key);
        cx.simulate_event(gpui::KeyUpEvent {
            keystroke: gpui::Keystroke::parse(key).unwrap(),
        });
    }
    cx.run_until_parked();
    click(cx, "confirm-dialog-confirm");
    assert!(
        shown(cx) && toasts(cx).is_empty(),
        "Tab wraps to Select all"
    );
}

fn press(cx: &mut VisualTestContext, key: &'static str) {
    cx.simulate_keystrokes(key);
    cx.simulate_event(gpui::KeyUpEvent {
        keystroke: gpui::Keystroke::parse(key).unwrap(),
    });
    cx.run_until_parked();
}

#[gpui::test]
fn a_click_on_the_row_text_hands_space_to_its_box(cx: &mut TestAppContext) {
    let cx = open(cx, tasks());
    click(cx, "interrupted-row-a");
    press(cx, "space");
    click(cx, "interrupted-select-all");
    press(cx, "space");
    click(cx, "confirm-dialog-confirm");
    assert_eq!(
        toasts(cx),
        ["3 sessions couldn't resume. Open them to try again."],
        "a off then on; all off, then on"
    );
}

#[gpui::test]
fn enter_does_not_click_a_box(cx: &mut TestAppContext) {
    let cx = open(cx, tasks());
    click(cx, "interrupted-row-a");
    press(cx, "enter");
    click(cx, "confirm-dialog-confirm");
    assert_eq!(
        toasts(cx),
        ["2 sessions couldn't resume. Open them to try again."]
    );
}

#[gpui::test]
fn the_hovered_row_stays_lit_through_keys(cx: &mut TestAppContext) {
    let cx = open(cx, tasks());
    let at = cx.debug_bounds("interrupted-row-b").unwrap().center();
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("interrupted-row-b-hover").is_some());
    press(cx, "tab");
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("interrupted-row-b-hover").is_some(),
        "CSS :hover ignores keys"
    );
    assert!(
        cx.debug_bounds("interrupted-select-all-hover").is_none(),
        "Select all has none"
    );
}
