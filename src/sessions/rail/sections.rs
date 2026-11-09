//! `RailNav`'s Projects and Dialogues tiles over `railPanelStore`: which panel is open, where.

use std::time::Duration;

use gpui::{
    AnyElement, App, Bounds, ClickEvent, Global, HoverListenerMode, InteractiveElement as _,
    IntoElement, ParentElement as _, Pixels, Role, StatefulInteractiveElement as _, Styled as _,
    Task, Window, canvas, div, prelude::FluentBuilder as _,
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
    fn key(self) -> &'static str {
        match self {
            Section::Projects => "projects",
            Section::Dialogues => "dialogues",
        }
    }
}

/// `railPanelStore`'s state; the project level and the lamp scope come with 8.7e3c and 10.0.
#[derive(Default)]
pub struct Panels {
    pub open: Option<Section>,
    /// The trigger's bounds when it opened; the panel takes its right and top.
    pub anchor: Option<Bounds<Pixels>>,
    /// `openedViaKeyboard`: no hover grace closes it.
    pub via_keyboard: bool,
    /// Each tile's bounds this frame.
    tiles: [Option<Bounds<Pixels>>; 2],
    close: Option<Task<()>>,
}

impl Global for Panels {}

fn panels(cx: &mut App) -> &mut Panels {
    cx.default_global::<Panels>()
}

/// `openSection`: opens or switches the panel at `anchor`.
pub fn open(section: Section, anchor: Bounds<Pixels>, via_keyboard: bool, cx: &mut App) {
    let p = panels(cx);
    (p.open, p.anchor, p.via_keyboard, p.close) = (Some(section), Some(anchor), via_keyboard, None);
    log::info!(target: "sidebar", "rail panel {} opened, keyboard {via_keyboard}", section.key());
    cx.refresh_windows();
}

/// `cancelClose`.
pub fn cancel_close(cx: &mut App) {
    panels(cx).close = None;
}

/// `scheduleClose`: the grace after the pointer leaves; a keyboard open waits for an explicit close.
pub fn schedule_close(window: &mut Window, cx: &mut App) {
    if panels(cx).via_keyboard {
        return;
    }
    let task = window.spawn(cx, async move |cx| {
        cx.background_executor().timer(CLOSE_GRACE).await;
        let done = cx.update(|_, cx| {
            panels(cx).close = None;
            close_all(cx);
        });
        if let Err(error) = done {
            log::error!(target: "sidebar", "rail panel close: window gone: {error:#}");
        }
    });
    panels(cx).close = Some(task);
}

/// `closeAll`.
pub fn close_all(cx: &mut App) {
    let p = panels(cx);
    p.close = None;
    if let Some(section) = p.open.take() {
        (p.anchor, p.via_keyboard) = (None, false);
        log::info!(target: "sidebar", "rail panel {} closed", section.key());
        cx.refresh_windows();
    }
}

/// `openSectionAt`: the pinned preview goes and the tile's bounds anchor the panel.
fn open_at(section: Section, index: usize, via_keyboard: bool, window: &mut Window, cx: &mut App) {
    pinned::hide_preview(window, cx);
    let Some(anchor) = panels(cx).tiles[index] else {
        log::warn!(target: "sidebar", "rail panel {}: tile not laid out", section.key());
        return;
    };
    open(section, anchor, via_keyboard, cx);
}

/// The two tiles, after the Pinned divider.
pub fn tiles(window: &mut Window, cx: &mut App) -> Vec<AnyElement> {
    [Section::Projects, Section::Dialogues]
        .into_iter()
        .enumerate()
        .map(|(index, section)| tile(section, index, window, cx))
        .collect()
}

fn tile(section: Section, index: usize, window: &mut Window, cx: &mut App) -> AnyElement {
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
    let record = move |bounds, _: &mut Window, cx: &mut App| panels(cx).tiles[index] = Some(bounds);
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
                (true, false) => open_at(section, index, false, window, cx),
                (true, true) => cancel_close(cx),
                (false, _) => schedule_close(window, cx),
            }
        })
        .on_click(move |event: &ClickEvent, window, cx| {
            open_at(section, index, event.is_keyboard(), window, cx)
        })
        .child(canvas(record, |_, _, _, _| {}).absolute().inset_0())
        .child(lucide::icon(&source, 18.0, 2.0).text_color(fg))
        .into_any_element()
}

#[cfg(test)]
mod tests;
