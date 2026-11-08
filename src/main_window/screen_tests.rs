//! The main window opened as the app opens it, driven by mouse events.

use gpui::{Modifiers, TestAppContext, VisualTestContext, WindowAppearance, px, size};

use crate::{
    app_shortcuts::AppShortcuts, i18n::Translator, layout_tree::LayoutStore,
    os_appearance::Initial, preferences::Preferences, theme::Appearance, theme::Mode,
    toast::Toasts,
};

fn open(cx: &mut TestAppContext, dir: &std::path::Path) -> VisualTestContext {
    cx.update(|cx| {
        ely_gpui_component::init_for_tests(cx);
        let initial = Initial {
            os: WindowAppearance::Dark,
            gpui_agrees: true,
        };
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
