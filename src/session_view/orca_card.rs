//! `UserMessage`'s Orca form: a Lead's or a Worker's message as a card that unfolds.

use ely_gpui_component::{primitives::IconName, theme::ActiveTheme as _};
use gpui::{
    AnyElement, App, ElementId, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, RenderOnce, Role, StatefulInteractiveElement as _, Styled as _, Window,
    div, prelude::FluentBuilder as _, relative, transparent_black,
};

use super::{
    chip,
    flow::draw,
    source_labels::{self, Align},
    user_row,
    width::Widths,
};
use crate::{
    appearance_settings::{code_size, text_size},
    chat_message::merge::{Fields, get},
    i18n::{Arg, Translator},
    lucide, motion, press,
    rich_answer::js::Js,
    theme::Appearance,
    transition::{self, Curve},
    units::css,
    user_message::{self, Orca},
    wallpaper,
};

/// The column's `max-w-[560px]`.
const COLUMN: f32 = 560.0;
/// The body's `px-3` and the card's border, on both sides.
const INSET: f32 = 26.0;
/// The card's `rounded-[8px]` inside its border.
const INNER: f32 = 7.0;

/// `orcaCardTitle`: the Lead, or the Worker by the role its task origin names.
fn title(message: &Fields, orca: &Orca, t: &Translator) -> String {
    let origin = get(message, "automationOrigin").and_then(Js::obj);
    let session = origin.filter(|origin| origin.get("kind").and_then(Js::str) == Some("session"));
    let role = session.and_then(|origin| origin.get("orcaSenderLabel")?.str());
    match role.filter(|role| !role.is_empty()) {
        _ if orca.source == "lead" => t.t("chat.userMessage.orcaFromLead", &[]),
        Some(role) => t.t(
            "chat.userMessage.orcaFromWorkerNamed",
            &[("role", Arg::Text(role))],
        ),
        None => t.t("chat.userMessage.orcaFromWorker", &[]),
    }
}

/// The card's header: a button that unfolds the message.
#[derive(IntoElement)]
struct Head {
    key: String,
    title: String,
    open: bool,
    compact: bool,
}

impl RenderOnce for Head {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let name = format!("{}-orca-head", self.key);
        let id = ElementId::Name(name.clone().into());
        let child = |name: &'static str| {
            ElementId::NamedChild(std::sync::Arc::new(id.clone()), name.into())
        };
        let focus = window
            .use_keyed_state(child("focus"), cx, |_, cx| cx.focus_handle().tab_stop(true))
            .read(cx)
            .clone();
        let (press, state) = press::state(&id, (&focus, false), window, cx);
        let appearance = cx.global::<Appearance>();
        let mark = appearance.token("msg-tool-card-chevron");
        let fill = match state.hovered {
            true => appearance.token("cmd-palette-item-hover"),
            false => transparent_black(),
        };
        let curve = Curve {
            length: motion::length(motion::FAST, cx),
            easing: motion::ease_move,
            lerp: transition::mix_srgb,
        };
        let fill = transition::keyed(child("fill"), fill, curve, window, cx);
        let chevron = match self.open {
            true => IconName::ChevronDown,
            false => IconName::ChevronRight,
        };
        let size = text_size(13.0, cx) - f32::from(u8::from(self.compact));
        let (title, key) = (format!("{}-orca-title", self.key), self.key);
        let el = div()
            .id(id)
            .debug_selector(move || name)
            .track_focus(&focus)
            .role(Role::Button)
            .aria_expanded(self.open)
            .flex()
            .w_full()
            .items_center()
            .gap(css(8.0))
            .px(css(12.0))
            .py(css(8.0))
            // gpui clips to no rounded corner, so the face takes the card's own.
            .rounded_t(css(INNER))
            .when(!self.open, |el| el.rounded_b(css(INNER)))
            .bg(fill)
            .cursor_pointer()
            .text_size(css(size))
            .font_weight(FontWeight::MEDIUM)
            .line_height(relative(1.0))
            .on_click(move |_, _, cx| user_row::toggle(&key, cx))
            .child(lucide::icon(IconName::Bot.svg(), 14.0, 2.0).text_color(mark))
            .child(
                div()
                    .debug_selector(move || title)
                    .min_w_0()
                    .flex_1()
                    .truncate()
                    .child(chip::nowrap(&self.title)),
            )
            .child(lucide::icon(chevron.svg(), 14.0, 2.0).text_color(mark));
        press::listen(el, &press, false)
    }
}

