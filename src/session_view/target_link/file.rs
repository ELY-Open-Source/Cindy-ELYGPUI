//! A resolved local target's faces: `FileTargetChip`'s box and `ResolvedLocalLink`.

use std::cell::Cell;

use gpui::{
    AnyElement, App, ClickEvent, Div, ElementId, Entity, FocusHandle, Hsla,
    InteractiveElement as _, IntoElement as _, MouseButton, ParentElement as _, Role, SharedString,
    Stateful, StatefulInteractiveElement as _, Styled as _, Window, canvas,
    prelude::FluentBuilder as _,
};

use super::file_menu;
use crate::{
    browser_file,
    local_path::shape,
    markdown_target::{self, Place},
    motion,
    session_view::links::{self, Asked, Spots, What},
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

/// A resolved local target: its path, whether a folder, the place named and the task's folder.
#[derive(Clone)]
pub struct Resolved {
    pub path: SharedString,
    pub folder: bool,
    pub at: Place,
    pub cwd: SharedString,
}

impl Resolved {
    /// `canOpenInBrowser`.
    pub fn browsable(&self) -> bool {
        !self.folder && browser_file::openable(&self.path)
    }
}

/// What a target's boxes ask its menu with: the file, the message's ask and the boxes drawn.
#[derive(Clone)]
pub struct Asks {
    pub file: Resolved,
    pub asked: Entity<Asked>,
    pub spots: Spots,
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
    asks: Asks,
    drawn: Cell<bool>,
}

/// The chip mounted as `id`; `transition-colors hover:bg-[var(--cmd-palette-item-hover)]`.
pub fn chip(
    id: &str,
    title: SharedString,
    focus: FocusHandle,
    asks: Asks,
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
        asks,
        drawn: Cell::new(false),
    }
}

/// A box of a resolved target: its clicks, keys and menu; the first box is the Tab stop.
pub fn listen(
    el: Stateful<Div>,
    first: bool,
    focus: &FocusHandle,
    role: Role,
    asks: &Asks,
) -> Stateful<Div> {
    let (pressed, back) = (focus.clone(), focus.clone());
    let (keys, left, middle, right) = (asks.clone(), asks.clone(), asks.clone(), asks.clone());
    el.cursor_pointer()
        .when(first, |el| el.track_focus(focus).role(role))
        // Every box hears the keys of a target it holds; Enter and Space bubble on, as Cindy's do.
        .on_key_down(move |event, window, cx| {
            let k = &event.keystroke;
            if k.key == "menu" || (k.key == "f10" && k.modifiers.shift) {
                window.prevent_default();
                cx.stop_propagation();
                let (what, under) = (What::File(keys.file.clone()), links::under(&keys.spots));
                links::ask(&keys.asked, what, under, cx);
            } else if matches!(k.key.as_str(), "enter" | "space") {
                window.prevent_default();
                let held = (k.modifiers, MouseButton::Left);
                file_menu::activate(&keys.file, held, window, cx);
            }
        })
        // The innermost target takes the press: a chip about it sees the default prevented.
        .on_any_mouse_down(move |_, window, cx| {
            if !window.default_prevented() {
                window.prevent_default();
                pressed.focus(window, cx);
            }
        })
        // Keys activate on key down above; gpui's key-up click is not Cindy's.
        .on_click(move |event, window, cx| {
            if matches!(event, ClickEvent::Mouse(_)) {
                cx.stop_propagation();
                let held = (event.modifiers(), MouseButton::Left);
                file_menu::activate(&left.file, held, window, cx);
            }
        })
        .on_mouse_down(MouseButton::Right, move |event, window, cx| {
            window.prevent_default();
            cx.stop_propagation();
            // The menu hands focus back to the target it was asked on.
            back.focus(window, cx);
            let what = What::File(right.file.clone());
            links::ask(&right.asked, what, event.position, cx);
        })
        .on_aux_click(move |event, window, cx| {
            let ClickEvent::Mouse(mouse) = event else {
                return;
            };
            if mouse.up.button == MouseButton::Middle {
                cx.stop_propagation();
                let held = (event.modifiers(), MouseButton::Middle);
                file_menu::activate(&middle.file, held, window, cx);
            }
        })
}

impl Chip {
    /// The chip's box on row `row`: `role="button"`, its hover and its title.
    pub fn fragment(&self, el: Div, row: usize) -> AnyElement {
        let first = !self.drawn.replace(true);
        let id = ElementId::Name(format!("{}-r{row}", self.id).into());
        let (hover, title) = (self.hover.clone(), self.title.clone());
        let spots = self.asks.spots.clone();
        let probe = canvas(
            move |bounds, _, _| spots.borrow_mut().push(bounds),
            |_, _, _, _| {},
        );
        let el = el.child(probe.absolute().inset_0());
        listen(el.id(id), first, &self.focus, Role::Button, &self.asks)
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
