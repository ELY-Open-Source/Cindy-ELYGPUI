//! `LocalDbFatalScreen`: an opaque surface, the modal scrim, and a dialog that names the cause.

use ely_gpui_component::{
    buttons::{Button, ButtonVariant},
    theme::ControlSize,
    typography::SelectableText,
};
use gpui::{
    Animation, AnimationExt as _, AnyElement, App, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, WindowControlArea, div, prelude::FluentBuilder,
};

use super::{Fatal, LocalDb, State};
use crate::{
    appearance_settings::text_size, i18n::Translator, motion, theme::Appearance, units::css,
};

/// `resolveLocalDbFatalView`, less the updater views (chapter 23) and `corrupt`, which needs
/// the backup restore this port never runs: title, description and the retry label if any.
fn view(fatal: &Fatal, t: &dyn Fn(&str) -> String) -> (String, String, Option<String>) {
    match fatal.code {
        "DB_SCHEMA_LOCKED" => (
            t("localDbFatal.schemaLease.title"),
            fatal.message.clone(),
            Some(t("localDbFatal.schemaLease.retry")),
        ),
        "DB_INIT_FAILED" => (
            t("localDbFatal.initFailed.title"),
            t("localDbFatal.initFailed.description"),
            Some(t("localDbFatal.initFailed.retry")),
        ),
        _ => (
            t("localDbFatal.noUpdate.title"),
            t("localDbFatal.noUpdate.description"),
            None,
        ),
    }
}

/// `localDbFatalDetailsText`.
fn details(fatal: &Fatal) -> String {
    [fatal.code, fatal.message.as_str()]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The screen over the whole window, chrome included, while the database refuses to open.
pub fn screen(cx: &App) -> Option<AnyElement> {
    let LocalDb {
        state: State::Failed(fatal),
        details_open,
        ..
    } = cx.global::<LocalDb>()
    else {
        return None;
    };
    let translator = cx.global::<Translator>();
    let t = |key: &str| translator.t(key, &[]);
    let (title, description, retry) = view(fatal, &t);
    let appearance = cx.global::<Appearance>();
    let (body, small) = (text_size(16.0, cx), text_size(12.0, cx));
    let open = *details_open;
    let toggle = div()
        .id("local-db-details")
        .cursor_pointer()
        .text_size(css(small))
        .text_color(appearance.token("text-tertiary"))
        .hover(|s| s.text_color(appearance.token("text-secondary")))
        .on_click(|_, _, cx| {
            let db = cx.global_mut::<LocalDb>();
            db.details_open = !db.details_open;
            cx.refresh_windows();
        })
        .child(t("localDbFatal.details"));
    let text = div()
        .id("local-db-details-text")
        .mt(css(8.0))
        .max_h(css(160.0))
        .overflow_y_scroll()
        .rounded(css(8.0))
        .bg(appearance.token("surface-chip"))
        .p(css(12.0))
        .text_size(css(text_size(11.0, cx)))
        .text_color(appearance.token("text-secondary"))
        .child(SelectableText::new(
            "local-db-details-select",
            details(fatal),
        ));
    let copy = (fatal.code == "DB_INIT_FAILED").then(|| {
        let text = details(fatal);
        let button = Button::new("local-db-copy", t("localDbFatal.corrupt.copy"))
            .variant(ButtonVariant::Secondary)
            .size(ControlSize::Sm)
            .on_click(move |_, _, cx| {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.clone()));
                let copied =
                    cx.read_from_clipboard().and_then(|item| item.text()) == Some(text.clone());
                if !copied {
                    log::warn!(target: "localDb", "technical details did not reach the clipboard");
                }
                cx.global_mut::<LocalDb>().details_open |= !copied;
                cx.refresh_windows();
            });
        div()
            .debug_selector(|| "local-db-copy".into())
            .child(button)
    });
    let confirm = retry.map(|label| {
        let button = Button::new("local-db-retry", label)
            .variant(ButtonVariant::Primary)
            .size(ControlSize::Md)
            .on_click(|_, _, cx| super::ensure_ready(cx));
        div()
            .mt(css(24.0))
            .flex()
            .justify_end()
            .child(div().min_w(css(96.0)).child(button))
    });
    let panel = div()
        .w_full()
        .max_w(css(400.0))
        .p(css(16.0))
        .rounded(css(12.0))
        .border(css(1.0))
        .border_color(appearance.token("border-default"))
        .bg(appearance.token("confirm-bg"))
        .shadow(vec![appearance.shadow_token("shadow-menu", cx)])
        .child(
            div()
                .text_size(css(text_size(18.0, cx)))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(appearance.token("confirm-title"))
                .child(title),
        )
        .child(
            div()
                .mt(css(8.0))
                .text_size(css(body))
                .text_color(appearance.token("confirm-desc"))
                .child(description),
        )
        .child(
            div()
                .mt(css(12.0))
                .flex()
                .flex_col()
                .items_start()
                .gap(css(4.0))
                .children(copy)
                .child(toggle)
                .when(open, |el| el.child(text)),
        )
        .children(confirm)
        .with_animation(
            "local-db-fatal-in",
            Animation::new(motion::length(motion::ENTER, cx)).with_easing(motion::motion_ease_out),
            |el, d| el.opacity(d).mt(css(8.0 * (1.0 - d))),
        );
    let screen = div()
        .id("local-db-fatal")
        .debug_selector(|| "local-db-fatal".into())
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .bg(appearance.token("surface"))
        .window_control_area(WindowControlArea::Drag)
        .child(
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .p(css(16.0))
                .bg(appearance.token("overlay-modal"))
                .child(panel),
        );
    Some(gpui::deferred(screen).with_priority(3).into_any_element())
}
