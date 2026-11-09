//! `ConfirmDialog`'s standard presentation, driven through a window with real events.

use std::{cell::RefCell, rc::Rc, time::Duration};

use gpui::{
    Context, Entity, FocusHandle, InteractiveElement as _, IntoElement, Modifiers,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, TestAppContext,
    VisualTestContext, Window, WindowAppearance, div, px, size,
};

use super::ConfirmDialog;
use crate::{os_appearance::Initial, theme::Appearance, theme::Mode};

type Log = Rc<RefCell<Vec<&'static str>>>;

struct Host {
    open: bool,
    disabled: bool,
    auto: bool,
    cancel_first: bool,
    tall: bool,
    outside: FocusHandle,
    log: Log,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (closer, confirm_log, outside_log) =
            (cx.entity().downgrade(), self.log.clone(), self.log.clone());
        let close_log = self.log.clone();
        let content = self
            .tall
            .then(|| div().children((0..60).map(|i| div().h(px(30.0)).child(format!("line {i}")))));
        let mut dialog = ConfirmDialog::new(
            "test-confirm",
            self.open,
            ("Title".into(), "Continue".into(), "Later".into()),
            move |_, cx| {
                close_log.borrow_mut().push("close");
                closer
                    .update(cx, |host, cx| {
                        host.open = false;
                        cx.notify();
                    })
                    .unwrap();
            },
            move |_, _| confirm_log.borrow_mut().push("confirm"),
        )
        .max_width(460.0)
        .description("Some tasks stopped.")
        .confirm_disabled(self.disabled);
        if let Some(content) = content {
            dialog = dialog.content(content);
        }
        if self.auto {
            dialog = dialog.auto_focus_confirm();
        }
        if self.cancel_first {
            dialog = dialog.cancel_first();
        }
        div()
            .size_full()
            .child(
                div()
                    .id("outside")
                    .debug_selector(|| "outside".into())
                    .track_focus(&self.outside)
                    .size(px(60.0))
                    .on_click(move |_, _, _| outside_log.borrow_mut().push("outside")),
            )
            .child(dialog)
    }
}

struct Setup {
    disabled: bool,
    auto: bool,
    tall: bool,
}

const STANDARD: Setup = Setup {
    disabled: false,
    auto: true,
    tall: false,
};

fn open(cx: &mut TestAppContext, setup: Setup) -> (Entity<Host>, Log, &mut VisualTestContext) {
    cx.update(|cx| {
        ely_gpui_component::init_for_tests(cx);
        let initial = Initial {
            os: WindowAppearance::Light,
            gpui_agrees: true,
        };
        cx.set_global(crate::motion::ReducedMotion::default());
        cx.set_global(crate::units::Zoom(1.0));
        cx.set_global(Appearance::new(Mode::System, None, initial, None).unwrap());
    });
    let log = Log::default();
    let shared = log.clone();
    let (host, vcx) = cx.add_window_view(|_, cx| Host {
        open: false,
        disabled: setup.disabled,
        auto: setup.auto,
        cancel_first: true,
        tall: setup.tall,
        outside: cx.focus_handle().tab_stop(true),
        log: shared,
    });
    vcx.simulate_resize(size(px(1200.0), px(800.0)));
    vcx.update(|window, cx| {
        let outside = host.read(cx).outside.clone();
        window.focus(&outside, cx);
    });
    vcx.run_until_parked();
    host.update(vcx, |host, cx| {
        host.open = true;
        cx.notify();
    });
    vcx.run_until_parked();
    (host, log, vcx)
}

/// Lets the clock run and paints the frames the dialog asked for.
fn wait(cx: &mut VisualTestContext, ms: u64) {
    cx.executor().advance_clock(Duration::from_millis(ms));
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let at = cx.debug_bounds(selector).unwrap().center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
}

/// A key's down and up, as gpui clicks a focused button on the up.
fn press(cx: &mut VisualTestContext, keys: &'static str) {
    for key in keys.split(' ') {
        cx.simulate_keystrokes(key);
        cx.simulate_event(gpui::KeyUpEvent {
            keystroke: gpui::Keystroke::parse(key).unwrap(),
        });
    }
    cx.run_until_parked();
}

fn outside_focused(host: &Entity<Host>, cx: &mut VisualTestContext) -> bool {
    cx.update(|window, cx| host.read(cx).outside.is_focused(window))
}

#[gpui::test]
fn lays_out_the_standard_panel(cx: &mut TestAppContext) {
    let (_, _, cx) = open(cx, STANDARD);
    wait(cx, 300);
    let panel = cx.debug_bounds("confirm-dialog-panel").unwrap();
    assert_eq!(panel.size.width, px(460.0), "maxWidth");
    assert_eq!(panel.center().x, px(600.0));
    assert!((panel.center().y - px(400.0)).abs() < px(0.5), "centered");
    let (cancel, confirm) = (
        cx.debug_bounds("confirm-dialog-cancel").unwrap(),
        cx.debug_bounds("confirm-dialog-confirm").unwrap(),
    );
    assert!(cancel.origin.x < confirm.origin.x, "cancel first");
    assert_eq!(
        confirm.right(),
        panel.right() - px(17.0),
        "justified end inside p-4"
    );
    assert_eq!(confirm.origin.x - cancel.right(), px(10.0), "gap-2.5");
    assert!(cancel.size.width >= px(96.0) && cancel.size.height >= px(36.0));
}

