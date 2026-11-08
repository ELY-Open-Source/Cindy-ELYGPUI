//! The main window opened as the app opens it, driven by mouse events.

use gpui::{Modifiers, TestAppContext, VisualTestContext, WindowAppearance, px, size};

use crate::{
    app_shortcuts::AppShortcuts,
    i18n::Translator,
    layout_tree::{LayoutStore, Node},
    os_appearance::Initial,
    preferences::Preferences,
    theme::Appearance,
    theme::Mode,
    toast::Toasts,
};

fn open(cx: &mut TestAppContext, dir: &std::path::Path) -> VisualTestContext {
    open_zoomed(cx, dir, 1.0)
}

fn open_zoomed(cx: &mut TestAppContext, dir: &std::path::Path, zoom: f32) -> VisualTestContext {
    cx.update(|cx| {
        ely_gpui_component::init_for_tests(cx);
        let initial = Initial {
            os: WindowAppearance::Dark,
            gpui_agrees: true,
        };
        cx.set_global(crate::motion::ReducedMotion::default());
        cx.set_global(crate::units::Zoom(zoom));
        cx.set_global(Appearance::new(Mode::System, None, initial, None).unwrap());
        cx.set_global(Translator::new("en").unwrap());
        cx.set_global(Toasts::default());
        cx.set_global(LayoutStore::open(dir));
        cx.set_global(AppShortcuts::load(dir));
        cx.set_global(Preferences::load(dir).unwrap());
        let activation = super::Activation {
            mac_setting: false,
            windows_setting: false,
        };
        super::open(dir.to_owned(), activation, cx).unwrap();
    });
    let window = cx.windows()[0];
    let vcx = VisualTestContext::from_window(window, cx);
    vcx.simulate_resize(size(px(1600.0), px(900.0)));
    vcx.run_until_parked();
    vcx
}

#[cfg(not(target_os = "macos"))]
#[gpui::test]
fn the_header_drags_and_the_controls_close_through_the_saved_state(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let mut vcx = open(cx, dir.path());
    let header = vcx.debug_bounds("content-header").expect("header painted");
    assert_eq!(
        (header.origin.x, header.origin.y, header.size.height),
        (px(260.0), px(0.0), px(46.0))
    );
    let close = vcx
        .debug_bounds("window-control-Close")
        .expect("close painted");
    assert_eq!(close.size, size(px(28.0), px(28.0)));
    assert_eq!(close.origin.x + close.size.width, px(1600.0 - 8.0));
    assert!(!dir.path().join("window-state.json").exists());
    vcx.simulate_click(close.center(), Modifiers::none());
    vcx.run_until_parked();
    assert!(
        dir.path().join("window-state.json").exists(),
        "a drawn close saves as a system close does"
    );
    assert!(cx.windows().is_empty());
}

#[cfg(not(target_os = "macos"))]
#[gpui::test]
fn the_header_follows_the_sidebar(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let mut vcx = open(cx, dir.path());
    let ctrl_b = if cfg!(target_os = "macos") {
        "cmd-b"
    } else {
        "ctrl-b"
    };
    vcx.simulate_keystrokes(ctrl_b);
    vcx.executor()
        .advance_clock(std::time::Duration::from_millis(300));
    vcx.run_until_parked();
    vcx.update(|window, _| window.refresh());
    vcx.run_until_parked();
    let header = vcx.debug_bounds("content-header").expect("header painted");
    assert_eq!(header.origin.x, px(0.0));
}

#[cfg(not(target_os = "macos"))]
#[gpui::test]
fn the_header_starts_inset_and_never_drags_a_panel(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let mut vcx = open(cx, dir.path());
    let header = vcx.debug_bounds("content-header").expect("header painted");
    let slot = vcx
        .debug_bounds("content-header-slot")
        .expect("slot painted");
    assert_eq!(
        slot.origin.x - header.origin.x,
        px(8.0),
        "the first frame does not slide in"
    );
    let none = Modifiers::none();
    vcx.simulate_mouse_down(header.center(), gpui::MouseButton::Left, none);
    vcx.executor()
        .advance_clock(std::time::Duration::from_millis(600));
    vcx.run_until_parked();
    vcx.simulate_mouse_move(
        gpui::point(px(1500.0), px(400.0)),
        gpui::MouseButton::Left,
        none,
    );
    vcx.run_until_parked();
    assert!(
        vcx.debug_bounds("panel-drag-ghost").is_none(),
        "the header band is the window's"
    );
    vcx.simulate_mouse_up(
        gpui::point(px(1500.0), px(400.0)),
        gpui::MouseButton::Left,
        none,
    );
}

fn wait(vcx: &mut VisualTestContext, ms: u64) {
    vcx.executor()
        .advance_clock(std::time::Duration::from_millis(ms));
    vcx.run_until_parked();
    vcx.update(|window, _| window.refresh());
    vcx.run_until_parked();
}

