use gpui::{
    Context, FocusHandle, IntoElement, KeyUpEvent, Keystroke, Modifiers, MouseButton,
    ParentElement as _, Pixels, Point, Render, Styled as _, TestAppContext, VisualTestContext,
    Window, div, point, px, size,
};

use super::{label, tiles};
use crate::{
    router,
    sessions::{
        Sessions, settings,
        view_mode::{self, Pinned},
        view_tests::{install, task},
    },
};

#[test]
fn the_label_takes_a_latin_word_or_four_units() {
    assert_eq!(label("  Fix login flow "), "Fix");
    assert_eq!(label("Refactoring the app"), "Refacto");
    assert_eq!(label("v1.2-beta release"), "v1.2-be");
    assert_eq!(label("中文标题测试"), "中文标题");
    assert_eq!(label("a 中文"), "a 中文", "one letter is no word");
    assert_eq!(
        label("-ab cd"),
        "-ab ",
        "a word starts with a letter or digit"
    );
    assert_eq!(label("😀😀😀"), "😀😀", "four UTF-16 units");
}

#[test]
fn tiles_leave_out_archived_and_workers_and_follow_the_order() {
    let mut all: Vec<_> = ["a", "b", "c", "d", "e", "f"]
        .iter()
        .map(|id| task(id, None, false, 1))
        .collect();
    for s in &mut all[..5] {
        s.pinned_at = Some(1);
    }
    all[3].status = "archived".into();
    all[4].orca_worker = true;
    let ids =
        |v: Vec<crate::sessions::group::Session>| v.into_iter().map(|s| s.id).collect::<Vec<_>>();
    assert_eq!(ids(tiles(&all, &[])), ["a", "b", "c"]);
    let order = ["c".to_owned(), "a".to_owned()];
    assert_eq!(ids(tiles(&all, &order)), ["c", "a", "b"], "unranked last");
}

struct Rail(FocusHandle);

impl Render for Rail {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rail = div()
            .w(px(78.0))
            .h_full()
            .flex()
            .flex_col()
            .child(crate::sessions::rail::upper(1.0, true, window, cx));
        ely_gpui_component::primitives::FocusScope::new(&self.0)
            .size_full()
            .flex()
            .child(rail)
            .child(div().flex_1().h_full())
    }
}

/// The rail with a0, a1 and a2 pinned, newest first; no order saved yet.
fn rail(cx: &mut TestAppContext) -> (tempfile::TempDir, &mut VisualTestContext) {
    let dir = tempfile::tempdir().unwrap();
    install(cx, dir.path());
    let owner = dir.path().join("owner");
    let (view, cx) = cx.add_window_view(|_, cx| Rail(cx.focus_handle()));
    cx.update(|window, cx| {
        window.focus(&view.read(cx).0.clone(), cx);
        window.activate_window();
        let store = cx.global_mut::<Sessions>();
        store.owner_dir = Some(owner);
        let loaded = store.loaded.as_mut().unwrap();
        for (i, s) in loaded.sessions.iter_mut().take(3).enumerate() {
            s.pinned_at = Some(3 - i as i64);
            s.preview = Some(format!(" latest of {} ", s.id));
            s.summary = Some(format!("summary of {}", s.id));
        }
    });
    cx.simulate_resize(size(px(800.0), px(600.0)));
    cx.run_until_parked();
    (dir, cx)
}

fn bounds(cx: &mut VisualTestContext, id: &str) -> Option<gpui::Bounds<Pixels>> {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.debug_bounds(Box::leak(format!("rail-pin-{id}").into_boxed_str()))
}

fn center(cx: &mut VisualTestContext, id: &str) -> Point<Pixels> {
    bounds(cx, id)
        .unwrap_or_else(|| panic!("{id} drawn"))
        .center()
}

fn path(cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| router::path(cx).to_owned())
}

#[gpui::test]
fn the_tiles_stand_in_a_column_then_the_divider(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    let (a0, a1, a2) = (center(cx, "a0"), center(cx, "a1"), center(cx, "a2"));
    assert!(a0.y < a1.y && a1.y < a2.y);
    assert!(bounds(cx, "a3").is_none(), "only pinned tasks");
    let divider = cx.debug_bounds("rail-pin-divider").unwrap();
    assert!(divider.top() > bounds(cx, "a2").unwrap().bottom());
    cx.update(|_, cx| {
        let loaded = cx.global_mut::<Sessions>().loaded.as_mut().unwrap();
        loaded.sessions.iter_mut().for_each(|s| s.pinned_at = None);
    });
    assert!(bounds(cx, "a0").is_none());
    assert!(
        cx.debug_bounds("rail-pin-divider").is_none(),
        "no pins, no divider"
    );
}