/// The unfolded message: `pre-wrap` in the code face.
fn body(key: &str, text: &str, widths: Widths, window: &Window, cx: &App) -> AnyElement {
    let rem = window.rem_size();
    let size = css(code_size(-2.0, cx) - f32::from(u8::from(widths.compact)));
    let line = css(code_size(4.0, cx));
    let mut style = window.text_style();
    style.font_family = cx.theme().mono_family.clone();
    (style.font_size, style.line_height) = (size.into(), line.into());
    let width = css((COLUMN.min(widths.message) - INSET).max(0.0)).to_pixels(rem);
    // Blink lays out no CR and no FF (`9.5q-r3-blink-controls.log`); an empty `pre` holds no line.
    let text: String = text
        .chars()
        .filter(|c| !matches!(c, '\r' | '\x0c'))
        .collect();
    let rows = match text.is_empty() {
        true => Vec::new(),
        false => draw::plain(&text, &style, width, window),
    };
    let appearance = cx.global::<Appearance>();
    let (selector, text_selector) = (format!("{key}-orca-body"), format!("{key}-orca-text"));
    let pre = div()
        .debug_selector(move || text_selector)
        .flex()
        .flex_col()
        .whitespace_nowrap()
        .font_family(style.font_family.clone())
        .text_size(size)
        .line_height(line)
        .text_color(appearance.hsl_token("foreground"))
        .children(rows);
    div()
        .debug_selector(move || selector)
        .border_t(css(1.0))
        .border_color(appearance.token("msg-tool-card-border"))
        .px(css(12.0))
        .py(css(8.0))
        .child(pre)
        .into_any_element()
}

/// The Orca message's row: its source labels, then the card, set left.
pub(super) fn element(
    key: &str,
    (message, content): (&Fields, &str),
    orca: &Orca,
    widths: Widths,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let origin = get(message, "automationOrigin");
    let forwarded = origin.and_then(Js::obj).and_then(|o| o.get("kind")?.str()) == Some("session");
    let hook = get(message, "hookSource");
    let text = user_message::display_text(content, hook, forwarded, Some(orca));
    let open = user_row::unfolded(key, cx);
    let head = Head {
        key: key.to_owned(),
        title: title(message, orca, cx.global::<Translator>()),
        open,
        compact: widths.compact,
    };
    let body = open.then(|| body(key, &text, widths, window, cx));
    let appearance = cx.global::<Appearance>();
    let selector = format!("{key}-orca");
    let card = div()
        .debug_selector(move || selector)
        .w_full()
        .overflow_hidden()
        .rounded(css(8.0))
        .border(css(1.0))
        .border_color(appearance.token("msg-tool-card-border"))
        .bg(wallpaper::glass(appearance.token("msg-tool-card-bg"), cx))
        .text_color(appearance.token("msg-tool-card-text"))
        .child(head)
        .children(body);
    let labels = source_labels::row(key, message, Align::Start, cx);
    let column = div()
        .flex()
        .flex_col()
        .items_start()
        .gap(css(8.0))
        .w_full()
        .max_w(css(COLUMN))
        .children(labels)
        .child(card);
    div()
        .flex()
        .w_full()
        .justify_start()
        .child(column)
        .into_any_element()
}

#[cfg(test)]
mod tests;