fn width(vcx: &mut VisualTestContext, selector: &'static str) -> Option<gpui::Pixels> {
    vcx.debug_bounds(selector).map(|b| b.size.width)
}

#[cfg(not(target_os = "macos"))]
#[gpui::test]
fn the_toggle_hides_the_sidebar_and_the_header_makes_room(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let mut vcx = open(cx, dir.path());
    let toggle = vcx.debug_bounds("sidebar-toggle").expect("toggle painted");
    assert_eq!((toggle.origin.x, toggle.size.width), (px(8.0), px(28.0)));
    vcx.simulate_click(toggle.center(), Modifiers::none());
    wait(&mut vcx, 300);
    assert_eq!(width(&mut vcx, "sidebar"), Some(px(0.0)));
    let header = vcx.debug_bounds("content-header").unwrap();
    let slot = vcx.debug_bounds("content-header-slot").unwrap();
    assert_eq!(
        slot.origin.x - header.origin.x,
        px(8.0 + 132.0),
        "room for the cluster"
    );
}

#[cfg(not(target_os = "macos"))]
#[gpui::test]
fn hovering_the_toggle_peeks_and_a_click_pins(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let mut vcx = open(cx, dir.path());
    let toggle = vcx.debug_bounds("sidebar-toggle").unwrap().center();
    vcx.simulate_keystrokes("ctrl-b");
    wait(&mut vcx, 300);
    let none = Modifiers::none();
    vcx.simulate_mouse_move(gpui::point(px(900.0), px(400.0)), None, none);
    vcx.simulate_mouse_move(toggle, None, none);
    wait(&mut vcx, 100);
    assert!(
        vcx.debug_bounds("sidebar-peek-drawer").is_none(),
        "120ms of intent first"
    );
    wait(&mut vcx, 300);
    let drawer = vcx
        .debug_bounds("sidebar-peek-drawer")
        .expect("the drawer slides out");
    assert_eq!((drawer.origin.x, drawer.size.width), (px(0.0), px(260.0)));
    assert_eq!(
        width(&mut vcx, "sidebar"),
        Some(px(0.0)),
        "the content does not move"
    );
    vcx.simulate_mouse_move(gpui::point(px(100.0), px(400.0)), None, none);
    wait(&mut vcx, 1000);
    assert!(
        vcx.debug_bounds("sidebar-peek-drawer").is_some(),
        "the drawer holds the pointer"
    );

    vcx.simulate_mouse_move(gpui::point(px(900.0), px(400.0)), None, none);
    wait(&mut vcx, 159);
    assert!(
        vcx.debug_bounds("sidebar-peek-drawer").is_some(),
        "the grace holds it"
    );
    wait(&mut vcx, 400);
    assert!(vcx.debug_bounds("sidebar-peek-drawer").is_none());

    vcx.simulate_mouse_move(toggle, None, none);
    wait(&mut vcx, 400);
    vcx.simulate_click(toggle, none);
    wait(&mut vcx, 100);
    assert!(
        vcx.debug_bounds("sidebar-peek-drawer").is_some(),
        "pinning keeps the drawer"
    );
    wait(&mut vcx, 400);
    assert!(vcx.debug_bounds("sidebar-peek-drawer").is_none());
    assert_eq!(width(&mut vcx, "sidebar"), Some(px(260.0)));
}

/// Hidden by Ctrl+B, then hovered on the toggle until `ms` after the peek opened.
#[cfg(not(target_os = "macos"))]
fn peeking(
    cx: &mut TestAppContext,
    dir: &std::path::Path,
    ms: u64,
) -> (VisualTestContext, gpui::Point<gpui::Pixels>) {
    let mut vcx = open(cx, dir);
    let toggle = vcx.debug_bounds("sidebar-toggle").unwrap().center();
    vcx.simulate_keystrokes("ctrl-b");
    wait(&mut vcx, 300);
    let none = Modifiers::none();
    vcx.simulate_mouse_move(gpui::point(px(900.0), px(400.0)), None, none);
    vcx.simulate_mouse_move(toggle, None, none);
    wait(&mut vcx, 120 + ms);
    (vcx, toggle)
}

#[cfg(not(target_os = "macos"))]
#[gpui::test]
fn a_pin_mid_slide_shows_the_whole_drawer(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut vcx, toggle) = peeking(cx, dir.path(), 10);
    vcx.simulate_click(toggle, Modifiers::none());
    wait(&mut vcx, 0);
    assert_eq!(
        vcx.debug_bounds("sidebar-peek-drawer").unwrap().origin.x,
        px(0.0)
    );
}

