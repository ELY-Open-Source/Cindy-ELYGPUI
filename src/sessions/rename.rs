//! `SessionRenameInput` and the row's rename flow: the display title selected in a field,
//! Enter, blur or a press outside saves, Escape drops it.

use ely_gpui_component::forms::{InputEvent, TextInput};
use gpui::{
    AnyElement, App, AppContext as _, Entity, InteractiveElement as _, IntoElement as _,
    MouseButton, ParentElement as _, StatefulInteractiveElement as _, Styled as _, Subscription,
    Window, div,
};

use super::{Sessions, group::Session, row::Scope, title, write};
use crate::{
    appearance_settings::text_size, i18n::Translator, theme::Appearance,
    theme_import::color::js_trim, units::css,
};

/// The row under edit, as `SessionItem`'s `isEditing` and `editValue` keep it.
pub struct Editing {
    pub id: String,
    /// The list whose row holds the field.
    pub scope: Scope,
    input: Entity<TextInput>,
    /// The display title the field opened with.
    display: String,
    _events: Subscription,
}

/// `handleDoubleClick` and `handleRenameSelect`: the field opens on the display title,
/// selected and focused.
pub fn start(s: &Session, scope: Scope, window: &mut Window, cx: &mut App) {
    let t = cx.global::<Translator>();
    let display = match scope {
        Scope::Header => title::header(s, t),
        _ => title::display(s, t),
    };
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
        scope,
        input,
        display,
        _events: events,
    });
    cx.refresh_windows();
}

/// The field of `id`'s row in `scope`, if it is under edit.
pub fn editing<'a>(id: &str, scope: Scope, cx: &'a App) -> Option<&'a Editing> {
    let e = cx.global::<Sessions>().editing.as_ref()?;
    (e.id == id && e.scope == scope).then_some(e)
}

/// The content header's field, whatever task it opened on.
pub fn header_editing(cx: &App) -> Option<&Editing> {
    let e = cx.global::<Sessions>().editing.as_ref()?;
    (e.scope == Scope::Header).then_some(e)
}

/// The content header unmounted: its edit goes with it, unsaved.
pub fn drop_header(cx: &mut App) {
    if header_editing(cx).is_some() {
        log::info!(target: "sessions", "the header's rename dropped: the header is gone");
        cx.global_mut::<Sessions>().editing = None;
    }
}

/// `commitTitle`: once; blank or unchanged saves nothing. The header's goes to the task shown now.
fn commit(raw: &str, window: &mut Window, cx: &mut App) {
    let Some(editing) = cx.global_mut::<Sessions>().editing.take() else {
        return;
    };
    cx.refresh_windows();
    let trimmed = js_trim(raw);
    let header = editing.scope == Scope::Header;
    let session = match header {
        true => crate::session_view::header::task(cx).cloned(),
        false => crate::sessions::find(&editing.id, cx).cloned(),
    };
    let Some(session) = session else {
        log::warn!(target: "sessions", "rename {}: the task is gone", editing.id);
        return;
    };
    let unchanged = match header {
        // The header also counts the stored title as unchanged.
        true => {
            let shown = title::header(&session, cx.global::<Translator>());
            trimmed == session.title || trimmed == shown
        }
        false => trimmed == editing.display,
    };
    if trimmed.is_empty() || unchanged {
        log::info!(target: "sessions", "rename {} closed unchanged", session.id);
        return;
    }
    let stored = title::stored(&session, trimmed);
    write::apply(&session.id, write::Meta::Title(stored), window, cx);
}

/// Escape: the edit ends with nothing saved.
fn cancel(cx: &mut App) {
    if let Some(editing) = cx.global_mut::<Sessions>().editing.take() {
        log::info!(target: "sessions", "rename {} cancelled", editing.id);
        cx.refresh_windows();
    }
}

/// The field's type: `text-sm font-medium`; the AI rename button in `pr-7` comes with 11.0.
pub const TEXT: (f32, gpui::FontWeight) = (14.0, gpui::FontWeight::MEDIUM);

/// A row's field: `min-w-0 flex-1`, in the active foreground on the active row.
pub fn field(
    editing: &Editing,
    text: (f32, gpui::FontWeight),
    active: bool,
    cx: &App,
) -> AnyElement {
    frame(editing, text, 24.0, active, cx)
        .flex_1()
        .min_w_0()
        .into_any_element()
}

/// The content header's field: `w-72 max-w-[40vw]` around an `h-7` input.
pub fn header_field(editing: &Editing, window: &Window, cx: &App) -> AnyElement {
    frame(editing, TEXT, 28.0, false, cx)
        // `WINDOW_NO_DRAG_STYLE`: a press here never moves the window.
        .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
        .w(css(288.0))
        .max_w(window.viewport_size().width * 0.4)
        .into_any_element()
}

/// `SessionRenameInput`: a pill `h-6` or `h-7` tall; the active row borders it at 28%.
fn frame(
    editing: &Editing,
    (size, weight): (f32, gpui::FontWeight),
    height: f32,
    active: bool,
    cx: &App,
) -> gpui::Stateful<gpui::Div> {
    let appearance = cx.global::<Appearance>();
    let active_fg = appearance.token("sidebar-item-active-foreground");
    let (border, color) = match active {
        true => (active_fg.opacity(0.28), active_fg),
        false => (
            appearance.token("border-default"),
            appearance.hsl_token("foreground"),
        ),
    };
    let input = editing.input.clone();
    div()
        .id("task-rename")
        .debug_selector(|| "task-rename".into())
        .relative()
        .flex()
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
        .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
        .child(
            div()
                .flex()
                .items_center()
                .w_full()
                .min_w_0()
                .h(css(height))
                .pl(css(6.0))
                .pr(css(28.0))
                .rounded(css(9999.0))
                .border(css(1.0))
                .border_color(border)
                .text_size(css(text_size(size, cx)))
                .font_weight(weight)
                .text_color(color)
                .child(editing.input.clone()),
        )
}
