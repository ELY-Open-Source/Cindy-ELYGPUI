//! `UserMessage`'s hook form: `HookTaskCard`, a channel's message as a card set left.

use ely_gpui_component::primitives::IconName;
use gpui::{
    AnyElement, App, ElementId, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, RenderOnce, Role, StatefulInteractiveElement as _, Styled as _,
    TextStyle, Transformation, Window, div, prelude::FluentBuilder as _, radians, relative,
};

use super::{
    attachments, chip,
    flow::draw,
    images, select,
    source_labels::{self, Over},
    user_row,
    width::Widths,
};
use crate::{
    appearance_settings::text_size,
    chat_message::merge::Fields,
    hook_card::{self, Context, Icon},
    i18n::Translator,
    lucide, motion, press,
    rich_answer::js::{Js, Obj},
    theme::Appearance,
    tip,
    transition::{self, Curve},
    units::css,
    user_message::{self, collapse},
    wallpaper,
};

/// The column's `max-w-[560px]`.
const COLUMN: f32 = 560.0;
/// The body's `px-[14px]` and the card's border, on both sides.
const INSET: f32 = 30.0;
/// The context's `pl-[18px]`.
const INDENT: f32 = 18.0;
/// `line-clamp-10`.
const KEPT: usize = 10;

const SLACK: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M5.042 15.165a2.528 2.528 0 0 1-2.52 2.523A2.528 2.528 0 0 1 0 15.165a2.527 2.527 0 0 1 2.522-2.52h2.52v2.52zM6.313 15.165a2.527 2.527 0 0 1 2.521-2.52 2.527 2.527 0 0 1 2.521 2.52v6.313A2.528 2.528 0 0 1 8.834 24a2.528 2.528 0 0 1-2.521-2.522v-6.313zM8.834 5.042a2.528 2.528 0 0 1-2.521-2.52A2.528 2.528 0 0 1 8.834 0a2.528 2.528 0 0 1 2.521 2.522v2.52H8.834zM8.834 6.313a2.528 2.528 0 0 1 2.521 2.521 2.528 2.528 0 0 1-2.521 2.521H2.522A2.528 2.528 0 0 1 0 8.834a2.528 2.528 0 0 1 2.522-2.521h6.312zM18.956 8.834a2.528 2.528 0 0 1 2.522-2.521A2.528 2.528 0 0 1 24 8.834a2.528 2.528 0 0 1-2.522 2.521h-2.522V8.834zM17.688 8.834a2.528 2.528 0 0 1-2.523 2.521 2.527 2.527 0 0 1-2.52-2.521V2.522A2.527 2.527 0 0 1 15.165 0a2.528 2.528 0 0 1 2.523 2.522v6.312zM15.165 18.956a2.528 2.528 0 0 1 2.523 2.522A2.528 2.528 0 0 1 15.165 24a2.527 2.527 0 0 1-2.52-2.522v-2.522h2.52zM15.165 17.688a2.527 2.527 0 0 1-2.52-2.523 2.526 2.526 0 0 1 2.52-2.52h6.313A2.527 2.527 0 0 1 24 15.165a2.528 2.528 0 0 1-2.522 2.523h-6.313z"/></svg>"#;
const X: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231 5.45-6.231zm-1.161 17.52h1.833L7.084 4.126H5.117l11.966 15.644z"/></svg>"#;

fn curve<T>(length: std::time::Duration, lerp: fn(T, T, f32) -> T, cx: &App) -> Curve<T> {
    Curve {
        length: motion::length(length, cx),
        easing: motion::ease_move,
        lerp,
    }
}

