//! The sidebar frame driven by mouse events, beside the content area as the main window lays it.

use std::{path::Path, time::Duration};

use gpui::{
    AppContext as _, Context, Entity, IntoElement, Modifiers, MouseButton, MouseDownEvent,
    MouseUpEvent, ParentElement, Render, Styled, TestAppContext, VisualTestContext, Window,
    WindowAppearance, div, point, px, size,
};

use super::Sidebar;
use crate::{
    layout_root::LayoutRoot, layout_tree::LayoutStore, os_appearance::Initial,
    preferences::Preferences, theme::Appearance, theme::Mode,
};

pub(super) struct Row {
    sidebar: Entity<Sidebar>,
    content: Entity<LayoutRoot>,
}

impl Render for Row {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .child(self.sidebar.clone())
            .child(self.content.clone())
    }
}

pub(super) fn open<'a>(
    cx: &'a mut TestAppContext,
    dir: &Path,
    prefs: &str,
) -> (Entity<Row>, &'a mut VisualTestContext) {
    std::fs::write(dir.join(crate::preferences::FILE), prefs).unwrap();
    cx.update(|cx| {
        let initial = Initial {
            os: WindowAppearance::Dark,
            gpui_agrees: true,
        };
        ely_gpui_component::init_for_tests(cx);
        cx.set_global(crate::motion::ReducedMotion::default());
        cx.set_global(crate::units::Zoom(1.0));
        cx.set_global(crate::window_material::Material::load());
        crate::wallpaper::none_for_tests(cx);
        cx.set_global(Preferences::load(dir).unwrap());
        crate::sessions::install(std::path::PathBuf::new(), None, cx);
        crate::router::install(cx);
        crate::localdb::install(None, cx);
        cx.set_global(Appearance::new(Mode::System, None, initial, None).unwrap());
        cx.set_global(crate::i18n::Translator::new("en").unwrap());
        cx.set_global(crate::toast::Toasts::default());
        cx.set_global(LayoutStore::open(dir));
    });
    let (row, vcx) = cx.add_window_view(|_, cx| {
        let sidebar = cx.new(Sidebar::new);
        let painted = sidebar.clone();
        let content =
            cx.new(|cx| LayoutRoot::new(Box::new(move |cx| painted.read(cx).seen(cx)), cx));
        let notify = content.downgrade();
        cx.observe(&sidebar, move |_, _, cx| {
            notify.update(cx, |_, cx| cx.notify()).ok();
        })
        .detach();
        Row { sidebar, content }
    });
    vcx.simulate_resize(size(px(1600.0), px(900.0)));
    vcx.run_until_parked();
    (row, vcx)
}

fn widths(row: &Entity<Row>, cx: &mut VisualTestContext) -> (f32, f32) {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let sidebar = cx
        .debug_bounds("sidebar")
        .map_or(0.0, |b| f32::from(b.size.width));
    let content = cx.update(|_, cx| row.read(cx).content.read(cx).available());
    (sidebar, content as f32)
}

fn wait(cx: &mut VisualTestContext, ms: u64) {
    cx.executor().advance_clock(Duration::from_millis(ms));
    cx.run_until_parked();
}

fn saved(dir: &Path, key: &str) -> Option<String> {
    Preferences::load(dir)
        .unwrap()
        .text(key)
        .unwrap()
        .map(str::to_owned)
}

