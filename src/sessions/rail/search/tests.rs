use std::time::Duration;

use ely_gpui_component::primitives::FocusScope;
use gpui::{
    Context, FocusHandle, InteractiveElement as _, IntoElement, KeyUpEvent, Keystroke, Modifiers,
    ParentElement as _, Render, Styled as _, TestAppContext, VisualTestContext, Window, div, point,
    px, size,
};
use rusqlite::Connection;

use super::Popover;
use crate::{
    localdb::{
        self,
        search::conversations::tests::{schema, task},
    },
    router,
    sessions::{
        search::{self, Which, run},
        view_tests::{click, install},
    },
};

/// The rail beside content that counts the presses it takes.
struct Rail(FocusHandle);

#[derive(Default)]
struct Under(usize);

impl gpui::Global for Under {}

impl Render for Rail {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rail = div()
            .w(px(78.0))
            .h_full()
            .flex()
            .flex_col()
            .child(crate::sessions::rail::upper(1.0, true, window, cx));
        let under = div()
            .id("under")
            .flex_1()
            .h_full()
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
                cx.default_global::<Under>().0 += 1
            });
        FocusScope::new(&self.0)
            .size_full()
            .flex()
            .child(rail)
            .child(under)
    }
}

fn tasks(db: &Connection) {
    task(db, "t", "Fix login flow", 10);
    task(db, "c", "Other work", 20);
}

fn rail(cx: &mut TestAppContext) -> (tempfile::TempDir, &mut VisualTestContext) {
    let dir = tempfile::tempdir().unwrap();
    install(cx, dir.path());
    let (view, cx) = cx.add_window_view(|_, cx| Rail(cx.focus_handle()));
    cx.update(|window, cx| {
        window.focus(&view.read(cx).0.clone(), cx);
        window.activate_window();
    });
    cx.simulate_resize(size(px(1200.0), px(900.0)));
    let db = Connection::open_in_memory().unwrap();
    schema(&db);
    tasks(&db);
    cx.update(|_, cx| localdb::serve(db, cx));
    cx.run_until_parked();
    (dir, cx)
}

fn wait(cx: &mut VisualTestContext, ms: u64) {
    cx.executor().advance_clock(Duration::from_millis(ms));
    cx.run_until_parked();
}

fn drawn(cx: &mut VisualTestContext, selector: &'static str) -> bool {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.debug_bounds(selector).is_some()
}

fn open(cx: &mut VisualTestContext) -> bool {
    cx.update(|_, cx| cx.global::<Popover>().open)
}

fn found(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|_, cx| {
        run::state(Which::Rail, cx).1.map_or_else(Vec::new, |r| {
            r.results.iter().map(|i| i.session.id.clone()).collect()
        })
    })
}

fn query(cx: &mut VisualTestContext, w: Which) -> String {
    cx.update(|_, cx| search::query(w, cx).to_owned())
}

fn focused_field(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, cx| {
        let field = search::field(Which::Rail, window, cx);
        field.read(cx).focus().is_focused(window)
    })
}

#[gpui::test]
fn the_tile_opens_an_empty_popover_onto_its_field(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    click(cx, "rail-search");
    assert!(open(cx) && drawn(cx, "rail-search-panel"));
    assert!(
        drawn(cx, "rail-search-empty"),
        "the empty line before a query"
    );
    assert!(focused_field(cx), "focus moves to the field");
    let tile = cx.debug_bounds("rail-search").unwrap();
    let panel = cx.debug_bounds("rail-search-panel").unwrap();
    assert_eq!(panel.left(), tile.right() + px(8.0), "at the tile's right");
    assert_eq!(panel.top(), tile.top());
    click(cx, "rail-search");
    assert!(!open(cx), "the trigger toggles it");
}

#[gpui::test]
fn the_popover_searches_on_its_own_and_resets_on_close(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    cx.simulate_input("login");
    wait(cx, 900);
    assert!(found(cx).is_empty(), "closed, it does not search");
    click(cx, "rail-search");
    cx.simulate_input("login");
    wait(cx, 900);
    assert_eq!(found(cx), ["t"]);
    assert_eq!(query(cx, Which::Inline), "", "the inline search is apart");
    cx.simulate_keystrokes("escape");
    assert!(!open(cx));
    assert_eq!(query(cx, Which::Rail), "", "a close resets the query");
    let back = cx.update(|_, cx| {
        let f = cx.global::<Popover>().focus.clone().unwrap();
        f.trigger
    });
    assert!(
        cx.update(|window, _| back.is_focused(window)),
        "Escape returns focus to the tile"
    );
}

