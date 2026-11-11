//! `UserAttachmentChip`: a message's file chips over its bubble; clicks and menus come with 9.13.

use ely_gpui_component::primitives::IconName;
use gpui::{
    AnyElement, App, ElementId, FontWeight, InteractiveElement as _, IntoElement as _,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _,
};

use super::chip;
use crate::{
    appearance_settings::text_size,
    attachment_safety,
    chat_message::merge::{Fields, get},
    i18n::{Arg, Translator},
    lucide, motion,
    rich_answer::js::Js,
    theme::Appearance,
    tip,
    transition::{self, Curve},
    units::css,
    wallpaper,
};

/// The chip's `max-w-[min(280px,100%)]` in a column `room` wide.
fn most(room: f32) -> f32 {
    room.min(280.0)
}

/// A file's name and path.
fn named(file: &Js) -> Option<(&str, &str)> {
    let file = file.obj()?;
    Some((file.get("name")?.str()?, file.get("path")?.str()?))
}

/// The message's `files`.
pub fn files(message: &Fields) -> Vec<(&str, &str)> {
    let list = get(message, "files").and_then(Js::arr).unwrap_or_default();
    list.iter().filter_map(named).collect()
}

/// Whether the chat only saves the file, and its icon: Download, else `FileTypeIcon`.
fn face(name: &str, path: &str) -> (bool, &'static [u8]) {
    match attachment_safety::downgraded(name, path) {
        true => (true, IconName::Download.svg()),
        false => (false, chip::file_icon(name)),
    }
}

/// One chip, at most `room` CSS pixels wide.
fn file(
    id: String,
    (name, path): (&str, &str),
    room: f32,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let element = ElementId::Name(id.clone().into());
    let child = |name: &'static str| {
        ElementId::NamedChild(std::sync::Arc::new(element.clone()), name.into())
    };
    // `transition-colors hover:bg-[var(--cmd-palette-item-hover)]`; hover ignores key presses.
    let hovered = window.use_keyed_state(child("hover"), cx, |_, _| false);
    let appearance = cx.global::<Appearance>();
    let fill = match *hovered.read(cx) {
        true => appearance.token("cmd-palette-item-hover"),
        false => wallpaper::glass(appearance.token("msg-user-bg"), cx),
    };
    let curve = Curve {
        length: motion::length(motion::FAST, cx),
        easing: motion::ease_move,
        lerp: transition::mix_srgb,
    };
    let fill = transition::keyed(child("fill"), fill, curve, window, cx);
    let appearance = cx.global::<Appearance>();
    let (color, border) = (
        appearance.token("msg-user-text"),
        appearance.token("msg-user-border"),
    );
    // A file the chat only saves names what a click does.
    let (save, icon) = face(name, path);
    let label = save.then(|| {
        let t = cx.global::<Translator>();
        t.t(
            "chat.userMessage.saveAttachmentAs",
            &[("name", Arg::Text(name))],
        )
    });
    let title = name.to_owned();
    div()
        .id(element)
        .debug_selector(move || id)
        .when_some(label, |el, label| el.aria_label(label))
        .hover_listener_mode(gpui::HoverListenerMode::InputModalityIndependent)
        .on_hover(move |over, _, cx| {
            hovered.update(cx, |hovered, cx| {
                *hovered = *over;
                cx.notify();
            })
        })
        .flex()
        .items_center()
        .gap(css(6.0))
        .h(css(28.0))
        .min_w_0()
        .max_w(css(most(room)))
        .rounded(css(9999.0))
        .border(css(1.0))
        .border_color(border)
        .bg(fill)
        .px(css(10.0))
        .cursor_pointer()
        .text_size(css(text_size(13.0, cx)))
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
        .child(lucide::icon(icon, 14.0, 2.0).text_color(color))
        .child(div().min_w_0().truncate().child(chip::nowrap(name)))
        // The native `title` is gpui's one tooltip; an empty one shows nothing.
        .when(!title.is_empty(), |el| el.tooltip(tip::tip(title)))
        .into_any_element()
}

/// `fileAttachmentNodes`: the chips wrapped in a column `room` CSS pixels wide, set `left` or right.
pub fn row(
    key: &str,
    files: &[(&str, &str)],
    (room, left): (f32, bool),
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let chip =
        |(n, at): (usize, &(&str, &str))| file(format!("{key}-file-{n}"), *at, room, window, cx);
    let chips: Vec<AnyElement> = files.iter().enumerate().map(chip).collect();
    let selector = format!("{key}-files");
    div()
        .debug_selector(move || selector)
        .flex()
        .flex_wrap()
        .items_end()
        .map(|el| match left {
            true => el.justify_start(),
            false => el.justify_end(),
        })
        .gap(css(6.0))
        .max_w_full()
        .children(chips)
        .into_any_element()
}

#[cfg(test)]
mod tests;
