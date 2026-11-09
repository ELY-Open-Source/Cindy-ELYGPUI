//! `Button` at `size="lg"` in the `confirmation` palette, as `ConfirmDialog` draws its actions.

use std::rc::Rc;

use gpui::{
    App, BoxShadow, Div, ElementId, FocusHandle, Hsla, InteractiveElement as _, ParentElement as _,
    Role, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, Window, div, point,
    prelude::FluentBuilder as _, px, relative,
};

use crate::{
    appearance_settings::text_size,
    motion::{self, ReducedMotion},
    press,
    theme::Appearance,
    transition::{self, Curve},
    units::{css, window_px},
};

/// `variant="primary"` or `"secondary"`.
#[derive(Clone, Copy, PartialEq)]
pub enum Look {
    Primary,
    Secondary,
}

pub type Run = Rc<dyn Fn(&mut Window, &mut App)>;

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
    let appearance = cx.global::<Appearance>();
    let [border, rest, hover, press_fill, fg] = tones(look, appearance);
    let ring = BoxShadow {
        color: appearance.token("focus-ring"),
        offset: point(px(0.0), px(0.0)),
        blur_radius: px(0.0),
        spread_radius: window_px(2.0, cx),
        inset: false,
    };
    let (press, state) = press::state(&id, (focus, disabled), window, cx);
    let reduced = cx.global::<ReducedMotion>().0;
    let fill_to = match (state.pressed, state.hovered) {
        (true, _) => press_fill,
        (_, true) => hover,
        _ => rest,
    };
    // The inset stays put under reduced motion.
    let inset_to = if state.pressed && !reduced { 0.0 } else { -1.0 };
    let mix = match look {
        Look::Primary => transition::mix_oklab,
        Look::Secondary => transition::mix_srgb,
    };
    let fill = transition::keyed(
        child(&id, "fill"),
        fill_to,
        Curve {
            length: motion::length(motion::FAST, cx),
            easing: motion::ease,
            lerp: mix,
        },
        window,
        cx,
    );
    let outset = transition::keyed(
        child(&id, "inset"),
        inset_to,
        Curve {
            length: motion::length(motion::INSTANT, cx),
            easing: motion::motion_ease_out,
            lerp: transition::lerp,
        },
        window,
        cx,
    );
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
        .map(|el| press::listen(el, &press, disabled))
        .map(|el| {
            if disabled {
                return el.opacity(0.6);
            }
            el.track_focus(focus)
                .cursor_pointer()
                .focus_visible(|s| s.shadow(vec![ring]))
                .on_click(move |_, window, cx| on_click(window, cx))
        })
}

fn child(id: &ElementId, part: &'static str) -> ElementId {
    ElementId::NamedChild(std::sync::Arc::new(id.clone()), part.into())
}