#[gpui::test]
fn a_press_outside_closes_without_returning_focus(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    click(cx, "rail-search");
    cx.simulate_input("login");
    let inside = cx.debug_bounds("rail-search-panel").unwrap().center();
    cx.simulate_click(inside, Modifiers::none());
    cx.run_until_parked();
    assert!(open(cx), "a press on the panel keeps it");
    cx.simulate_click(point(px(1100.0), px(850.0)), Modifiers::none());
    cx.run_until_parked();
    assert!(!open(cx));
    assert_eq!(query(cx, Which::Rail), "");
}

#[gpui::test]
fn focus_leaving_the_panel_closes_it(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    click(cx, "rail-search");
    assert!(focused_field(cx));
    cx.update(|window, cx| {
        let elsewhere = cx.focus_handle();
        window.focus(&elsewhere, cx);
    });
    cx.run_until_parked();
    assert!(!open(cx));
}

#[gpui::test]
fn a_chosen_result_closes_the_popover_and_opens_its_task(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    click(cx, "rail-search");
    cx.simulate_input("login");
    wait(cx, 900);
    click(cx, "search-result-t");
    assert!(!open(cx));
    assert_eq!(
        cx.update(|_, cx| router::path(cx).to_owned()),
        "/cc-agent/t"
    );
}

#[gpui::test]
fn the_clear_button_empties_the_field(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    click(cx, "rail-search");
    cx.simulate_input("login");
    assert!(drawn(cx, "rail-search-clear"));
    click(cx, "rail-search-clear");
    assert_eq!(query(cx, Which::Rail), "");
    assert!(open(cx), "the popover stays");
}

#[gpui::test]
fn the_sort_is_shared_by_both_searches(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    click(cx, "rail-search");
    click(cx, "rail-search-filter-button");
    click(cx, "menu-row-Sort by");
    click(cx, "menu-row-Recently active");
    let sort = cx.update(|_, cx| cx.global::<search::Search>().sort);
    assert_eq!(sort, "activityDesc");
    assert!(open(cx), "a choice in its menu keeps the popover");
}

#[gpui::test]
fn the_inline_search_counts_the_popover_as_a_popper(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    click(cx, "rail-search");
    cx.update(|window, cx| {
        let field = search::field(Which::Inline, window, cx);
        field.update(cx, |input, cx| input.set_text("plan", cx));
    });
    cx.run_until_parked();
    assert_eq!(query(cx, Which::Inline), "plan");
    let inside = cx.debug_bounds("rail-search-panel").unwrap().center();
    cx.simulate_click(inside, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(query(cx, Which::Inline), "plan");
}

fn under(cx: &mut VisualTestContext) -> usize {
    cx.update(|_, cx| cx.default_global::<Under>().0)
}

#[gpui::test]
fn a_press_on_the_panel_reaches_nothing_beneath(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    click(cx, "rail-search");
    let panel = cx.debug_bounds("rail-search-panel").unwrap();
    let pad = point(panel.right() - px(4.0), panel.bottom() - px(4.0));
    cx.simulate_click(pad, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(under(cx), 0);
    cx.simulate_click(point(px(1100.0), px(850.0)), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(under(cx), 1, "outside, the content takes it");
}

fn key_click(cx: &mut VisualTestContext, key: &'static str) {
    cx.simulate_keystrokes(key);
    let keystroke = Keystroke::parse(key).unwrap();
    cx.simulate_event(KeyUpEvent { keystroke });
    cx.run_until_parked();
}

#[gpui::test]
fn clear_is_a_tab_stop_and_tab_wraps_inside(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    click(cx, "rail-search");
    cx.simulate_input("x");
    cx.simulate_keystrokes("tab");
    key_click(cx, "enter");
    assert_eq!(query(cx, Which::Rail), "", "Enter on Clear clears");
    assert!(open(cx));
    cx.update(|window, cx| {
        let field = search::field(Which::Rail, window, cx)
            .read(cx)
            .focus()
            .clone();
        window.focus(&field, cx);
    });
    cx.simulate_keystrokes("shift-tab");
    let filter = cx.update(|_, cx| cx.global::<Popover>().focus.clone().unwrap().filter);
    assert!(
        cx.update(|window, _| filter.is_focused(window)),
        "Shift-Tab wraps to the end"
    );
    cx.simulate_keystrokes("tab");
    assert!(focused_field(cx), "Tab wraps to the field");
    assert!(open(cx), "focus never left");
}

#[gpui::test]
fn the_rail_menu_is_a_popper_to_the_inline_search(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    click(cx, "rail-search");
    cx.update(|window, cx| {
        let field = search::field(Which::Inline, window, cx);
        field.update(cx, |input, cx| input.set_text("plan", cx));
    });
    click(cx, "rail-search-filter-button");
    click(cx, "menu-row-Sort by");
    click(cx, "menu-row-Recently active");
    assert_eq!(query(cx, Which::Inline), "plan");
    assert!(open(cx));
}
