//! The rules of `lib/toast.ts`, driven through a window as Cindy's renderer would.

use std::{cell::Cell, rc::Rc, time::Duration};

use gpui::{
    AppContext as _, Context, Entity, IntoElement, Modifiers, ParentElement, Render, Styled,
    TestAppContext, VisualTestContext, Window, WindowAppearance, div, point, px, size,
};

use super::{Options, ToastContainer, Toasts, Variant};
use crate::{i18n::Translator, os_appearance::Initial, theme::Appearance, theme::Mode};

struct Host {
    toasts: Entity<ToastContainer>,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().relative().size_full().child(self.toasts.clone())
    }
}

fn open(cx: &mut TestAppContext) -> &mut VisualTestContext {
    open_with_host(cx).1
}

fn open_with_host(cx: &mut TestAppContext) -> (Entity<Host>, &mut VisualTestContext) {
    cx.update(|cx| {
        ely_gpui_component::init_for_tests(cx);
        let initial = Initial {
            os: WindowAppearance::Light,
            gpui_agrees: true,
        };
        cx.set_global(crate::motion::ReducedMotion::default());
        cx.set_global(crate::units::Zoom(1.0));
        cx.set_global(crate::window_material::Material::load());
        crate::wallpaper::none_for_tests(cx);
        crate::sessions::install(std::path::PathBuf::new(), None, cx);
        crate::localdb::install(None, cx);
        cx.set_global(Appearance::new(Mode::System, None, initial, None).unwrap());
        cx.set_global(Translator::new("en").unwrap());
        cx.set_global(Toasts::default());
    });
    let (host, vcx) = cx.add_window_view(|window, cx| Host {
        toasts: cx.new(|cx| ToastContainer::new(window, cx)),
    });
    vcx.simulate_resize(size(px(1200.0), px(800.0)));
    (host, vcx)
}

/// Shown messages, newest first, with their exiting flag.
fn shown(cx: &mut VisualTestContext) -> Vec<(String, bool)> {
    cx.update(|window, cx| {
        let window = window.window_handle().window_id();
        cx.global::<Toasts>()
            .items(window)
            .iter()
            .map(|t| (t.message.to_string(), t.exiting))
            .collect()
    })
}

fn wait(cx: &mut VisualTestContext, ms: u64) {
    cx.executor().advance_clock(Duration::from_millis(ms));
    cx.run_until_parked();
}

fn names(list: &[(String, bool)]) -> Vec<&str> {
    list.iter().map(|(m, _)| m.as_str()).collect()
}

#[gpui::test]
fn shows_three_and_queues_the_rest_in_order(cx: &mut TestAppContext) {
    let cx = open(cx);
    let closed = Rc::new(Cell::new(0));
    cx.update(|window, cx| {
        for (i, message) in ["a", "b", "c", "d"].into_iter().enumerate() {
            let closed = closed.clone();
            let on_close = Rc::new(move |_: &mut gpui::App| closed.set(closed.get() + 1));
            let duration = Some(Duration::from_millis(1000 + 100 * i as u64));
            super::info(
                window,
                message,
                Options {
                    duration,
                    on_close: Some(on_close),
                    ..Default::default()
                },
                cx,
            );
        }
    });
    wait(cx, 0);
    assert_eq!(names(&shown(cx)), ["c", "b", "a"]);
    wait(cx, 1000);
    assert_eq!(
        shown(cx),
        [("c".into(), false), ("b".into(), false), ("a".into(), true)]
    );
    wait(cx, 300);
    assert_eq!(
        names(&shown(cx)),
        ["d", "c", "b"],
        "the queued one takes the top"
    );
    assert_eq!(closed.get(), 1);
    wait(cx, 1299);
    assert!(!shown(cx)[0].1, "d's clock started when it was shown");
    wait(cx, 1);
    assert!(shown(cx)[0].1);
}