fn between(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// `disclosureClassName`: bare words that unfold something, with a chevron for the context.
#[derive(IntoElement)]
struct Disclosure {
    name: String,
    /// The fold's key in the content's record.
    fold: String,
    label: String,
    open: bool,
    chevron: bool,
    /// The label's size in CSS pixels and its line height as a ratio.
    text: (f32, f32),
}

impl RenderOnce for Disclosure {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = ElementId::Name(self.name.clone().into());
        let child = |name: &'static str| {
            ElementId::NamedChild(std::sync::Arc::new(id.clone()), name.into())
        };
        let focus = window
            .use_keyed_state(child("focus"), cx, |_, cx| cx.focus_handle().tab_stop(true))
            .read(cx)
            .clone();
        let (press, state) = press::state(&id, (&focus, false), window, cx);
        let appearance = cx.global::<Appearance>();
        let (rest, lit) = (
            appearance.token("text-tertiary"),
            appearance.token("text-secondary"),
        );
        let ring = appearance.token("focus-ring");
        let color = if state.hovered { lit } else { rest };
        let fast = curve(motion::FAST, transition::mix_srgb, cx);
        let color = transition::keyed(child("color"), color, fast, window, cx);
        let turn = curve(motion::FAST, between, cx);
        let turned = if self.open { 0.25 } else { 0.0 };
        let turned = transition::keyed(child("turn"), turned, turn, window, cx);
        let chevron = self.chevron.then(|| {
            lucide::icon(IconName::ChevronRight.svg(), 12.0, 2.0)
                .text_color(color)
                .with_transformation(Transformation::rotate(radians(
                    turned * std::f32::consts::TAU,
                )))
        });
        // `focus-visible:outline-2` at `outline-offset-2`.
        let outlined = focus.is_focused(window) && window.last_input_was_keyboard();
        let selector = format!("{}-outline", self.name);
        let outline = outlined.then(|| {
            div()
                .debug_selector(move || selector)
                .absolute()
                .inset(css(-4.0))
                .border(css(2.0))
                .border_color(ring)
        });
        let (name, fold) = (self.name, self.fold);
        let el = div()
            .id(id)
            .debug_selector(move || name)
            .track_focus(&focus)
            .role(Role::Button)
            .aria_expanded(self.open)
            .relative()
            .flex()
            .items_center()
            .when(self.chevron, |el| {
                el.gap(css(6.0)).font_weight(FontWeight::MEDIUM)
            })
            .min_h(css(24.0))
            .min_w(css(24.0))
            .py(css(4.0))
            .cursor_pointer()
            .text_size(css(self.text.0))
            .line_height(relative(self.text.1))
            .text_color(color)
            .on_click(move |_, _, cx| user_row::toggle(&fold, cx))
            .children(chevron)
            .child(self.label)
            .relative()
            .child(select::still(0.0, 0.0))
            .children(outline);
        // `w-fit`: the row keeps the button from stretching.
        div().flex().child(press::listen(el, &press, false))
    }
}

/// The window's text style at `size` CSS pixels, with the line it takes at `ratio`.
fn style(size: f32, ratio: f32, window: &Window) -> (TextStyle, Pixels) {
    let rem = window.rem_size();
    let mut style = window.text_style();
    style.font_size = css(size).to_pixels(rem).into();
    let line = window.pixel_snap(css(size * ratio).to_pixels(rem));
    (style, line)
}

