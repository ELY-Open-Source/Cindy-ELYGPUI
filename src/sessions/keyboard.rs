//! The task as a Tab stop (`role="button" tabIndex={0}`): Enter and Space open it, and the
//! context-menu key opens its menu where Chromium's `ShowNonLocatedContextMenu` puts it, the
//! centre of the focused element's visible bounds.

use std::{cell::Cell, rc::Rc};

use ely_gpui_component::menus::ContextMenu;
use gpui::{
    App, Bounds, Div, ElementId, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    Keystroke, MouseButton, ParentElement as _, Pixels, Point, RenderOnce, Role, Stateful,
    StatefulInteractiveElement as _, Styled as _, Window, canvas,
};

use super::{Sessions, group::Session, open, reveal, select, task_menu};
use crate::{i18n::Translator, router};

/// An element's visible bounds, as its last prepaint saw them.
pub(super) type Probe = Rc<Cell<Option<Bounds<Pixels>>>>;

/// Records the parent's bounds, clipped as `VisibleBoundsInLocalRoot` clips them.
pub(super) fn probe(slot: Probe) -> impl IntoElement {
    canvas(
        move |b, window, _| slot.set(Some(b.intersect(&window.content_mask().bounds))),
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
}

/// The row's probe: it counts the row among those drawn and reveals it when asked.
fn row_probe(id: String, slot: Probe) -> impl IntoElement {
    canvas(
        move |b, window, cx| {
            slot.set(Some(b.intersect(&window.content_mask().bounds)));
            select::drawn(&id, cx);
            reveal::painted(&id, b, window, cx);
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
}

/// `is_unmodified_menu_key`, on key up on Windows, or `is_shift_f10` off macOS, on key down.
fn menu_key(k: &Keystroke, up: bool) -> bool {
    let m = &k.modifiers;
    let others = m.control || m.alt || m.platform;
    match k.key.as_str() {
        "menu" => !others && !m.shift && up == cfg!(target_os = "windows"),
        "f10" => !cfg!(target_os = "macos") && !others && m.shift && !up,
        _ => false,
    }
}

/// The task menu's host: a right click opens it at the pointer, the menu key at the focused
/// element. The wrapper stays while editing, so a press that commits a rename still clicks.
#[derive(IntoElement)]
pub(super) struct Host {
    session: Session,
    kind: &'static str,
    body: Stateful<Div>,
    more: Probe,
}

/// `kind` keys the look, as Cindy mounts one component per look; `more` is More's probe.
pub(super) fn with_menu(s: &Session, kind: &'static str, body: Stateful<Div>, more: Probe) -> Host {
    Host {
        session: s.clone(),
        kind,
        body,
        more,
    }
}

/// `setMenuPos`: each ask opens the menu once.
fn ask(asked: &Entity<Option<(u64, Point<Pixels>)>>, at: Point<Pixels>, cx: &mut App) {
    asked.update(cx, |asked, cx| {
        let number = asked.map_or(1, |(n, _)| n + 1);
        *asked = Some((number, at));
        cx.notify();
    });
}

struct Keys {
    id: String,
    focus: FocusHandle,
    row: Probe,
    more: Probe,
    asked: Entity<Option<(u64, Point<Pixels>)>>,
}

impl Keys {
    fn pressed(&self, k: &Keystroke, up: bool, window: &mut Window, cx: &mut App) {
        let editing = cx
            .global::<Sessions>()
            .editing
            .as_ref()
            .is_some_and(|e| e.id == self.id);
        let own = self.focus.is_focused(window);
        if !up && matches!(k.key.as_str(), "enter" | "space") {
            // `e.target !== e.currentTarget`: a key on More is More's own.
            if own && !editing {
                log::info!(target: "sessions", "task {} opened from the keyboard", self.id);
                open::open(&self.id, cx);
            }
            return;
        }
        if editing || !menu_key(k, up) {
            return;
        }
        cx.stop_propagation();
        let probe = if own { &self.row } else { &self.more };
        let Some(bounds) = probe.get() else {
            return log::warn!(target: "sessions", "task {} menu key: no bounds drawn", self.id);
        };
        log::info!(target: "sessions", "task {} menu from the keyboard", self.id);
        ask(&self.asked, bounds.center(), cx);
    }
}

impl RenderOnce for Host {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = self.session.id.clone();
        let key = |part: &str| ElementId::Name(format!("task-{part}-{id}").into());
        let focus = window
            .use_keyed_state(key("focus"), cx, |_, cx| cx.focus_handle().tab_stop(true))
            .read(cx)
            .clone();
        let asked = window.use_keyed_state(key("menu-asked"), cx, |_, _| None);
        let request = *asked.read(cx);
        let active = router::active_session(cx) == Some(id.as_str());
        let was =
            window.use_keyed_state(key(&format!("was-active-{}", self.kind)), cx, |_, _| false);
        if active && !*was.read(cx) {
            reveal::activated(&id, cx);
        }
        was.update(cx, |was, _| *was = active);
        let keys = Rc::new(Keys {
            id: id.clone(),
            focus: focus.clone(),
            row: Rc::default(),
            more: self.more,
            asked: asked.clone(),
        });
        let (down, up) = (keys.clone(), keys.clone());
        let body = self
            .body
            .relative()
            .track_focus(&focus)
            .role(Role::Button)
            .on_key_down(move |e, window, cx| down.pressed(&e.keystroke, false, window, cx))
            .on_key_up(move |e, window, cx| up.pressed(&e.keystroke, true, window, cx))
            .on_mouse_down(MouseButton::Right, move |e, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
                ask(&asked, e.position, cx);
            })
            .child(row_probe(id.clone(), keys.row.clone()));
        ContextMenu::new(
            key("menu"),
            task_menu::menu(&self.session, cx.global::<Translator>()),
        )
        .manual(request)
        .w_full()
        .child(body)
    }
}

#[cfg(test)]
mod tests;