#[gpui::test]
fn lasts_by_variant(cx: &mut TestAppContext) {
    let cx = open(cx);
    cx.update(|window, cx| {
        super::success(window, "ok", Options::default(), cx);
        super::error(window, "bad", Options::default(), cx);
        super::loading(window, "busy", Options::default(), cx);
    });
    let exiting = |cx: &mut VisualTestContext| -> Vec<String> {
        shown(cx).into_iter().filter(|t| t.1).map(|t| t.0).collect()
    };
    wait(cx, 1199);
    assert!(exiting(cx).is_empty());
    wait(cx, 1);
    assert_eq!(exiting(cx), ["ok"]);
    wait(cx, 8000 - 1200 - 1);
    assert!(exiting(cx).is_empty(), "the error lasts 8s");
    wait(cx, 1);
    assert_eq!(exiting(cx), ["bad"]);
    wait(cx, 300);
    assert_eq!(
        shown(cx),
        [("busy".into(), false)],
        "loading stays until dismissed"
    );
    let kind = cx.update(|window, cx| {
        cx.global::<Toasts>()
            .items(window.window_handle().window_id())[0]
            .variant
    });
    assert_eq!(kind, Variant::Loading);
}

#[gpui::test]
fn hover_holds_the_clock_and_leaves_at_least_a_second(cx: &mut TestAppContext) {
    let cx = open(cx);
    cx.update(|window, cx| super::info(window, "hold", Options::default(), cx));
    wait(cx, 1100);
    cx.simulate_mouse_move(point(px(600.0), px(40.0)), None, Modifiers::none());
    wait(cx, 5000);
    assert_eq!(shown(cx), [("hold".into(), false)]);
    cx.simulate_mouse_move(point(px(600.0), px(700.0)), None, Modifiers::none());
    wait(cx, 999);
    assert_eq!(shown(cx), [("hold".into(), false)]);
    wait(cx, 1);
    assert_eq!(shown(cx), [("hold".into(), true)]);
}

#[gpui::test]
fn dismissing_and_updating(cx: &mut TestAppContext) {
    let cx = open(cx);
    let window = cx.update(|window, cx| {
        for message in ["a", "b", "c", "d"] {
            super::warning(window, message, Options::default(), cx);
        }
        window.window_handle().window_id()
    });
    cx.update(|_, cx| super::update(window, 1, "a2", cx));
    assert_eq!(names(&shown(cx)), ["c", "b", "a2"]);
    cx.update(|_, cx| super::dismiss(window, 4, cx));
    wait(cx, 300);
    cx.update(|_, cx| super::dismiss(window, 3, cx));
    wait(cx, 300);
    assert_eq!(
        names(&shown(cx)),
        ["b", "a2"],
        "the queued d went with no fade"
    );
    cx.update(|_, cx| super::dismiss_all(window, cx));
    wait(cx, 300);
    assert!(shown(cx).is_empty());
}

#[gpui::test]
fn a_wrapped_message_drops_the_pill(cx: &mut TestAppContext) {
    let (host, cx) = open_with_host(cx);
    cx.update(|window, cx| {
        super::info(window, "Saved", Options::default(), cx);
        super::info(window, "word ".repeat(120), Options::default(), cx);
    });
    wait(cx, 0);
    let toasts = cx.update(|_, cx| host.read(cx).toasts.clone());
    let multiline = cx.update(|_, cx| toasts.read(cx).multiline());
    assert_eq!(multiline, [(1, false), (2, true)]);
}

#[gpui::test]
fn hovering_a_fading_toast_does_not_hold_it(cx: &mut TestAppContext) {
    let cx = open(cx);
    let window = cx.update(|window, cx| {
        for message in ["a", "b", "c", "d"] {
            super::info(window, message, Options::default(), cx);
        }
        window.window_handle().window_id()
    });
    wait(cx, 0);
    cx.update(|_, cx| super::dismiss(window, 3, cx));
    wait(cx, 100);
    cx.simulate_mouse_move(point(px(600.0), px(40.0)), None, Modifiers::none());
    wait(cx, 200);
    assert_eq!(
        names(&shown(cx)),
        ["d", "b", "a"],
        "c left and d took its place"
    );
}
