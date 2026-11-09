//! `RailNav`'s Projects and Dialogues tiles over `railPanelStore`: which panel is open, where.

use std::time::Duration;

use gpui::{
    AnyElement, App, Bounds, ClickEvent, FocusHandle, Global, Hitbox, HitboxBehavior,
    HoverListenerMode, InteractiveElement as _, IntoElement, ParentElement as _, Pixels, Role,
    StatefulInteractiveElement as _, Styled as _, Task, Window, canvas, div,
    prelude::FluentBuilder as _,
};

use crate::{
    i18n::Translator, lucide, sessions::rail::pinned, theme::Appearance, tip::tip, units::css,
};

/// `RAIL_PANEL_CLOSE_GRACE_MS`.
const CLOSE_GRACE: Duration = Duration::from_millis(120);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Section {
    Projects,
    Dialogues,
}

impl Section {
    fn index(self) -> usize {
        self as usize
    }

    pub(super) fn key(self) -> &'static str {
        match self {
            Section::Projects => "projects",
            Section::Dialogues => "dialogues",
        }
    }
}

/// `railPanelStore`'s state; the project level and the lamp scope come with 8.7e3c2 and 10.0.
#[derive(Default)]
pub struct Panels {
    pub open: Option<Section>,
    /// The trigger's bounds when it opened; the panel takes its right and top.
    pub anchor: Option<Bounds<Pixels>>,
    /// `openedViaKeyboard`: no hover grace closes it.
    pub via_keyboard: bool,
    /// `keyboardFocusReturnRef`: the tile a keyboard open hands focus back to.
    pub(super) opener: Option<FocusHandle>,
    /// Each tile's bounds and hitbox this frame.
    tiles: [Option<(Bounds<Pixels>, Hitbox)>; 2],
    /// Whether each tile was in view at its last prepaint.
    visible: [bool; 2],
    /// The panel list's Show all, reset with the panel.
    pub(super) show_all: bool,
    /// A panel row's menu ran an item, so the press that chose it stays inside.
    pub(super) chosen: bool,
    /// Focus has moved into a keyboard-opened panel.
    pub(super) focused_in: bool,
    close: Option<Task<()>>,
    _escape: Option<gpui::Subscription>,
}

impl Global for Panels {}

fn panels(cx: &mut App) -> &mut Panels {
    cx.default_global::<Panels>()
}

/// `openSection`: opens or switches the panel at `anchor`; a keyboard open remembers `opener`.
pub fn open(section: Section, anchor: Bounds<Pixels>, opener: Option<FocusHandle>, cx: &mut App) {
    let p = panels(cx);
    let via_keyboard = opener.is_some();
    if p.open != Some(section) {
        p.show_all = false;
    }
    (p.open, p.anchor, p.via_keyboard, p.close) = (Some(section), Some(anchor), via_keyboard, None);
    if p._escape.is_none() {
        let escape = super::panels::escape(cx);
        panels(cx)._escape = Some(escape);
    }
    let p = panels(cx);
    if via_keyboard {
        (p.opener, p.focused_in) = (opener, false);
    }
    log::info!(target: "sidebar", "rail panel {} opened, keyboard {via_keyboard}", section.key());
    cx.refresh_windows();
}

/// `cancelClose`.
pub fn cancel_close(cx: &mut App) {
    panels(cx).close = None;
}

/// `scheduleClose`: the grace after the pointer leaves; a keyboard open waits for an explicit close.
pub fn schedule_close(window: &mut Window, cx: &mut App) {
    if panels(cx).via_keyboard || super::panels::suppressed(window, cx) {
        return;
    }
    let task = window.spawn(cx, async move |cx| {
        cx.background_executor().timer(CLOSE_GRACE).await;
        let done = cx.update(|window, cx| {
            panels(cx).close = None;
            if !super::panels::suppressed(window, cx) {
                close_all(window, cx);
            }
        });
        if let Err(error) = done {
            log::error!(target: "sidebar", "rail panel close: window gone: {error:#}");
        }
    });
    panels(cx).close = Some(task);
}

/// `closeAll`; focus goes back to a keyboard opener still in view, as `focusIfVisible` does.
pub fn close_all(window: &mut Window, cx: &mut App) {
    let p = panels(cx);
    p.close = None;
    let Some(section) = p.open.take() else {
        return;
    };
    (p.anchor, p.via_keyboard, p.show_all, p.chosen) = (None, false, false, false);
    p._escape = None;
    let back = p.opener.take().filter(|_| p.visible[section.index()]);
    log::info!(target: "sidebar", "rail panel {} closed, focus back {}", section.key(), back.is_some());
    if let Some(back) = back {
        window.focus(&back, cx);
    }
    cx.refresh_windows();
}

