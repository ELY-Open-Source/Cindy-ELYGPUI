//! `SidebarNavigationCustomize`: the draft over the built-in entries, in place of the rows.

use std::rc::Rc;

use ely_gpui_component::{
    buttons::{Button, ButtonVariant},
    primitives::IconName,
    theme::ControlSize,
};
use gpui::{
    AnyElement, App, BoxShadow, ElementId, FocusHandle, InteractiveElement as _, IntoElement,
    MouseButton, ParentElement as _, Role, StatefulInteractiveElement as _, Styled as _, Window,
    div, point, prelude::FluentBuilder as _, px,
};

use super::more::More;
use crate::{
    appearance_settings::text_size,
    i18n::{Arg, Translator},
    lucide::icon,
    sessions::sortable,
    theme::Appearance,
    tip::tip,
    units::{css, window_px},
};

/// The panel's sortable list.
const LIST: &str = "nav-customize-list";

/// `SIDEBAR_NAVIGATION_ITEMS`, which `DEFAULT_SIDEBAR_NAVIGATION_VISIBLE` also lists.
const ITEMS: [&str; 3] = ["automations", "plugins", "search"];

/// `CustomizeDraft`; plugin main views join it with 16.0.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft {
    order: Vec<&'static str>,
    visible: Vec<&'static str>,
}

impl Default for Draft {
    /// `getSidebarNavigationPrefs(null)`: the defaults.
    fn default() -> Self {
        Draft {
            order: ITEMS.to_vec(),
            visible: ITEMS.to_vec(),
        }
    }
}

impl Draft {
    fn toggle(&mut self, id: &'static str) {
        match self.visible.contains(&id) {
            true => self.visible.retain(|v| *v != id),
            false => self.visible.push(id),
        }
    }

    /// `move`: the source takes the target's index.
    fn move_to(&mut self, source: &str, target: &str) {
        let (Some(from), Some(to)) = (self.index(source), self.index(target)) else {
            return;
        };
        if from != to {
            let id = self.order.remove(from);
            self.order.insert(to, id);
        }
    }

    /// `moveBy`: one place up or down, if there is one.
    fn move_by(&mut self, id: &str, down: bool) {
        let Some(at) = self.index(id) else { return };
        let target = match down {
            true => self.order.get(at + 1),
            false => at.checked_sub(1).and_then(|i| self.order.get(i)),
        };
        if let Some(target) = target.copied() {
            self.move_to(id, target);
        }
    }

    fn reorder(&mut self, ids: &[String]) {
        let order: Vec<&'static str> = ids
            .iter()
            .filter_map(|id| ITEMS.iter().find(|i| **i == id.as_str()).copied())
            .collect();
        if order.len() == self.order.len() {
            self.order = order;
        }
    }

    fn index(&self, id: &str) -> Option<usize> {
        self.order.iter().position(|i| *i == id)
    }
}

fn draft(cx: &mut App) -> Option<&mut Draft> {
    cx.global_mut::<More>().customize.as_mut()
}

fn edit(cx: &mut App, change: impl FnOnce(&mut Draft)) {
    if let Some(d) = draft(cx) {
        change(d);
        log::info!(target: "sidebar", "customize draft: order {:?}, visible {:?}", d.order, d.visible);
    }
    cx.refresh_windows();
}

/// `save`: `setSidebarNavigationPrefs(null, ..)` stores nothing; the data owner comes with 13.0.
fn done(cx: &mut App) {
    let d = cx.global_mut::<More>().customize.take();
    log::info!(target: "sidebar", "customize done, not saved without a data owner: {d:?}");
    cx.refresh_windows();
}

fn icon_of(id: &str) -> IconName {
    match id {
        "automations" => IconName::Timer,
        // Ely has no lucide `plug`.
        "plugins" => IconName::Puzzle,
        _ => IconName::Search,
    }
}

