//! `SessionRenameInput` and the row's rename flow: the display title selected in a field,
//! Enter, blur or a press outside saves, Escape drops it.

use ely_gpui_component::forms::{InputEvent, TextInput};
use gpui::{
    AnyElement, App, AppContext as _, Entity, InteractiveElement as _, IntoElement as _,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, Subscription, Window, div,
};

use super::{Sessions, group::Session, title, write};
use crate::{
    appearance_settings::text_size, i18n::Translator, theme::Appearance,
    theme_import::color::js_trim, units::css,
};

/// The row under edit, as `SessionItem`'s `isEditing` and `editValue` keep it.
pub struct Editing {
    pub id: String,
    input: Entity<TextInput>,
    /// The display title the field opened with.
    display: String,
    _events: Subscription,
}

/// `handleDoubleClick` and `handleRenameSelect`: the field opens on the display title,
/// selected and focused.
pub fn start(s: &Session, window: &mut Window, cx: &mut App) {
    let display = title::display(s, cx.global::<Translator>());
    let input = cx.new(|cx| {
        let mut input = TextInput::new(window, cx);
        input.set_text(display.clone(), cx);
        input.select(0..display.len(), cx);
        input
    });
    let events = window.subscribe(&input, cx, |input, event, window, cx| {
        if matches!(event, InputEvent::Submit | InputEvent::Blur) {
            let text = input.read(cx).text().to_owned();
            commit(&text, window, cx);
        }
    });
    window.focus(&input.read(cx).focus().clone(), cx);
    log::info!(target: "sessions", "rename {} started", s.id);
    let store = cx.global_mut::<Sessions>();
    if let Some(stale) = &store.editing {
        log::info!(target: "sessions", "rename {} dropped: its field is gone", stale.id);
    }
    store.editing = Some(Editing {
        id: s.id.clone(),
        input,
        display,
        _events: events,
    });
    cx.refresh_windows();
}

/// `commitTitle`: once; a blank or unchanged title saves nothing.
fn commit(raw: &str, window: &mut Window, cx: &mut App) {
    let Some(editing) = cx.global_mut::<Sessions>().editing.take() else {
        return;
    };
    cx.refresh_windows();
    let trimmed = js_trim(raw);
    if trimmed.is_empty() || trimmed == editing.display {
        log::info!(target: "sessions", "rename {} closed unchanged", editing.id);
        return;
    }
    let Some(session) = cx
        .global::<Sessions>()
        .loaded
        .as_ref()
        .and_then(|l| l.sessions.iter().find(|s| s.id == editing.id).cloned())
    else {
        log::warn!(target: "sessions", "rename {}: the task left the list", editing.id);
        return;
    };
    let stored = title::stored(&session, trimmed);
    write::apply(&editing.id, write::Meta::Title(stored), window, cx);
}

/// Escape: the edit ends with nothing saved.
fn cancel(cx: &mut App) {
    if let Some(editing) = cx.global_mut::<Sessions>().editing.take() {
        log::info!(target: "sessions", "rename {} cancelled", editing.id);
        cx.refresh_windows();
    }
}

/// `min-w-0 flex-1` around `h-6 rounded-full border px-1.5 pr-7 text-sm font-medium`; the
/// AI rename button that fills `pr-7` comes with 11.0.
pub fn field(editing: &Editing, cx: &App) -> AnyElement {
    let appearance = cx.global::<Appearance>();
    let input = editing.input.clone();
    div()
        .id("task-rename")
        .debug_selector(|| "task-rename".into())
        .relative()
        .flex()
        .flex_1()
        .min_w_0()
        .items_center()
        .capture_key_down(|event, window, cx| {
            if event.keystroke.key == "escape" {
                window.prevent_default();
                cx.stop_propagation();
                cancel(cx);
            }
        })
        .on_mouse_down_out(move |_, window, cx| {
            let text = input.read(cx).text().to_owned();
            commit(&text, window, cx);
        })
        .on_click(|_, _, cx| cx.stop_propagation())
        .child(
            div()
                .flex()
                .items_center()
                .w_full()
                .min_w_0()
                .h(css(24.0))
                .pl(css(6.0))
                .pr(css(28.0))
                .rounded(css(9999.0))
                .border(css(1.0))
                .border_color(appearance.token("border-default"))
                .text_size(css(text_size(14.0, cx)))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(appearance.hsl_token("foreground"))
                .child(editing.input.clone()),
        )
        .into_any_element()
}