#[cfg(not(target_os = "macos"))]
#[gpui::test]
fn a_pin_while_sliding_out_shows_the_whole_drawer(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut vcx, toggle) = peeking(cx, dir.path(), 400);
    vcx.simulate_mouse_move(gpui::point(px(900.0), px(400.0)), None, Modifiers::none());
    wait(&mut vcx, 160 + 60);
    assert!(
        vcx.debug_bounds("sidebar-peek-drawer").unwrap().origin.x < px(0.0),
        "sliding out"
    );
    vcx.simulate_click(toggle, Modifiers::none());
    wait(&mut vcx, 0);
    assert_eq!(
        vcx.debug_bounds("sidebar-peek-drawer").unwrap().origin.x,
        px(0.0)
    );
}

#[cfg(not(target_os = "macos"))]
#[gpui::test]
fn hiding_during_the_pin_drops_the_column_at_once(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (mut vcx, toggle) = peeking(cx, dir.path(), 400);
    vcx.simulate_click(toggle, Modifiers::none());
    wait(&mut vcx, 100);
    vcx.simulate_click(toggle, Modifiers::none());
    wait(&mut vcx, 0);
    assert_eq!(width(&mut vcx, "sidebar"), Some(px(0.0)));
    assert!(vcx.debug_bounds("sidebar-peek-drawer").is_none());
}

#[cfg(not(target_os = "macos"))]
#[gpui::test]
fn reduced_motion_shows_the_drawer_without_a_slide(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let mut vcx = open(cx, dir.path());
    vcx.update(|_, cx| cx.set_global(crate::motion::ReducedMotion(true)));
    let toggle = vcx.debug_bounds("sidebar-toggle").unwrap().center();
    vcx.simulate_keystrokes("ctrl-b");
    wait(&mut vcx, 0);
    assert_eq!(width(&mut vcx, "sidebar"), Some(px(0.0)));
    let none = Modifiers::none();
    vcx.simulate_mouse_move(gpui::point(px(900.0), px(400.0)), None, none);
    vcx.simulate_mouse_move(toggle, None, none);
    wait(&mut vcx, 120);
    assert_eq!(
        vcx.debug_bounds("sidebar-peek-drawer").unwrap().origin.x,
        px(0.0)
    );
}

#[cfg(not(target_os = "macos"))]
#[gpui::test]
fn zoom_scales_every_css_pixel(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let mut vcx = open_zoomed(cx, dir.path(), 1.5);
    assert_eq!(width(&mut vcx, "sidebar"), Some(px(390.0)));
    let header = vcx.debug_bounds("content-header").unwrap();
    assert_eq!((header.origin.x, header.size.height), (px(390.0), px(69.0)));
    let slot = vcx.debug_bounds("content-header-slot").unwrap();
    assert_eq!(slot.size.height, px(69.0 - 1.5), "the border zooms too");
    let toggle = vcx.debug_bounds("sidebar-toggle").unwrap();
    assert_eq!((toggle.origin.x, toggle.size.width), (px(12.0), px(42.0)));
    let none = Modifiers::none();
    let edge = gpui::point(px(390.0 - 3.0), px(400.0));
    vcx.simulate_mouse_down(edge, gpui::MouseButton::Left, none);
    vcx.simulate_mouse_move(
        edge + gpui::point(px(150.0), px(0.0)),
        gpui::MouseButton::Left,
        none,
    );
    vcx.simulate_mouse_up(
        edge + gpui::point(px(150.0), px(0.0)),
        gpui::MouseButton::Left,
        none,
    );
    wait(&mut vcx, 0);
    let saved = Preferences::load(dir.path()).unwrap();
    assert_eq!(
        saved.text("sidebar-width").unwrap(),
        Some("360"),
        "100 CSS pixels wider"
    );
    assert_eq!(width(&mut vcx, "sidebar"), Some(px(540.0)));
}

#[cfg(not(target_os = "macos"))]
#[gpui::test]
fn zoom_keeps_divider_math_in_css_pixels(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let mut vcx = open_zoomed(cx, dir.path(), 1.5);
    let avail = 1600.0 / 1.5 - 260.0;
    let right = (avail * 0.5) as f32 * 1.5;
    let seam = gpui::point(px(1600.0 - right - 0.75), px(400.0));
    let none = Modifiers::none();
    vcx.simulate_mouse_down(seam, gpui::MouseButton::Left, none);
    vcx.simulate_mouse_move(
        seam + gpui::point(px(150.0), px(0.0)),
        gpui::MouseButton::Left,
        none,
    );
    vcx.simulate_mouse_up(
        seam + gpui::point(px(150.0), px(0.0)),
        gpui::MouseButton::Left,
        none,
    );
    wait(&mut vcx, 0);
    let fraction = vcx.update(|_, cx| {
        let Node::Split(root) = &cx.global::<LayoutStore>().layout().content else {
            panic!("row")
        };
        root.children[1].fraction
    });
    assert!(
        (fraction - (0.5 - 100.0 / avail)).abs() < 1e-3,
        "{fraction}"
    );
}