#[gpui::test]
fn enters_from_8px_below(cx: &mut TestAppContext) {
    let (_, _, cx) = open(cx, STANDARD);
    let first = cx.debug_bounds("confirm-dialog-panel").unwrap().origin.y;
    wait(cx, 300);
    let settled = cx.debug_bounds("confirm-dialog-panel").unwrap().origin.y;
    assert!(
        first > settled && first - settled <= px(8.0),
        "{first:?} {settled:?}"
    );
}

#[gpui::test]
fn enter_confirms_the_focused_confirm_then_closes(cx: &mut TestAppContext) {
    let (host, log, cx) = open(cx, STANDARD);
    press(cx, "enter");
    assert_eq!(*log.borrow(), ["confirm", "close"]);
    assert!(!host.read_with(cx, |h, _| h.open));
    assert!(outside_focused(&host, cx), "focus goes back");
}

#[gpui::test]
fn cancel_has_focus_without_auto_focus(cx: &mut TestAppContext) {
    let (_, log, cx) = open(
        cx,
        Setup {
            auto: false,
            ..STANDARD
        },
    );
    press(cx, "enter");
    assert_eq!(*log.borrow(), ["close"]);
}

#[gpui::test]
fn tab_stays_inside(cx: &mut TestAppContext) {
    let (_, log, cx) = open(cx, STANDARD);
    press(cx, "tab tab enter");
    assert_eq!(*log.borrow(), ["confirm", "close"], "back on the confirm");
}

#[gpui::test]
fn escape_closes_and_fades_out(cx: &mut TestAppContext) {
    let (host, log, cx) = open(cx, STANDARD);
    press(cx, "escape");
    assert_eq!(*log.borrow(), ["close"]);
    assert!(outside_focused(&host, cx));
    wait(cx, 0);
    assert!(
        cx.debug_bounds("confirm-dialog-panel").is_some(),
        "drawn while it fades"
    );
    click(cx, "confirm-dialog-confirm");
    assert_eq!(*log.borrow(), ["close"], "nothing runs while it fades");
    wait(cx, 150);
    wait(cx, 0);
    assert!(cx.debug_bounds("confirm-dialog-panel").is_none());
}

#[gpui::test]
fn a_press_on_the_scrim_keeps_it_and_reaches_nothing_below(cx: &mut TestAppContext) {
    let (host, log, cx) = open(cx, STANDARD);
    wait(cx, 300);
    click(cx, "outside");
    assert!(log.borrow().is_empty());
    assert!(host.read_with(cx, |h, _| h.open));
}

#[gpui::test]
fn a_disabled_confirm_takes_no_click_or_focus(cx: &mut TestAppContext) {
    let (host, log, cx) = open(
        cx,
        Setup {
            disabled: true,
            ..STANDARD
        },
    );
    wait(cx, 300);
    click(cx, "confirm-dialog-confirm");
    assert!(log.borrow().is_empty());
    assert!(!outside_focused(&host, cx), "the panel holds focus");
    press(cx, "tab enter");
    assert_eq!(*log.borrow(), ["close"], "Tab reaches the cancel only");
}

#[gpui::test]
fn tall_content_scrolls_between_title_and_buttons(cx: &mut TestAppContext) {
    let (_, _, cx) = open(
        cx,
        Setup {
            tall: true,
            ..STANDARD
        },
    );
    wait(cx, 300);
    let panel = cx.debug_bounds("confirm-dialog-panel").unwrap();
    assert_eq!(panel.size.height, px(680.0), "85vh");
    let confirm = cx.debug_bounds("confirm-dialog-confirm").unwrap();
    assert_eq!(
        confirm.bottom(),
        panel.bottom() - px(17.0),
        "the buttons stay"
    );
    let body = cx.debug_bounds("confirm-dialog-body").unwrap();
    assert!(body.bottom() <= confirm.top() - px(24.0));
}

#[gpui::test]
fn the_cancel_button_closes(cx: &mut TestAppContext) {
    let (host, log, cx) = open(cx, STANDARD);
    wait(cx, 300);
    click(cx, "confirm-dialog-cancel");
    assert_eq!(*log.borrow(), ["close"]);
    assert!(outside_focused(&host, cx));
}

#[gpui::test]
fn reduced_motion_shows_and_drops_it_at_once(cx: &mut TestAppContext) {
    let (_, _, cx) = open(cx, STANDARD);
    cx.update(|_, cx| cx.set_global(crate::motion::ReducedMotion(true)));
    press(cx, "escape");
    wait(cx, 0);
    wait(cx, 0);
    assert!(cx.debug_bounds("confirm-dialog-panel").is_none());
}

