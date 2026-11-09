//! `beginRename` and `commitRename`: the project name's field.

use ely_gpui_component::forms::{InputEvent, TextInput};
use gpui::{
    AnyElement, App, AppContext as _, Entity, InteractiveElement as _, IntoElement as _,
    MouseButton, ParentElement as _, StatefulInteractiveElement as _, Styled as _, Subscription,
    Window, div,
};

use super::{super::Sessions, alias};
use crate::{
    appearance_settings::text_size, i18n::Translator, theme::Appearance,
    theme_import::color::js_trim, units::css,
};

/// The project under rename, as `isEditingName` and `editValue` keep it.
pub struct Editing {
    key: String,
    input: Entity<TextInput>,
    /// `project.displayName` when the field opened.
    display: String,
    _events: Subscription,
}

/// The field opens on the shown name, selected and focused.
pub fn start(key: &str, display: &str, window: &mut Window, cx: &mut App) {
    let input = cx.new(|cx| {
        let mut input = TextInput::new(window, cx);
        input.set_text(display.to_owned(), cx);
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
    log::info!(target: "sessions", "rename project {key} started");
    cx.global_mut::<Sessions>().project_editing = Some(Editing {
        key: key.to_owned(),
        input,
        display: display.to_owned(),
        _events: events,
    });
    cx.refresh_windows();
}

/// The field of `key`'s header, if it is under rename.
pub fn editing<'a>(key: &str, cx: &'a App) -> Option<&'a Editing> {
    let e = cx.global::<Sessions>().project_editing.as_ref()?;
    (e.key == key).then_some(e)
}

/// `commitRename`: once; an unchanged name saves nothing, a blank one clears the alias.
fn commit(raw: &str, window: &mut Window, cx: &mut App) {
    let Some(editing) = cx.global_mut::<Sessions>().project_editing.take() else {
        return;
    };
    cx.refresh_windows();
    let trimmed = js_trim(raw);
    if trimmed == editing.display {
        log::info!(target: "sessions", "rename project {} closed unchanged", editing.key);
        return;
    }
    alias::rename(&editing.key, trimmed, window, cx);
}

fn cancel(cx: &mut App) {
    if let Some(editing) = cx.global_mut::<Sessions>().project_editing.take() {
        log::info!(target: "sessions", "rename project {} cancelled", editing.key);
        cx.refresh_windows();
    }
}

/// The `h-6 px-1.5 rounded text-sm` field in a 1.5px `--focus-ring` border.
pub fn field(editing: &Editing, cx: &App) -> AnyElement {
    let appearance = cx.global::<Appearance>();
    let label = cx
        .global::<Translator>()
        .t("ccAgent.sidebar.projectAlias.inputLabel", &[]);
    let input = editing.input.clone();
    div()
        .id("project-rename")
        .debug_selector(|| "project-rename".into())
        .aria_label(label)
        .flex()
        .flex_1()
        .min_w_0()
        .items_center()
        .h(css(24.0))
        .px(css(6.0))
        .rounded(css(4.0))
        .border(css(1.5))
        .border_color(appearance.token("focus-ring"))
        .text_size(css(text_size(14.0, cx)))
        .text_color(appearance.hsl_token("foreground"))
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
        .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
        .child(editing.input.clone())
        .into_any_element()
}
