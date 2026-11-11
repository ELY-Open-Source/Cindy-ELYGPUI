//! A resolved local target's faces: `FileTargetChip`'s box and `ResolvedLocalLink`.

use std::cell::Cell;

use gpui::{
    AnyElement, App, ClickEvent, Div, ElementId, Entity, FocusHandle, Hsla,
    InteractiveElement as _, IntoElement as _, MouseButton, Role, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, Window, prelude::FluentBuilder as _,
};

use crate::{
    local_path::shape,
    markdown_target::{self, Place},
    motion,
    theme::Appearance,
    theme_import::color::js_trim,
    tip,
    transition::{self, Curve},
};

/// How a resolved target draws: in the chip's box or as a link, with or without its icon.
#[derive(Clone, Copy)]
pub struct FileFace {
    pub boxed: bool,
    pub icon: Option<&'static [u8]>,
}

/// `utils.basename`.
fn basename(path: &str) -> &str {
    match path.rsplit(['\\', '/']).next() {
        Some(last) if !last.is_empty() => last,
        _ => path,
    }
}

/// `shouldRenderCodeReferenceLabel` for a resolved target: its label reads as a file reference.
pub fn code_label(href: &str, at: Place, label: &str) -> bool {
    let label = js_trim(label);
    if label.is_empty() || label.contains('\n') {
        return false;
    }
    let base = basename(href);
    let lined = at.line.map(|line| format!("{base}:{line}"));
    let placed = (lined.as_ref()).zip(at.column);
    let placed = placed.map(|(lined, column)| format!("{lined}:{column}"));
    if label == href || label == base || lined.as_deref() == Some(label) {
        return true;
    }
    if placed.as_deref() == Some(label) {
        return true;
    }
    let (path, _) = markdown_target::split_line(label);
    shape::file(path) || markdown_target::bare_file(path)
}

/// A mounted `FileTargetChip`: its fill, the row under the pointer, its `title` and Tab stop.
pub struct Chip {
    id: String,
    pub fill: Hsla,
    hover: Entity<Option<usize>>,
    title: SharedString,
    focus: FocusHandle,
    drawn: Cell<bool>,
}

/// The chip mounted as `id`; `transition-colors hover:bg-[var(--cmd-palette-item-hover)]`.
pub fn chip(
    id: &str,
    title: SharedString,
    focus: FocusHandle,
    window: &mut Window,
    cx: &mut App,
) -> Chip {
    let state = |name: &str| ElementId::Name(format!("{id}-{name}").into());
    let hover = window.use_keyed_state(state("hover"), cx, |_, _| None::<usize>);
    let appearance = cx.global::<Appearance>();
    let fill = appearance.token(match hover.read(cx) {
        Some(_) => "cmd-palette-item-hover",
        None => "msg-md-inline-code-bg",
    });
    let curve = Curve {
        length: motion::length(motion::FAST, cx),
        easing: motion::ease_move,
        lerp: transition::mix_srgb,
    };
    Chip {
        id: id.to_owned(),
        fill: transition::keyed(state("fill"), fill, curve, window, cx),
        hover,
        title,
        focus,
        drawn: Cell::new(false),
    }
}

/// `onOpen`: the lightboxes and the file browser come with 9.13.
fn open() {
    log::warn!(target: "sessionView", "a resolved file activated: nothing opens it before 9.13");
}

/// A box of a resolved target: its click, middle click and keys; the first box is the Tab stop.
pub fn listen(el: Stateful<Div>, first: bool, focus: &FocusHandle, role: Role) -> Stateful<Div> {
    let pressed = focus.clone();
    el.cursor_pointer()
        .when(first, |el| {
            el.track_focus(focus)
                .role(role)
                .on_key_down(|event, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        window.prevent_default();
                        cx.stop_propagation();
                        open();
                    }
                })
        })
        // The innermost target takes the press: a chip about it sees the default prevented.
        .on_any_mouse_down(move |_, window, cx| {
            if !window.default_prevented() {
                window.prevent_default();
                pressed.focus(window, cx);
            }
        })
        // Keys activate on key down above; gpui's key-up click is not Cindy's.
        .on_click(|event, _, cx| {
            if matches!(event, ClickEvent::Mouse(_)) {
                cx.stop_propagation();
                open();
            }
        })
        // The file's menu comes with 9.6h4d: a link about the chip opens none of its own.
        .on_mouse_down(MouseButton::Right, |_, window, cx| {
            window.prevent_default();
            cx.stop_propagation();
        })
        .on_aux_click(|event, _, cx| {
            let ClickEvent::Mouse(mouse) = event else {
                return;
            };
            if mouse.up.button == MouseButton::Middle {
                cx.stop_propagation();
                open();
            }
        })
}

impl Chip {
    /// The chip's box on row `row`: `role="button"`, its hover and its title.
    pub fn fragment(&self, el: Div, row: usize) -> AnyElement {
        let first = !self.drawn.replace(true);
        let id = ElementId::Name(format!("{}-r{row}", self.id).into());
        let (hover, title) = (self.hover.clone(), self.title.clone());
        listen(el.id(id), first, &self.focus, Role::Button)
            // Hover ignores key presses, as CSS `:hover` does.
            .hover_listener_mode(gpui::HoverListenerMode::InputModalityIndependent)
            .on_hover(move |over, _, cx| {
                hover.update(cx, |hover, cx| {
                    match (*over, *hover == Some(row)) {
                        (true, _) => *hover = Some(row),
                        (false, true) => *hover = None,
                        (false, false) => return,
                    }
                    cx.notify();
                })
            })
            .when(!title.is_empty(), |el| el.tooltip(tip::tip(title)))
            .into_any_element()
    }
}