#[gpui::test]
fn hides_into_the_tree_and_hands_the_space_to_the_content(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (row, cx) = open(cx, dir.path(), r#"{"sidebar-width":"300"}"#);
    assert_eq!(widths(&row, cx), (300.0, 1300.0));
    let sidebar = cx.update(|_, cx| row.read(cx).sidebar.clone());
    cx.update(|_, cx| sidebar.update(cx, |s, cx| s.toggle(cx)));
    wait(cx, 125);
    let (half, content) = widths(&row, cx);
    assert!(half > 0.0 && half < 300.0, "{half}");
    assert!(
        (content - (1600.0 - half)).abs() < 1.0,
        "the content follows every frame"
    );
    wait(cx, 130);
    assert_eq!(widths(&row, cx), (0.0, 1600.0));
    let tree = std::fs::read_to_string(dir.path().join("layout.v1.json")).unwrap();
    assert!(tree.contains("\"collapsed\": true"), "{tree}");

    let reread = cx.update(|_, cx| {
        cx.set_global(LayoutStore::open(dir.path()));
        super::collapsed_in_tree(cx)
    });
    assert!(reread, "a restart stays hidden");
}

fn handle(cx: &mut VisualTestContext, x: f32, clicks: usize) {
    let position = point(px(x - 2.0), px(400.0));
    let modifiers = Modifiers::none();
    cx.simulate_event(MouseDownEvent {
        position,
        modifiers,
        button: MouseButton::Left,
        click_count: clicks,
        first_mouse: false,
    });
}

#[gpui::test]
fn drags_the_width_into_the_rail_and_resets_it(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (row, cx) = open(cx, dir.path(), "{}");
    let none = Modifiers::none();
    handle(cx, 260.0, 1);
    cx.simulate_mouse_move(point(px(358.0), px(400.0)), MouseButton::Left, none);

    assert_eq!(
        widths(&row, cx).0,
        360.0,
        "the width follows the pointer at once"
    );
    cx.simulate_mouse_up(point(px(358.0), px(400.0)), MouseButton::Left, none);
    assert_eq!(saved(dir.path(), "sidebar-width").as_deref(), Some("360"));

    handle(cx, 360.0, 1);
    cx.simulate_mouse_move(point(px(100.0), px(400.0)), MouseButton::Left, none);
    cx.simulate_mouse_up(point(px(100.0), px(400.0)), MouseButton::Left, none);
    wait(cx, 300);
    assert_eq!(widths(&row, cx), (78.0, 1522.0));
    assert_eq!(saved(dir.path(), "sidebar-rail").as_deref(), Some("true"));
    assert_eq!(
        saved(dir.path(), "sidebar-width").as_deref(),
        Some("360"),
        "the rail keeps the width"
    );

    handle(cx, 78.0, 1);
    cx.simulate_event(MouseUpEvent {
        position: point(px(76.0), px(400.0)),
        modifiers: none,
        button: MouseButton::Left,
        click_count: 1,
    });
    handle(cx, 78.0, 2);
    wait(cx, 300);
    assert_eq!(widths(&row, cx).0, 260.0);
    assert_eq!(saved(dir.path(), "sidebar-rail").as_deref(), Some("false"));
    assert_eq!(saved(dir.path(), "sidebar-width").as_deref(), Some("260"));
}

#[gpui::test]
fn a_press_mid_expand_stops_the_slide(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (row, cx) = open(cx, dir.path(), "{}");
    let sidebar = cx.update(|_, cx| row.read(cx).sidebar.clone());
    cx.update(|_, cx| sidebar.update(cx, |s, cx| s.toggle(cx)));
    wait(cx, 300);
    cx.update(|_, cx| sidebar.update(cx, |s, cx| s.toggle(cx)));
    wait(cx, 100);
    let (mid, _) = widths(&row, cx);
    assert!(mid > 0.0 && mid < 260.0, "{mid}");
    handle(cx, mid, 1);
    assert_eq!(widths(&row, cx).0, 260.0, "the drag owns the width at once");
}

#[gpui::test]
fn reduced_motion_hides_without_a_slide(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (row, cx) = open(cx, dir.path(), "{}");
    cx.update(|_, cx| cx.set_global(crate::motion::ReducedMotion(true)));
    let sidebar = cx.update(|_, cx| row.read(cx).sidebar.clone());
    cx.update(|_, cx| sidebar.update(cx, |s, cx| s.toggle(cx)));
    wait(cx, 0);
    assert_eq!(widths(&row, cx), (0.0, 1600.0));
}
