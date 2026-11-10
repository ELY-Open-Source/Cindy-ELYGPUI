use gpui::{Bounds, Pixels, TestAppContext, VisualTestContext, px};

use crate::{layout_root::screen_tests, sessions};

/// The content area `width` wide with `s1` open.
fn open(cx: &mut TestAppContext, dir: &std::path::Path, width: f32) -> Parts {
    let cx = screen_tests::open(cx, dir, screen_tests::tree(0.6, 0.4), width);
    cx.update(|_, cx| {
        crate::localdb::serve(super::super::tests::db(), cx);
        sessions::load_for_tests(&[("s1", "active", "desktop")], cx);
    });
    cx.run_until_parked();
    parts(cx)
}

struct Parts {
    view: Bounds<Pixels>,
    messages: Bounds<Pixels>,
    scroll: Bounds<Pixels>,
    fade: Bounds<Pixels>,
    zone: Bounds<Pixels>,
    slot: Bounds<Pixels>,
}

fn parts(cx: &mut VisualTestContext) -> Parts {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let mut at = |selector| cx.debug_bounds(selector).expect(selector);
    Parts {
        view: at("session-view"),
        messages: at("session-messages"),
        scroll: at("session-scroll"),
        fade: at("session-composer-fade"),
        zone: at("session-composer-zone"),
        slot: at("session-composer-slot"),
    }
}

/// The column's widths against the hook's, and the dock's place at the view's foot.
fn check(p: &Parts, message: f32, input: f32, pad: f32) {
    assert_eq!(p.messages.size.width, px(message));
    let margin = (p.view.size.width - px(message)) / 2.0;
    assert_eq!(p.messages.left(), p.view.left() + margin, "mx-auto");
    assert_eq!(p.scroll.size, p.view.size, "the scroller fills the view");
    assert_eq!(p.messages.size.height, px(28.0), "pt-7 over no row");
    assert_eq!(p.slot.size.width, px(input));
    assert_eq!(p.zone.left(), p.view.left() + px(pad));
    assert_eq!(p.zone.right(), p.view.right() - px(pad));
    assert_eq!(p.zone.bottom(), p.view.bottom());
    assert_eq!(p.zone.size.height, px(20.0), "pb-5 around an empty row");
    assert_eq!(p.fade.size.height, px(32.0));
    assert_eq!(p.fade.bottom(), p.zone.top());
    assert_eq!(p.fade.size.width, p.zone.size.width);
    let centre = p.slot.left() + p.slot.size.width / 2.0;
    assert_eq!(centre, p.view.left() + p.view.size.width / 2.0);
}

#[gpui::test]
fn a_wide_column_caps_the_messages_and_centres_them(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let p = open(cx, dir.path(), 2400.0);
    assert!(p.view.size.width > px(1014.0));
    check(&p, 914.0, 934.0, 40.0);
}

#[gpui::test]
fn a_column_under_the_cap_keeps_50_beside_the_messages(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let p = open(cx, dir.path(), 1400.0);
    let width = f32::from(p.view.size.width);
    assert!((700.0..1014.0).contains(&width), "{width}");
    check(&p, width - 100.0, width - 80.0, 40.0);
}

#[gpui::test]
fn a_column_under_700_is_compact(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let p = open(cx, dir.path(), 900.0);
    let width = f32::from(p.view.size.width);
    assert!(width < 700.0, "{width}");
    check(&p, width - 40.0, width - 20.0, 10.0);
}