/// How far the confirm's face reaches past its padding box.
fn face_outset(cx: &mut VisualTestContext) -> f32 {
    let button = cx.debug_bounds("confirm-dialog-confirm").unwrap();
    let face = cx.debug_bounds("test-confirm-confirm-face").unwrap();
    (button.origin.x - face.origin.x) / px(1.0)
}

/// The border box at rest, the padding box pressed.
const REST: f32 = 0.0;
const PRESSED: f32 = -1.0;

fn space_down(cx: &mut VisualTestContext) {
    cx.simulate_keystrokes("space");
    cx.run_until_parked();
}

/// Advances the clock and draws; whether the last drawn frame asked for it.
fn asks(cx: &mut VisualTestContext, ms: u64) -> bool {
    cx.executor().advance_clock(Duration::from_millis(ms));
    let asked = cx.update(|window, cx| window.simulate_next_frame(cx)) > 0;
    cx.run_until_parked();
    asked
}

#[gpui::test]
fn held_space_presses_the_face_in_over_80ms(cx: &mut TestAppContext) {
    let (_, log, cx) = open(cx, STANDARD);
    wait(cx, 300);
    assert!(!asks(cx, 0), "settled");
    assert_eq!(face_outset(cx), REST);
    space_down(cx);
    assert_eq!(
        face_outset(cx),
        REST,
        "the transition starts where the face was"
    );
    assert!(asks(cx, 79), "the press asked for a frame");
    assert!(asks(cx, 21), "the frame at 79ms still moved");
    assert_eq!(face_outset(cx), PRESSED, "the 80ms inset is in");
    assert!(asks(cx, 0), "the 150ms fill still runs at 100ms");
    assert!(asks(cx, 50));
    assert!(!asks(cx, 0), "the 150ms fill ended");
    assert_eq!(face_outset(cx), PRESSED);
    assert!(log.borrow().is_empty(), "the click waits for the key up");
    cx.simulate_event(gpui::KeyUpEvent {
        keystroke: gpui::Keystroke::parse("space").unwrap(),
    });
    cx.run_until_parked();
    assert_eq!(*log.borrow(), ["confirm", "close"]);
}

#[gpui::test]
fn focus_leaving_lets_the_space_press_go(cx: &mut TestAppContext) {
    let (_, _, cx) = open(cx, STANDARD);
    wait(cx, 300);
    space_down(cx);
    wait(cx, 100);
    wait(cx, 0);
    assert_eq!(face_outset(cx), PRESSED);
    press(cx, "shift-tab");
    wait(cx, 100);
    wait(cx, 0);
    assert_eq!(face_outset(cx), REST);
    cx.simulate_event(gpui::KeyUpEvent {
        keystroke: gpui::Keystroke::parse("space").unwrap(),
    });
    press(cx, "tab");
    wait(cx, 100);
    wait(cx, 0);
    assert_eq!(face_outset(cx), REST, "the press is gone, not hidden");
}

#[gpui::test]
fn reduced_motion_stops_a_running_fill(cx: &mut TestAppContext) {
    let (_, _, cx) = open(cx, STANDARD);
    wait(cx, 300);
    space_down(cx);
    assert!(asks(cx, 40));
    cx.update(|_, cx| cx.set_global(crate::motion::ReducedMotion(true)));
    assert!(asks(cx, 0), "the frame at 40ms asked");
    assert!(!asks(cx, 0), "the fill and inset stopped");
}

#[gpui::test]
fn reduced_motion_keeps_the_face_out(cx: &mut TestAppContext) {
    let (_, _, cx) = open(cx, STANDARD);
    cx.update(|_, cx| cx.set_global(crate::motion::ReducedMotion(true)));
    wait(cx, 0);
    space_down(cx);
    wait(cx, 100);
    assert_eq!(face_outset(cx), REST);
}

#[test]
fn colors_mix_premultiplied_in_their_space() {
    use gpui::{Hsla, Rgba};
    let rgba = |r, g, b, a| Hsla::from(Rgba { r, g, b, a });
    let clear = rgba(0.0, 0.0, 0.0, 0.0);
    let tint = rgba(1.0, 1.0, 1.0, 0.06);
    let mid: Rgba = super::button::mix_srgb(clear, tint, 0.5).into();
    assert!(
        (mid.a - 0.03).abs() < 1e-4 && (mid.r - 1.0).abs() < 1e-3,
        "{mid:?}"
    );
    let (dark, light) = (rgba(0.09, 0.09, 0.09, 1.0), rgba(0.2, 0.2, 0.2, 1.0));
    let ends: [Rgba; 2] = [
        super::button::mix_oklab(dark, light, 0.0).into(),
        super::button::mix_oklab(dark, light, 1.0).into(),
    ];
    assert!((ends[0].r - 0.09).abs() < 1e-3 && (ends[1].r - 0.2).abs() < 1e-3);
    let half: Rgba = super::button::mix_oklab(dark, light, 0.5).into();
    let srgb: Rgba = super::button::mix_srgb(dark, light, 0.5).into();
    assert!(
        half.r < srgb.r,
        "Oklab's midpoint is darker in sRGB: {half:?} {srgb:?}"
    );
}
