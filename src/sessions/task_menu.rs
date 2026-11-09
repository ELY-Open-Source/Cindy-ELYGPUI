//! `SessionTaskMenu` as far as this chapter writes, and `SessionAction`'s More button.

use ely_gpui_component::{
    menus::{Menu, MenuItem, menu_under},
    primitives::IconName,
};
use gpui::{
    App, ElementId, FocusHandle, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, RenderOnce, Role, StatefulInteractiveElement as _, Styled as _, Window,
    div, prelude::FluentBuilder as _,
};

use super::{Sessions, group::Session, pin, rename, title, write};
use crate::{i18n::Translator, lucide::icon, theme::Appearance, tip::tip, units::css};

fn session(id: &str, cx: &App) -> Option<Session> {
    let loaded = cx.global::<Sessions>().loaded.as_ref()?;
    loaded.sessions.iter().find(|s| s.id == id).cloned()
}

/// Tags, move, share, export, a new window, archive and delete come with the chapters that
/// own them.
pub fn menu(s: &Session, t: &Translator) -> Menu {
    let item = |key: &str| MenuItem::new(t.t(&format!("ccAgent.sidebar.sessionMenu.{key}"), &[]));
    let id = s.id.clone();
    let rename = item("rename").on_click(move |window, cx| match session(&id, cx) {
        Some(s) => rename::start(&s, window, cx),
        None => log::warn!(target: "sessions", "rename {id}: the task left the list"),
    });
    let mut menu = Menu::new();
    if s.status != "archived" && !title::empty_draft(s) {
        let (id, pinned) = (s.id.clone(), s.pinned_at.is_some());
        let key = if pinned { "unpin" } else { "pin" };
        menu =
            menu.item(item(key).on_click(move |window, cx| pin::toggle(&id, pinned, window, cx)));
    }
    menu = menu.item(rename);
    if s.status != "archived" && !title::empty_draft(s) {
        let (id, muted) = (s.id.clone(), s.muted);
        let key = if muted {
            "unmuteNotifications"
        } else {
            "muteNotifications"
        };
        menu = menu.item(item(key).on_click(move |window, cx| {
            write::apply(&id, write::Meta::Muted(!muted), window, cx);
        }));
    }
    let id = s.id.clone();
    menu.separator()
        .item(item("copySessionLink").on_click(move |window, cx| write::copy_link(&id, window, cx)))
}

/// The hover More button: `size-5 rounded-md`, `EllipsisVertical` 14/2; on a card, `CardAction`'s
/// bordered `size-6 rounded-[7px]` at 13/2.
#[derive(IntoElement)]
pub struct More {
    pub id: String,
    pub menu: Menu,
    /// A Tab stop; Enter or Space opens the menu.
    pub focus: FocusHandle,
    pub card: bool,
}

impl RenderOnce for More {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let appearance = cx.global::<Appearance>();
        let card = self.card;
        let (color, hover_bg, hover_fg) = (
            if card {
                appearance.token("text-tertiary")
            } else {
                appearance.hsl_token("sidebar-action-icon")
            },
            appearance.hsl_token("sidebar-item-hover"),
            appearance.hsl_token("foreground"),
        );
        let (card_bg, card_border) = (
            appearance.token("cmd-palette-bg"),
            appearance.hsl_token("sidebar-border"),
        );
        let label = cx
            .global::<Translator>()
            .t("ccAgent.sidebar.sessionMenu.moreActions", &[]);
        let selector = format!("task-more-{}", self.id);
        let focus = self.focus;
        menu_under(
            ElementId::Name(format!("task-more-menu-{}", self.id).into()),
            &self.menu,
            move |click| {
                div()
                    .id(ElementId::Name(selector.clone().into()))
                    .debug_selector(move || selector)
                    .track_focus(&focus)
                    .role(Role::Button)
                    .aria_label(label.clone())
                    .group("task-more")
                    .flex()
                    .flex_none()
                    .size(css(if card { 24.0 } else { 20.0 }))
                    .items_center()
                    .justify_center()
                    .rounded(css(if card { 7.0 } else { 6.0 }))
                    .when(card, |el| {
                        el.bg(card_bg).border(css(1.0)).border_color(card_border)
                    })
                    .hover(|s| s.bg(hover_bg))
                    .tooltip(tip(label))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(move |event, window, cx| {
                        cx.stop_propagation();
                        click(event, window, cx);
                    })
                    .child(
                        icon(
                            IconName::EllipsisVertical.svg(),
                            if card { 13.0 } else { 14.0 },
                            2.0,
                        )
                        .text_color(color)
                        .group_hover("task-more", |s| s.text_color(hover_fg)),
                    )
                    .into_any_element()
            },
            window,
            cx,
        )
    }
}
