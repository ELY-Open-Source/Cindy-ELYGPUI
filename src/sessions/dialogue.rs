//! `SessionGroupNode`'s dialogues header with `DialogueGroupMenu`; Archive all comes with 18.0.

use ely_gpui_component::menus::{ContextMenu, Menu, MenuItem};
use gpui::{
    App, ClickEvent, Entity, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _,
    Pixels, Point, RenderOnce, Role, StatefulInteractiveElement as _, Styled as _, Window, point,
};

use super::{
    keyboard::{self, Probe},
    main_list::{Name, Toggle, header},
    search::lock::{self, Lock},
};
use crate::{i18n::Translator, lucide::MESSAGES_SQUARE};

const ID: &str = "dialogue:all-header";

/// The dialogues' header; `ids` are the tasks its Search keeps to.
#[derive(IntoElement)]
pub struct Head {
    pub collapsed: bool,
    pub toggle: Toggle,
    pub ids: Vec<String>,
}

type Asked = Entity<Option<(u64, Point<Pixels>)>>;

fn menu(ids: Vec<String>, cx: &App) -> Menu {
    let t = cx.global::<Translator>();
    let name = t.t("ccAgent.sidebar.dialogues", &[]);
    let item = MenuItem::new(t.t("ccAgent.sidebar.dialogueAction.search", &[]));
    Menu::new().item(item.on_click(move |window, cx| {
        let scope = Lock::Group {
            name: name.clone(),
            ids: ids.clone(),
        };
        lock::request(scope, window, cx)
    }))
}

/// The menu key or Shift+F10 opens the menu under the header's left edge.
fn menu_key(k: &gpui::Keystroke) -> bool {
    k.key == "menu" || (k.key == "f10" && k.modifiers.shift)
}

impl RenderOnce for Head {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = window
            .use_keyed_state("dialogue-header-focus", cx, |_, cx| {
                cx.focus_handle().tab_stop(true)
            })
            .read(cx)
            .clone();
        let asked: Asked = window.use_keyed_state("dialogue-menu-asked", cx, |_, _| None);
        let hovered = window.use_keyed_state("dialogue-header-hover", cx, |_, _| false);
        let (lit, request) = (*hovered.read(cx), *asked.read(cx));
        let name = cx
            .global::<Translator>()
            .t("ccAgent.sidebar.dialogues", &[]);
        let probe = Probe::default();
        let (toggle, keys, at, right) = (
            self.toggle.clone(),
            self.toggle.clone(),
            probe.clone(),
            asked.clone(),
        );
        let head = header(
            ID.into(),
            MESSAGES_SQUARE,
            (Name::Text(name), Some(lit)),
            self.collapsed,
            cx,
        )
        .hover_listener_mode(gpui::HoverListenerMode::InputModalityIndependent)
        .on_hover(move |h, _, cx| {
            hovered.update(cx, |hovered, cx| {
                *hovered = *h;
                cx.notify();
            })
        })
        .track_focus(&focus)
        .role(Role::Button)
        .aria_expanded(!self.collapsed)
        // Keys fold on key down; gpui's key-up click is not Cindy's.
        .on_click(move |event, _, cx| {
            if matches!(event, ClickEvent::Mouse(_)) {
                toggle(cx);
            }
        })
        .on_key_down(move |e, window, cx| {
            let k = &e.keystroke;
            if menu_key(k) {
                window.prevent_default();
                cx.stop_propagation();
                let Some(b) = at.get() else {
                    return log::warn!(target: "sessions", "dialogue menu key: no bounds drawn");
                };
                log::info!(target: "sessions", "dialogue menu from the keyboard");
                keyboard::ask(&asked, point(b.origin.x, b.bottom()), cx);
            } else if matches!(k.key.as_str(), "enter" | "space") {
                keys(cx);
            }
        })
        .on_mouse_down(MouseButton::Right, move |e, window, cx| {
            window.prevent_default();
            cx.stop_propagation();
            keyboard::ask(&right, e.position, cx);
        })
        .child(keyboard::probe(probe));
        ContextMenu::new("dialogue-menu", menu(self.ids, cx))
            .manual(request)
            .w_full()
            .child(head)
    }
}

#[cfg(test)]
mod tests;