#[gpui::test]
fn a_click_opens_the_task_unless_it_is_shown(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    let at = center(cx, "a1");
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(path(cx), "/cc-agent/a1");
    cx.update(|_, cx| router::navigate("/cc-agent/new", cx));
    cx.update(|_, cx| router::navigate("/cc-agent/a1", cx));
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(path(cx), "/cc-agent/a1");
}

#[gpui::test]
fn enter_on_a_focused_tile_opens_it(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    // New, Search, then the first pin.
    cx.simulate_keystrokes("tab tab tab");
    cx.simulate_keystrokes("enter");
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").unwrap(),
    });
    cx.run_until_parked();
    assert_eq!(path(cx), "/cc-agent/a0");
}

fn hover(cx: &mut VisualTestContext, at: Point<Pixels>) {
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

#[gpui::test]
fn hover_shows_the_preview_at_the_tile_right(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    let tile = bounds(cx, "a1").unwrap();
    hover(cx, tile.center());
    let card = cx.debug_bounds("rail-pin-preview").expect("the preview");
    assert_eq!(card.left(), tile.right() + px(10.0));
    assert_eq!(card.top(), tile.top() - px(4.0));
    assert_eq!(card.size.width, px(208.0));
    hover(cx, point(px(600.0), px(500.0)));
    assert!(
        cx.debug_bounds("rail-pin-preview").is_none(),
        "leaving hides it"
    );
}

#[gpui::test]
fn the_preview_body_follows_the_pinned_mode(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    let s = cx.update(|_, cx| cx.global::<Sessions>().loaded.as_ref().unwrap().sessions[0].clone());
    let body = |cx: &mut VisualTestContext| cx.update(|_, cx| super::preview_body(&s, cx));
    cx.update(|_, cx| view_mode::set_pinned(Pinned::Card, cx));
    assert_eq!(
        body(cx).as_deref(),
        Some("summary of a0"),
        "a pinned card's summary"
    );
    cx.update(|_, cx| view_mode::set_pinned(Pinned::Text, cx));
    assert_eq!(
        body(cx).as_deref(),
        Some("latest of a0"),
        "else the latest message"
    );
}

#[gpui::test]
fn an_unpinned_tile_takes_its_preview_away(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    let at = center(cx, "a2");
    hover(cx, at);
    assert!(cx.debug_bounds("rail-pin-preview").is_some());
    cx.update(|_, cx| {
        let loaded = cx.global_mut::<Sessions>().loaded.as_mut().unwrap();
        loaded.sessions[2].pinned_at = None;
    });
    assert!(bounds(cx, "a2").is_none());
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(cx.debug_bounds("rail-pin-preview").is_none());
}

#[gpui::test]
fn a_drag_reorders_and_saves(cx: &mut TestAppContext) {
    let (dir, cx) = rail(cx);
    cx.update(|_, cx| router::navigate("/cc-agent/d0", cx));
    let (start, end) = (center(cx, "a0"), center(cx, "a2"));
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(
        start + point(px(0.0), px(5.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();

    let at = center(cx, "a0");
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let want = ["a1", "a2", "a0"];
    let order = cx.update(|_, cx| {
        let loaded = cx.global::<Sessions>().loaded.as_ref().unwrap();
        loaded.settings.pinned_order.clone()
    });
    assert_eq!(order, want);
    assert_eq!(
        settings::read(&dir.path().join("owner"), "linux").pinned_order,
        want
    );
    assert_eq!(path(cx), "/cc-agent/d0", "a drag is no click");
}

#[gpui::test]
fn a_displaced_tile_slides_from_its_own_slot(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    let top = |cx: &mut VisualTestContext, id: &str| bounds(cx, id).unwrap().top();
    let (a0, a1) = (top(cx, "a0"), top(cx, "a1"));
    let start = center(cx, "a0");
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(
        start + point(px(0.0), px(5.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    let end = center(cx, "a1");
    cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let first = top(cx, "a1");
    assert!(
        (first - a1).abs() < px(1.0),
        "starts at its old slot: {first:?} vs {a1:?}"
    );
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(200));
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();
    assert_eq!(top(cx, "a1"), a0, "ends in the slot it took");
}