/// The unfolded context: each section's heading over its lines, with its height.
fn context(
    key: &str,
    context: &Context,
    width: f32,
    size: f32,
    window: &Window,
    cx: &App,
) -> (AnyElement, Pixels) {
    let rem = window.rem_size();
    let (style, line) = style(size, 1.5, window);
    let room = css((width - INDENT).max(0.0)).to_pixels(rem);
    let medium = TextStyle {
        font_weight: FontWeight::MEDIUM,
        ..style.clone()
    };
    let appearance = cx.global::<Appearance>();
    let quiet = appearance.token("text-tertiary");
    let (between_lines, between_sections) = (css(4.0).to_pixels(rem), css(12.0).to_pixels(rem));
    let mut height = css(8.0).to_pixels(rem);
    let mut sections = Vec::new();
    for (n, section) in context.sections.iter().enumerate() {
        let read = |text: &str, bold| draw::pre(text, (bold, None), (&style, room), window);
        let block = |part: usize| format!("{key}-hook-s{n}-{part}");
        // The heading breaks at the weight it is drawn in.
        let laid = draw::laid(&section.heading);
        let heading = draw::pre(&laid, (0, None), (&medium, room), window);
        let mut blocks = vec![heading.total];
        let heading = div()
            .font_weight(medium.font_weight)
            .text_color(quiet)
            .children(draw::lit(
                &block(0),
                (&laid, heading),
                (&medium, line),
                window,
            ));
        let mut parts = vec![heading.into_any_element()];
        for (nth, said) in section.lines.iter().enumerate() {
            let author = said.author.as_deref().map(draw::laid);
            let text = draw::laid(&said.text);
            let (text, bold) = match &author {
                Some(author) => (format!("{author} {text}"), author.len()),
                None => (text, 0),
            };
            let pre = read(&text, bold);
            blocks.push(pre.total);
            let rows = draw::lit(&block(nth + 1), (&text, pre), (&style, line), window);
            parts.push(div().children(rows).into_any_element());
        }
        let gaps = between_lines * (blocks.len() - 1) as f32;
        height += line * blocks.iter().sum::<usize>() as f32 + gaps;
        if n > 0 {
            height += between_sections;
        }
        let selector = format!("{key}-hook-section-{n}");
        let section = div()
            .debug_selector(move || selector)
            .flex()
            .flex_col()
            .gap(css(4.0))
            .children(parts);
        sections.push(section);
    }
    let selector = format!("{key}-hook-context-body");
    let body = div()
        .debug_selector(move || selector)
        .flex()
        .flex_col()
        .gap(css(12.0))
        .pt(css(8.0))
        .pl(css(INDENT))
        .whitespace_nowrap()
        .text_size(css(size))
        .line_height(line)
        .text_color(appearance.token("text-secondary"))
        .children(sections);
    (body.into_any_element(), height)
}

