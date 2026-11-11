//! A native button's `:hover` and `:active`: hover ignores the input, Space presses while focused.

use gpui::{
    App, ClickEvent, Div, ElementId, Entity, FocusHandle, InteractiveElement as _, KeyboardButton,
    MouseButton, Stateful, StatefulInteractiveElement as _, Window, prelude::FluentBuilder as _,
};

#[derive(Default)]
pub struct Press {
    hovered: bool,
    mouse: bool,
    /// Space held while focused.
    key: bool,
}

/// This frame's `:hover` and `:active`; a disabled button has neither.
#[derive(Clone, Copy)]
pub struct State {
    pub hovered: bool,
    pub pressed: bool,
}

/// Space presses a native button; Enter clicks it without `:active`.
fn is_space(keystroke: &gpui::Keystroke) -> bool {
    keystroke.key == "space" && !keystroke.modifiers.modified()
}

/// The press kept for `id`; blur cancels a Space press.
pub fn state(
    id: &ElementId,
    (focus, disabled): (&FocusHandle, bool),
    window: &mut Window,
    cx: &mut App,
) -> (Entity<Press>, State) {
    let key = ElementId::NamedChild(std::sync::Arc::new(id.clone()), "press".into());
    let press = window.use_keyed_state(key, cx, |_, _| Press::default());
    let focused = focus.is_focused(window);
    let state = press.update(cx, |p, _| {
        p.key &= focused;
        State {
            hovered: !disabled && p.hovered,
            pressed: !disabled && (p.mouse || p.key),
        }
    });
    (press, state)
}

fn set(press: &Entity<Press>, cx: &mut App, f: impl FnOnce(&mut Press)) {
    press.update(cx, |p, cx| {
        f(p);
        cx.notify();
    });
}

/// The listeners behind `state`; a disabled button tracks hover alone.
pub fn listen(el: Stateful<Div>, press: &Entity<Press>, disabled: bool) -> Stateful<Div> {
    let (hover, down, up, out) = (press.clone(), press.clone(), press.clone(), press.clone());
    let (key_down, key_up) = (press.clone(), press.clone());
    el.hover_listener_mode(gpui::HoverListenerMode::InputModalityIndependent)
        .on_hover(move |h, _, cx| set(&hover, cx, |p| p.hovered = *h))
        .when(!disabled, |el| {
            el.on_mouse_down(MouseButton::Left, move |_, _, cx| {
                set(&down, cx, |p| p.mouse = true)
            })
            .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                set(&up, cx, |p| p.mouse = false)
            })
            .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
                set(&out, cx, |p| p.mouse = false)
            })
            .on_key_down(move |event, _, cx| {
                if is_space(&event.keystroke) {
                    set(&key_down, cx, |p| p.key = true)
                }
            })
            .on_key_up(move |event, _, cx| {
                if is_space(&event.keystroke) {
                    set(&key_up, cx, |p| p.key = false)
                }
            })
        })
}

/// A Space held on a button: it clicks when it lifts, unless something ended it.
#[derive(Clone, Copy, Default)]
struct Space {
    held: bool,
    /// Another key or a modifier came with it, so gpui's own key-up click will not come.
    mixed: bool,
}

/// A native button's clicks: the mouse, Enter on key down, Space as it lifts, whatever they hold.
pub fn clicks(
    el: Stateful<Div>,
    (id, focus): (&ElementId, &FocusHandle),
    open: impl Fn(&mut Window, &mut App) + Clone + 'static,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let key = ElementId::NamedChild(std::sync::Arc::new(id.clone()), "space".into());
    let space = window.use_keyed_state(key, cx, |_, _| Space::default());
    if !focus.is_focused(window) {
        space.update(cx, |space, _| *space = Space::default());
    }
    // Every activation ends a held Space.
    let open = {
        let space = space.clone();
        move |window: &mut Window, cx: &mut App| {
            space.update(cx, |space, _| *space = Space::default());
            open(window, cx)
        }
    };
    let (clicked, entered, released) = (open.clone(), open.clone(), open);
    let (down, up, clean) = (space.clone(), space.clone(), space);
    // gpui's key-up click tells a clean Space from one the focus left meanwhile.
    el.on_click(move |event, window, cx| {
        let fresh = match event {
            ClickEvent::Mouse(_) => true,
            ClickEvent::Keyboard(key) => key.button == KeyboardButton::Space && clean.read(cx).held,
            _ => false,
        };
        if fresh {
            clicked(window, cx);
        }
    })
    .on_key_down(move |event, window, cx| {
        let stroke = &event.keystroke;
        match stroke.key.as_str() {
            "enter" => entered(window, cx),
            "space" => down.update(cx, |space, _| {
                space.mixed |= stroke.modifiers.modified();
                space.held = true;
            }),
            _ => down.update(cx, |space, _| space.mixed = space.held),
        }
    })
    .on_key_up(move |event, window, cx| {
        let stroke = &event.keystroke;
        let Space { held, mixed } = *up.read(cx);
        if stroke.key != "space" {
            return up.update(cx, |space, _| space.mixed = space.held);
        }
        if held && (mixed || stroke.modifiers.modified()) {
            return released(window, cx);
        }
        // The clean release is gpui's click, in this same dispatch.
        let up = up.clone();
        cx.defer(move |cx| up.update(cx, |space, _| *space = Space::default()));
    })
}
