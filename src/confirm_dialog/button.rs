//! `Button` at `size="lg"` in the `confirmation` palette, as `ConfirmDialog` draws its actions.

use std::{rc::Rc, time::Duration};

use gpui::{
    App, BoxShadow, Div, ElementId, FocusHandle, Hsla, InteractiveElement as _, MouseButton,
    ParentElement as _, Rgba, Role, SharedString, Stateful, StatefulInteractiveElement as _,
    Styled as _, Window, div, point, prelude::FluentBuilder as _, px, relative,
};

use super::transition::Transition;
use crate::{
    appearance_settings::text_size,
    motion::{self, ReducedMotion},
    theme::{Appearance, oklch},
    units::{css, window_px},
};

/// `variant="primary"` or `"secondary"`.
#[derive(Clone, Copy, PartialEq)]
pub enum Look {
    Primary,
    Secondary,
}

pub type Run = Rc<dyn Fn(&mut Window, &mut App)>;

/// `--motion-instant`.
const INSTANT: Duration = Duration::from_millis(80);

/// Space presses a native button; Enter clicks it without `:active`.
fn is_space(keystroke: &gpui::Keystroke) -> bool {
    keystroke.key == "space" && !keystroke.modifiers.modified()
}

/// `:hover` and `:active`, whatever the input, and the face's transitions.
struct Face {
    hovered: bool,
    mouse: bool,
    /// Space held while focused.
    key: bool,
    fill: Transition<Hsla>,
    inset: Transition<f32>,
}