/// `Collapse`: the body's height and opacity follow the fold over 200ms; closed, it unmounts.
fn folding(
    key: &str,
    open: bool,
    (body, height): (AnyElement, Pixels),
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyElement> {
    let id = ElementId::Name(format!("{key}-hook-collapse").into());
    let slow = curve(motion::BASE, between, cx);
    let shown = transition::keyed(id, f32::from(u8::from(open)), slow, window, cx);
    if shown >= 1.0 {
        return Some(body);
    }
    if shown <= 0.0 {
        return None;
    }
    let selector = format!("{key}-hook-collapse");
    Some(
        div()
            .debug_selector(move || selector)
            .overflow_hidden()
            .h(height * shown)
            .opacity(shown)
            .child(body)
            .into_any_element(),
    )
}

/// The card's header: the channel's mark and who spoke.
fn head(key: &str, card: &hook_card::Card, size: f32, cx: &App) -> AnyElement {
    let appearance = cx.global::<Appearance>();
    let ink = appearance.token("text-primary");
    let icon = match card.icon {
        Icon::Slack => lucide::mark(SLACK, 14.0),
        Icon::X => lucide::mark(X, 14.0),
        Icon::Telegram => lucide::icon(IconName::Send.svg(), 14.0, 1.75),
        Icon::Message => lucide::icon(IconName::MessageSquare.svg(), 14.0, 1.75),
    };
    let (selector, id) = (format!("{key}-hook-title"), format!("{key}-hook-title"));
    let (title, drawn) = (card.title.clone(), chip::nowrap(&card.label));
    let label = div()
        .id(ElementId::Name(id.clone().into()))
        .debug_selector(move || selector)
        .min_w_0()
        .truncate()
        .text_size(css(size))
        .line_height(relative(1.5))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(ink)
        .relative()
        .child(select::label(&id, &drawn))
        .child(drawn.clone())
        .when_some(title, |el, title| el.tooltip(tip::tip(title)));
    let selector = format!("{key}-hook-head");
    div()
        .debug_selector(move || selector)
        .flex()
        .items_center()
        .gap(css(8.0))
        .px(css(14.0))
        .pb(css(6.0))
        .child(icon.text_color(ink))
        .child(label)
        .into_any_element()
}

/// The hook message's row: its labels, images and files, then the card, set left.
pub(super) fn element(
    key: &str,
    (message, content): (&Fields, &str),
    hook: &Js,
    widths: Widths,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let none = Obj::new();
    let source = hook.obj().unwrap_or(&none);
    let stored = user_message::display_text(content, Some(hook), false, None);
    let text = draw::laid(&stored);
    let group = user_message::hook_group_context(content, Some(hook));
    let card = hook_card::card(source, group, cx.global::<Translator>());
    let compact = f32::from(u8::from(widths.compact));
    let sizes = |size: f32| text_size(size, cx) - compact;
    let (small, title, large) = (sizes(12.0), sizes(13.0), sizes(14.0));
    let column = COLUMN.min(widths.message);
    let inner = (column - INSET).max(0.0);
    // The fold: clean stored text, past 14 rows, keeps 10.
    let (look, line) = style(large, 1.6, window);
    let room = css(inner).to_pixels(window.rem_size());
    let clean = source.get("contentFormat").and_then(Js::str) == Some("user-text");
    let measured = clean && collapse::may_exceed(&stored, collapse::LONG);
    let all = draw::pre(&text, (0, None), (&look, room), window);
    let folds = measured && all.total > collapse::LONG;
    let open = user_row::unfolded(key, cx);
    let shown = match folds && !open {
        true => draw::pre(&text, (0, Some(KEPT)), (&look, room), window),
        false => all,
    };
    let t = cx.global::<Translator>();
    let fold = folds.then(|| {
        let label = match open {
            true => "chat.userMessage.collapseLongMessage",
            false => "chat.userMessage.expandLongMessage",
        };
        div().mt(css(8.0)).child(Disclosure {
            name: format!("{key}-hook-fold"),
            fold: key.to_owned(),
            label: t.t(label, &[]),
            open,
            chevron: false,
            text: (small, 1.6),
        })
    });
    let appearance = cx.global::<Appearance>();
    let selector = format!("{key}-hook-text");
    let rows = draw::lit(&selector, (&text, shown), (&look, line), window);
    let said = div().debug_selector(move || selector).children(rows);
    let said = div()
        .whitespace_nowrap()
        .text_size(css(large))
        .line_height(line)
        .text_color(appearance.token("text-primary"))
        .child(said)
        .children(fold);
    let attached = card.context.as_ref().map(|attached| {
        let fold = format!("{key}-context");
        let open = user_row::unfolded(&fold, cx);
        let toggle = Disclosure {
            name: format!("{key}-hook-context"),
            fold,
            label: attached.label.clone(),
            open,
            chevron: true,
            text: (small, 1.5),
        };
        let body = context(key, attached, inner, small, window, cx);
        let body = folding(key, open, body, window, cx);
        div().flex().flex_col().child(toggle).children(body)
    });
    let appearance = cx.global::<Appearance>();
    let body = div()
        .flex()
        .flex_col()
        .gap(css(8.0))
        .px(css(14.0))
        .pb(css(12.0))
        .child(said)
        .children(attached);
    let selector = format!("{key}-hook");
    let card = div()
        .debug_selector(move || selector)
        .w_full()
        .overflow_hidden()
        .rounded(css(12.0))
        .border(css(1.0))
        .border_color(appearance.token("msg-tool-card-border"))
        .bg(wallpaper::glass(appearance.token("msg-tool-card-bg"), cx))
        .pt(css(10.0))
        .child(head(key, &card, title, cx))
        .child(body);
    let labels = source_labels::row(key, message, Over::Hook, cx);
    let files = attachments::files(message);
    let files =
        (!files.is_empty()).then(|| attachments::row(key, &files, (column, true), window, cx));
    let images = images::elements(key, &images::images(message), window, cx);
    let column = div()
        .flex()
        .flex_col()
        .items_start()
        .gap(css(8.0))
        .w_full()
        .max_w(css(COLUMN))
        .children(labels)
        .children(images)
        .children(files)
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
