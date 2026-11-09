//! `TaskCheckbox` and the `label` around it: the box takes focus and Space, the label clicks it.

use std::rc::Rc;

use ely_gpui_component::primitives::IconName;
use gpui::{
    App, BoxShadow, ClickEvent, Div, ElementId, FocusHandle, InteractiveElement as _,
    KeyboardButton, ParentElement as _, Role, Stateful, StatefulInteractiveElement as _,
    Styled as _, Toggled, Window, div, point, prelude::FluentBuilder as _, px,
};

use crate::{
    lucide,
    theme::Appearance,
    units::{css, window_px},
};

pub type Toggle = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// A box's state; `Mixed` is `indeterminate`.
#[derive(Clone, Copy, PartialEq)]
pub enum Mark {
    Off,
    On,
    Mixed,
}

/// A native box clicks on Space, not Enter.
fn activates(event: &ClickEvent) -> bool {
    !matches!(
        event,
        ClickEvent::Keyboard(k) if k.button == KeyboardButton::Enter
    )
}

/// The box's Tab stop, kept per `key`.
pub fn focus(key: String, window: &mut Window, cx: &mut App) -> FocusHandle {
    window
        .use_keyed_state(ElementId::Name(key.into()), cx, |_, cx| {
            cx.focus_handle().tab_stop(true)
        })
        .read(cx)
        .clone()
}

/// `size-3.5` with `accent-[var(--confirm-btn-primary-bg)]`; box and label set one target.
pub fn checkbox(
    id: String,
    (mark, name): (Mark, String),
    focus: &FocusHandle,
    toggle: Toggle,
    cx: &App,
) -> Stateful<Div> {
    let appearance = cx.global::<Appearance>();
    let (accent, ink) = (
        appearance.token("confirm-btn-primary-bg"),
        appearance.token("confirm-btn-primary-text"),
    );
    let (border, fill) = (
        appearance.token("text-tertiary"),
        appearance.token("surface-elevated"),
    );
    let ring = BoxShadow {
        color: appearance.token("focus-ring"),
        offset: point(px(0.0), px(0.0)),
        blur_radius: px(0.0),
        spread_radius: window_px(2.0, cx),
        inset: false,
    };
    let glyph = match mark {
        Mark::On => Some(IconName::Check),
        Mark::Mixed => Some(IconName::Minus),
        Mark::Off => None,
    };
    let toggled = match mark {
        Mark::Off => Toggled::False,
        Mark::On => Toggled::True,
        Mark::Mixed => Toggled::Mixed,
    };
    let next = mark != Mark::On;
    let selector = id.clone();
    div()
        .id(ElementId::Name(id.into()))
        .debug_selector(move || selector.clone())
        .track_focus(focus)
        .role(Role::CheckBox)
        .aria_toggled(toggled)
        .aria_label(name)
        .flex()
        .flex_none()
        .size(css(14.0))
        .items_center()
        .justify_center()
        .rounded(css(2.0))
        .border(css(1.0))
        .cursor_pointer()
        .focus_visible(|s| s.shadow(vec![ring]))
        .map(|el| match glyph {
            Some(glyph) => el
                .border_color(accent)
                .bg(accent)
                .child(lucide::icon(glyph.svg(), 10.0, 3.0).text_color(ink)),
            None => el.border_color(border).bg(fill),
        })
        .on_click(move |event, window, cx| {
            if activates(event) {
                toggle(next, window, cx);
            }
        })
}

/// The `label`: a click focuses the box and clicks it; `hover` lights it whatever the input.
pub fn label(
    id: String,
    (box_focus, next, hover): (&FocusHandle, bool, bool),
    toggle: Toggle,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let hovered =
        window.use_keyed_state(ElementId::Name(format!("{id}-hover").into()), cx, |_, _| {
            false
        });
    let lit = *hovered.read(cx);
    let (selector, selector_hover) = (id.clone(), id.clone());
    let focus = box_focus.clone();
    div()
        .id(ElementId::Name(id.into()))
        .debug_selector(move || selector.clone())
        .cursor_pointer()
        .hover_listener_mode(gpui::HoverListenerMode::InputModalityIndependent)
        .on_hover(move |h, _, cx| {
            hovered.update(cx, |hovered, cx| {
                *hovered = *h;
                cx.notify();
            })
        })
        .relative()
        .when(hover && lit, |el| {
            let lit_selector = format!("{selector_hover}-hover");
            el.child(
                div()
                    .debug_selector(move || lit_selector.clone())
                    .absolute()
                    .inset_0()
                    .rounded(css(8.0))
                    .bg(cx.global::<Appearance>().token("surface-hover")),
            )
        })
        .on_click(move |_, window, cx| {
            window.focus(&focus, cx);
            toggle(next, window, cx);
        })
}