/// The face's border, fill, hover and press fills, and the text color.
fn tones(look: Look, a: &Appearance) -> [Hsla; 5] {
    match look {
        Look::Primary => [
            gpui::transparent_black(),
            a.token("confirm-btn-primary-bg"),
            a.token("confirm-btn-primary-hover"),
            a.token("confirm-btn-primary-pressed"),
            a.token("confirm-btn-primary-text"),
        ],
        Look::Secondary => [
            a.token("confirm-btn-secondary-border"),
            gpui::transparent_black(),
            a.token("confirm-btn-secondary-hover"),
            a.token("confirm-btn-secondary-hover"),
            a.token("confirm-btn-secondary-text"),
        ],
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Premultiplied, as CSS interpolates colors.
fn premultiplied(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let alpha = lerp(a[3], b[3], t);
    let channel = |i: usize| match alpha == 0.0 {
        true => 0.0,
        false => lerp(a[i] * a[3], b[i] * b[3], t) / alpha,
    };
    [channel(0), channel(1), channel(2), alpha]
}

/// Between legacy colors: sRGB.
pub fn mix_srgb(a: Hsla, b: Hsla, t: f32) -> Hsla {
    let (a, b): (Rgba, Rgba) = (a.into(), b.into());
    let [r, g, b, a] = premultiplied([a.r, a.g, a.b, a.a], [b.r, b.g, b.b, b.a], t);
    Rgba { r, g, b, a }.into()
}

/// With an `oklch()` token on either side: Oklab.
pub fn mix_oklab(a: Hsla, b: Hsla, t: f32) -> Hsla {
    let lab = |c: Hsla| {
        let rgba: Rgba = c.into();
        let (l, chroma, h) = oklch::from_srgb(rgba);
        let h = h.to_radians();
        [l, chroma * h.cos(), chroma * h.sin(), rgba.a]
    };
    let [l, ok_a, ok_b, alpha] = premultiplied(lab(a), lab(b), t);
    oklch::to_srgb(l, ok_a.hypot(ok_b), ok_b.atan2(ok_a).to_degrees(), alpha).into()
}

/// `cindy-button-frame`: a face 1px past the box, pressed back to it.
pub fn button(
    id: impl Into<ElementId>,
    (label, look): (SharedString, Look),
    (focus, disabled): (&FocusHandle, bool),
    on_click: Run,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let id: ElementId = id.into();
    let name = id.to_string();
    let now = cx.background_executor().now();
    let appearance = cx.global::<Appearance>();
    let [border, rest, hover, press, fg] = tones(look, appearance);
    let ring = BoxShadow {
        color: appearance.token("focus-ring"),
        offset: point(px(0.0), px(0.0)),
        blur_radius: px(0.0),
        spread_radius: window_px(2.0, cx),
        inset: false,
    };
    let face = window.use_keyed_state(
        ElementId::NamedChild(std::sync::Arc::new(id.clone()), "face".into()),
        cx,
        |_, _| Face {
            hovered: false,
            mouse: false,
            key: false,
            fill: Transition::still(rest, now),
            inset: Transition::still(-1.0, now),
        },
    );
    let focused = focus.is_focused(window);
    let f = face.read(cx);
    let pressed = !disabled && (f.mouse || (f.key && focused));
    let hovered = !disabled && f.hovered;
    let reduced = cx.global::<ReducedMotion>().0;
    // `--motion-instant` and `--motion-fast`; the inset stays put under reduced motion.
    let (instant, fast) = (
        motion::length(INSTANT, cx),
        motion::length(motion::FAST, cx),
    );
    let fill_to = match (pressed, hovered) {
        (true, _) => press,
        (_, true) => hover,
        _ => rest,
    };
    let inset_to = if pressed && !reduced { 0.0 } else { -1.0 };
    let mix = match look {
        Look::Primary => mix_oklab,
        Look::Secondary => mix_srgb,
    };
    let ((fill, filling), (outset, moving)) = face.update(cx, |f, _| {
        // Blur cancels a Space press.
        f.key &= focused;
        f.fill.toward(fill_to, (now, fast), motion::ease, mix);
        f.inset
            .toward(inset_to, (now, instant), motion::motion_ease_out, lerp);
        (
            f.fill.at(now, motion::ease, mix),
            f.inset.at(now, motion::motion_ease_out, lerp),
        )
    });
    if filling || moving {
        window.request_animation_frame();
    }
    let (enter, down, up, out) = (face.clone(), face.clone(), face.clone(), face.clone());
    let (key_down, key_up) = (face.clone(), face);
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label.clone())
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .min_w(css(96.0))
        .max_w_full()
        .min_h(css(36.0))
        .px(css(24.0))
        .py(css(6.0))
        .border(css(1.0))
        .border_color(gpui::transparent_black())
        .rounded(css(9999.0))
        .text_size(css(text_size(13.0, cx)))
        .line_height(relative(1.5))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(fg)
        .text_center()
        .hover_listener_mode(gpui::HoverListenerMode::InputModalityIndependent)
        .on_hover(move |h, _, cx| {
            enter.update(cx, |f, cx| {
                f.hovered = *h;
                cx.notify();
            })
        })
        .child(
            div()
                .debug_selector(move || format!("{name}-face"))
                .absolute()
                .inset(css(outset))
                .rounded(css(9999.0))
                .border(css(1.0))
                .border_color(border)
                .bg(fill),
        )
        .child(div().relative().child(label))
        .map(|el| {
            if disabled {
                return el.opacity(0.6);
            }
            el.track_focus(focus)
                .cursor_pointer()
                .focus_visible(|s| s.shadow(vec![ring]))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    down.update(cx, |f, cx| {
                        f.mouse = true;
                        cx.notify();
                    })
                })
                .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                    up.update(cx, |f, cx| {
                        f.mouse = false;
                        cx.notify();
                    })
                })
                .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
                    out.update(cx, |f, cx| {
                        f.mouse = false;
                        cx.notify();
                    })
                })
                .on_key_down(move |event, _, cx| {
                    if is_space(&event.keystroke) {
                        key_down.update(cx, |f, cx| {
                            f.key = true;
                            cx.notify();
                        })
                    }
                })
                .on_key_up(move |event, _, cx| {
                    if is_space(&event.keystroke) {
                        key_up.update(cx, |f, cx| {
                            f.key = false;
                            cx.notify();
                        })
                    }
                })
                .on_click(move |_, window, cx| on_click(window, cx))
        })
}