/// `focus-visible:ring-2 ring-[var(--focus-ring)]`.
fn ring(cx: &App) -> BoxShadow {
    BoxShadow {
        color: cx.global::<Appearance>().token("focus-ring"),
        offset: point(px(0.0), px(0.0)),
        blur_radius: px(0.0),
        spread_radius: window_px(2.0, cx),
        inset: false,
    }
}

fn handle(key: String, window: &mut Window, cx: &mut App) -> FocusHandle {
    window
        .use_keyed_state(ElementId::Name(key.into()), cx, |_, cx| {
            cx.focus_handle().tab_stop(true)
        })
        .read(cx)
        .clone()
}

/// The panel, a direct child of the scroller: `mx-2 mb-2 rounded-xl border p-3 shadow-menu`.
pub fn panel(window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let d = cx.try_global::<More>()?.customize.clone()?;
    let t = cx.global::<Translator>();
    let (title, done_label, reset_label) = (
        t.t("sidebar.navigation.customize.title", &[]),
        t.t("sidebar.navigation.customize.done", &[]),
        t.t("sidebar.navigation.customize.reset", &[]),
    );
    let appearance = cx.global::<Appearance>();
    let (border, bg, fg) = (
        appearance.hsl_token("sidebar-border"),
        appearance.token("surface-elevated"),
        appearance.token("text-primary"),
    );
    let shadow = appearance.shadow_token("shadow-menu", cx);
    let size = text_size(14.0, cx);
    let among = Rc::new(sortable::Among {
        list: LIST,
        ids: Rc::new(d.order.iter().map(|i| (*i).to_owned()).collect()),
        columns: 1,
    });
    // The drag's live order, so the other rows make way.
    let shown = sortable::arrange(LIST, d.order.clone(), 1, |i| (*i).to_owned(), cx);
    let rows = shown
        .into_iter()
        .flatten()
        .map(|id| {
            let draw: sortable::Draw = Rc::new(move |window, cx| row(id, window, cx));
            let child = row(id, window, cx);
            sortable::item(id, among.clone(), true, child, draw, cx)
        })
        .collect();
    let on_reorder: sortable::OnReorder = Rc::new(|ids, _, cx| edit(cx, |d| d.reorder(ids)));
    let header = div()
        .mb(css(8.0))
        .flex()
        .items_center()
        .justify_between()
        .gap(css(8.0))
        .px(css(4.0))
        .child(
            div()
                .id("nav-customize-title")
                .role(Role::Heading)
                .text_size(css(size))
                .line_height(css(20.0))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(fg)
                .child(title.clone()),
        )
        .child(
            div().debug_selector(|| "nav-customize-done".into()).child(
                Button::new("nav-customize-done", done_label)
                    .variant(ButtonVariant::Subtle)
                    .size(ControlSize::Sm)
                    .on_click(|_, _, cx| done(cx)),
            ),
        );
    let reset = div().mt(css(8.0)).flex().child(
        div().debug_selector(|| "nav-customize-reset".into()).child(
            Button::new("nav-customize-reset", reset_label)
                .variant(ButtonVariant::Ghost)
                .size(ControlSize::Sm)
                .on_click(|_, _, cx| edit(cx, |d| *d = Draft::default())),
        ),
    );
    let list = sortable::list(LIST, rows, on_reorder)
        .flex()
        .flex_col()
        .gap(css(2.0));
    Some(
        div()
            .id("nav-customize")
            .debug_selector(|| "nav-customize".into())
            .role(Role::Dialog)
            .aria_label(title)
            .flex_none()
            .mx(css(8.0))
            .mb(css(8.0))
            .rounded(css(12.0))
            .border(css(1.0))
            .border_color(border)
            .bg(bg)
            .p(css(12.0))
            .shadow(vec![shadow])
            .child(header)
            .child(list)
            .child(reset)
            .into_any_element(),
    )
}