/// Whether `at` hits a tile's drawn surface, `data-rail-panel-trigger`.
pub(super) fn on_tile(at: gpui::Point<Pixels>, window: &Window, cx: &App) -> bool {
    cx.try_global::<Panels>().is_some_and(|p| {
        p.tiles
            .iter()
            .flatten()
            .any(|(_, hit)| hit.is_hovered_at(at, window))
    })
}

/// The rail ended under an open panel: it closes, and its tile, now hidden, takes no focus.
pub fn rail_ended(window: &mut Window, cx: &mut App) {
    panels(cx).opener = None;
    close_all(window, cx);
}

/// A panel row's menu ran an item: the press stays inside the panel.
pub(crate) fn chosen(cx: &mut App) {
    panels(cx).chosen = true;
}

/// `openSectionAt`: the pinned preview goes and the tile's bounds anchor the panel.
fn open_at(section: Section, opener: Option<FocusHandle>, window: &mut Window, cx: &mut App) {
    pinned::hide_preview(window, cx);
    let Some((anchor, _)) = panels(cx).tiles[section.index()].clone() else {
        log::warn!(target: "sidebar", "rail panel {}: tile not laid out", section.key());
        return;
    };
    open(section, anchor, opener, cx);
}

/// The two tiles, after the Pinned divider.
pub fn tiles(window: &mut Window, cx: &mut App) -> Vec<AnyElement> {
    [Section::Projects, Section::Dialogues]
        .into_iter()
        .map(|section| tile(section, window, cx))
        .collect()
}

fn tile(section: Section, window: &mut Window, cx: &mut App) -> AnyElement {
    let index = section.index();
    let id = match section {
        Section::Projects => "rail-projects",
        Section::Dialogues => "rail-dialogues",
    };
    let focus = super::tab_stop(id, window, cx);
    let open = panels(cx).open == Some(section);
    let hovered = window.use_keyed_state(format!("{id}-hover"), cx, |_, _| false);
    let lit = *hovered.read(cx);
    let label = cx
        .global::<Translator>()
        .t(&format!("ccAgent.sidebar.railNav.{}", section.key()), &[]);
    let source = match section {
        Section::Projects => ely_gpui_component::primitives::IconName::Folder
            .svg()
            .to_vec(),
        Section::Dialogues => lucide::message_circle().into_bytes(),
    };
    let appearance = cx.global::<Appearance>();
    let hover = appearance.hsl_token("sidebar-item-hover");
    let (fg, chip) = match open {
        true => (
            appearance.token("msg-assistant-text"),
            Some(appearance.token("chat-input-chip-bg")),
        ),
        false => (appearance.hsl_token("titlebar-icon"), None),
    };
    // `IntersectionObserver` on the trigger: out of view, its panel closes.
    let record = move |bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App| {
        let hit = window.insert_hitbox(bounds, HitboxBehavior::Normal);
        let shown = bounds.intersect(&window.content_mask().bounds);
        let shown = shown.size.width > Pixels::ZERO && shown.size.height > Pixels::ZERO;
        let p = panels(cx);
        (p.tiles[index], p.visible[index]) = (Some((bounds, hit)), shown);
        if p.open == Some(section) && !shown {
            log::info!(target: "sidebar", "rail panel {}: its tile left view", section.key());
            window.defer(cx, close_all);
        }
    };
    let opener = focus.clone();
    div()
        .id(id)
        .debug_selector(move || id.into())
        .track_focus(&focus)
        .role(Role::Button)
        .aria_label(label.clone())
        .aria_expanded(open)
        .relative()
        .flex()
        .flex_none()
        .size(css(36.0))
        .items_center()
        .justify_center()
        .rounded(css(9999.0))
        .cursor_pointer()
        .map(|el| match chip {
            Some(chip) => el.bg(chip),
            None => el.when(lit, |el| el.bg(hover)),
        })
        // `controlledOpen={false}` while its panel is open.
        .when(!open, |el| el.tooltip(tip(label)))
        .hover_listener_mode(HoverListenerMode::InputModalityIndependent)
        .on_hover(move |on, window, cx| {
            hovered.update(cx, |h, cx| {
                *h = *on;
                cx.notify();
            });
            match (*on, panels(cx).open == Some(section)) {
                (true, false) => open_at(section, None, window, cx),
                (true, true) => cancel_close(cx),
                // `relatedTarget` inside the panel is no leaving.
                (false, _) if !super::panels::inside(window.mouse_position(), window, cx) => {
                    schedule_close(window, cx)
                }
                (false, _) => {}
            }
        })
        .on_click(move |event: &ClickEvent, window, cx| {
            let opener = event.is_keyboard().then(|| opener.clone());
            open_at(section, opener, window, cx)
        })
        .child(canvas(record, |_, _, _, _| {}).absolute().inset_0())
        .child(lucide::icon(&source, 18.0, 2.0).text_color(fg))
        .into_any_element()
}

#[cfg(test)]
mod tests;