/// One entry: the whole row toggles; the checkbox and the grip are its focusable controls.
fn row(id: &'static str, window: &mut Window, cx: &mut App) -> AnyElement {
    let visible = cx
        .global::<More>()
        .customize
        .as_ref()
        .is_some_and(|d| d.visible.contains(&id));
    let t = cx.global::<Translator>();
    let label = t.t(&format!("sidebar.navigation.items.{id}"), &[]);
    let reorder = t.t(
        "sidebar.navigation.customize.reorder",
        &[("name", Arg::Text(&label))],
    );
    let check = handle(format!("nav-customize-check-{id}"), window, cx);
    let grip = handle(format!("nav-customize-grip-{id}"), window, cx);
    let ring = ring(cx);
    let appearance = cx.global::<Appearance>();
    let hover = appearance.hsl_token("sidebar-item-hover");
    let (on_bg, on_fg) = (
        appearance.token("accent-cta-bg-pure"),
        appearance.token("accent-pure-cta-fg"),
    );
    let (off, off_hover) = (
        appearance.token("border-default"),
        appearance.token("text-secondary"),
    );
    let (nav, fg, tertiary) = (
        appearance.token("sidebar-nav-text"),
        appearance.token("text-primary"),
        appearance.token("text-tertiary"),
    );
    let size = text_size(14.0, cx);
    let group = format!("nav-customize-row-{id}");
    let own = check.clone();
    let checkbox = div()
        .id(ElementId::Name(format!("nav-customize-check-{id}").into()))
        .track_focus(&check)
        .role(Role::CheckBox)
        .aria_toggled(visible.into())
        .aria_label(label.clone())
        .flex()
        .flex_none()
        .size(css(20.0))
        .items_center()
        .justify_center()
        .rounded(css(9999.0))
        .focus_visible(|s| s.shadow(vec![ring.clone()]))
        .map(|el| match visible {
            true => el
                .bg(on_bg)
                .child(icon(IconName::Check.svg(), 12.0, 2.5).text_color(on_fg)),
            false => el
                .border(css(1.0))
                .border_color(off)
                .group_hover(group.clone(), |s| s.border_color(off_hover)),
        })
        // `filter="[role='checkbox']"`: a press here neither drags nor reaches the row.
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            window.focus(&own, cx);
            cx.stop_propagation();
        })
        .on_click(move |_, _, cx| edit(cx, |d| d.toggle(id)));
    let handle_el = div()
        .id(ElementId::Name(format!("nav-customize-grip-{id}").into()))
        .track_focus(&grip)
        .role(Role::Button)
        .aria_label(reorder.clone())
        .aria_keyshortcuts("ArrowUp ArrowDown")
        .flex()
        .flex_none()
        .size(css(28.0))
        .items_center()
        .justify_center()
        .rounded(css(8.0))
        .cursor_grab()
        .focus_visible(|s| s.shadow(vec![ring.clone()]))
        .tooltip(tip(reorder))
        .on_click(|_, _, cx| cx.stop_propagation())
        .on_key_down(move |event, _, cx| {
            let down = match event.keystroke.key.as_str() {
                "up" => false,
                "down" => true,
                _ => return,
            };
            cx.stop_propagation();
            edit(cx, |d| d.move_by(id, down));
        })
        .child(icon(IconName::GripVertical.svg(), 16.0, 2.0).text_color(tertiary));
    div()
        .id(ElementId::Name(group.clone().into()))
        .debug_selector(move || format!("nav-customize-row-{id}"))
        .group(group)
        .flex()
        .h(css(36.0))
        .items_center()
        .gap(css(8.0))
        .rounded(css(8.0))
        .px(css(4.0))
        .cursor_pointer()
        .hover(|s| s.bg(hover))
        .on_click(move |_, _, cx| edit(cx, |d| d.toggle(id)))
        .child(checkbox)
        .child(icon(icon_of(id).svg(), 15.0, 1.8).text_color(nav))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .truncate()
                .text_size(css(size))
                .line_height(css(20.0))
                .text_color(fg)
                .child(label),
        )
        .child(handle_el)
        .into_any_element()
}

#[cfg(test)]
mod tests;
