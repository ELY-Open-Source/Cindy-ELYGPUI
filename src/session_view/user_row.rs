//! `UserMessage`: an ordinary message's bubble, its text and the long message's fold.

use ely_gpui_component::primitives::IconName;
use gpui::{
    AnyElement, App, BoxShadow, ElementId, InteractiveElement as _, IntoElement,
    ParentElement as _, RenderOnce, Role, StatefulInteractiveElement as _, Styled as _, Window,
    div, point, prelude::FluentBuilder as _, px, relative,
};

use super::{
    SessionView, attachments,
    flow::draw::{self, Island, Look, Ranges},
    images,
    width::Widths,
};
use crate::{
    appearance_settings::text_size,
    chat_message::merge::{Fields, get},
    chat_quotes::{self, Segment},
    i18n::Translator,
    lucide, motion, press,
    rich_answer::js::{Js, truthy, utf16_len},
    theme::Appearance,
    theme_import::color::js_trim,
    transition::{self, Curve},
    units::{css, window_px},
    user_message::{
        self, collapse,
        sent::{self, Pasted, Reference, Span},
    },
    wallpaper,
};

/// `AUTOMATION_USER_MESSAGE_VISUAL_LINE_THRESHOLD`.
const AUTOMATION: usize = 4;
/// The column's `max-w-[488px]`.
const COLUMN: f32 = 488.0;
/// The bubble's `px-4` and its border, on both sides.
const INSET: f32 = 34.0;

/// How a message folds: past `threshold` lines it keeps `kept`.
struct Fold {
    threshold: usize,
    kept: usize,
}

/// `isRealAutomationOrigin` picks the automation fold.
fn fold(origin: Option<&Js>) -> Fold {
    let field = |key| origin.and_then(Js::obj)?.get(key)?.str();
    let hook = field("scheduleId").is_some_and(|id| id.starts_with("hook:"));
    match field("kind") == Some("scheduler") && !hook {
        true => Fold {
            threshold: AUTOMATION,
            kept: 3,
        },
        false => Fold {
            threshold: collapse::LONG,
            kept: 10,
        },
    }
}

fn unfolded(key: &str, cx: &App) -> bool {
    let content = cx
        .global::<SessionView>()
        .shown
        .as_ref()
        .and_then(|s| s.content.as_ref());
    content.is_some_and(|content| content.unfolded.contains(key))
}

/// `setLongMessageExpanded((expanded) => !expanded)`.
pub(super) fn toggle(key: &str, cx: &mut App) {
    let shown = cx.global_mut::<SessionView>().shown.as_mut();
    let Some(content) = shown.and_then(|s| s.content.as_mut()) else {
        return;
    };
    let open = !content.unfolded.remove(key);
    if open {
        content.unfolded.insert(key.to_owned());
    } else {
        super::link_chip::folded(key, cx);
    }
    log::info!(target: "sessionView", "message {key} unfolded: {open}");
    cx.refresh_windows();
}

/// The fold's button under the text.
#[derive(IntoElement)]
struct Toggle {
    key: String,
    open: bool,
    compact: bool,
}

impl RenderOnce for Toggle {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = ElementId::Name(format!("{}-fold", self.key).into());
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
            appearance.token("text-secondary"),
            appearance.token("msg-user-text"),
        );
        let ring = BoxShadow {
            color: appearance.token("focus-ring-soft"),
            offset: point(px(0.0), px(0.0)),
            blur_radius: px(0.0),
            spread_radius: window_px(2.0, cx),
            inset: false,
        };
        let label = match self.open {
            true => "chat.userMessage.collapseLongMessage",
            false => "chat.userMessage.expandLongMessage",
        };
        let label = cx.global::<Translator>().t(label, &[]);
        let curve = Curve {
            length: motion::length(motion::FAST, cx),
            easing: motion::ease_move,
            lerp: transition::mix_srgb,
        };
        let color = if state.hovered { lit } else { rest };
        let color = transition::keyed(child("color"), color, curve, window, cx);
        let chevron = match self.open {
            true => IconName::ChevronUp,
            false => IconName::ChevronDown,
        };
        let size = text_size(12.0, cx) - f32::from(u8::from(self.compact));
        let (selector, key) = (format!("{}-fold", self.key), self.key);
        let el = div()
            .id(id)
            .debug_selector(move || selector)
            .track_focus(&focus)
            .role(Role::Button)
            .aria_expanded(self.open)
            .mt(css(8.0))
            .flex()
            .items_center()
            .gap(css(4.0))
            .rounded(css(9999.0))
            .px(css(4.0))
            .py(css(2.0))
            .cursor_pointer()
            .text_size(css(size))
            .font_weight(gpui::FontWeight::MEDIUM)
            .line_height(css(20.0))
            .text_color(color)
            .focus_visible(|s| s.shadow(vec![ring]))
            .on_click(move |_, _, cx| toggle(&key, cx))
            .child(label)
            .child(lucide::icon(chevron.svg(), 13.0, 2.0).text_color(color));
        press::listen(el, &press, false)
    }
}

/// Where the shown text starts in the content, in UTF-16 units: `content.indexOf`.
fn source(content: &str, shown: &str) -> Option<usize> {
    content.find(shown).map(|at| utf16_len(&content[..at]))
}

/// `projectSentRanges` over the stored ranges: each moved into the text at `start`, or dropped.
fn ranges(message: &Fields, start: Option<usize>, length: usize) -> Ranges {
    let list = |key| get(message, key).and_then(Js::arr);
    let moved = |span| sent::within(span, start, length);
    let pasted = list("pastedTextRanges").unwrap_or_default().iter();
    let pasted = pasted.map(Pasted::read).filter_map(|pasted| {
        let span = moved(pasted.span)?;
        Some(Pasted { span, ..pasted })
    });
    let references = list("agentReferences").unwrap_or_default().iter();
    let references = references.map(Reference::read).filter_map(|reference| {
        let span = moved(reference.span)?;
        Some(Reference { span, ..reference })
    });
    let slash = |all: &[Js]| all.iter().map(Span::read).filter_map(moved).collect();
    Ranges {
        pasted: pasted.collect(),
        slash: list("slashCommandRanges").map(slash),
        references: references.collect(),
    }
}

/// The row of an ordinary message; the Orca and hook forms come with 9.5g.
pub fn element(
    key: &str,
    message: &Fields,
    widths: Widths,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyElement> {
    let content = get(message, "content")
        .and_then(Js::str)
        .unwrap_or_default();
    if truthy(get(message, "hookSource")) || user_message::orca(content).is_some() {
        return None;
    }
    let origin = get(message, "automationOrigin");
    let forwarded = origin.and_then(Js::obj).and_then(|o| o.get("kind")?.str()) == Some("session");
    let display = user_message::display_text(content, None, forwarded, None);
    let segments = (get(message, "quotesEncoded") == Some(&Js::Bool(true)))
        .then(|| chat_quotes::segments(&display));
    let body = match &segments {
        Some(segments) => chat_quotes::join_text(segments),
        None => display.clone(),
    };
    let quoted = segments.as_deref().filter(|all| {
        let quote = |segment: &Segment| matches!(segment, Segment::Quote(_));
        all.iter().any(quote)
    });
    let room = COLUMN.min(widths.message);
    let files = attachments::files(message);
    let files = (!files.is_empty()).then(|| attachments::row(key, &files, room, window, cx));
    let images = images::elements(key, &images::images(message), window, cx);
    let mut over: Vec<AnyElement> = images.into_iter().chain(files).collect();
    // A message with no text and no quote draws no bubble.
    if quoted.is_none() && js_trim(&body).is_empty() {
        return (!over.is_empty()).then(|| column(over));
    }
    let fold = fold(origin);
    let compact = f32::from(u8::from(widths.compact));
    let size = text_size(15.0, cx) - compact;
    let rem = window.rem_size();
    let width = css((COLUMN.min(widths.message) - INSET).max(0.0)).to_pixels(rem);
    let style = draw::style(css(size).to_pixels(rem), window);
    let source = source(content, &display);
    // Each text island takes the ranges that lie in it; one text takes them all.
    let islands: Vec<Island> = match quoted {
        Some(segments) => {
            let starts = sent::text_starts(&display, segments);
            let pairs = segments.iter().zip(starts);
            let island = pairs.map(|(segment, start)| match segment {
                Segment::Quote(quote) => Island::Quote(quote),
                Segment::Text(text) => {
                    let start = source.zip(start).map(|(source, start)| source + start);
                    Island::Text(text, ranges(message, start, utf16_len(text)))
                }
            });
            island.collect()
        }
        None => vec![Island::Text(
            &body,
            ranges(message, source, utf16_len(&body)),
        )],
    };
    // The mirror reads chips as their words while the text is the message's own.
    let plain = match body == display {
        true => {
            let t = cx.global::<Translator>();
            let label = |reference: &Reference| sent::label(reference, t);
            let [Island::Text(_, ranges)] = islands.as_slice() else {
                unreachable!("a text with quote chips is not the display text")
            };
            sent::inline_plain(&body, &ranges.pasted, &ranges.references, label)
        }
        false => body.clone(),
    };
    let folds = collapse::may_exceed(&plain, fold.threshold)
        && draw::count(&plain, &style, width, window) > fold.threshold;
    let open = unfolded(key, cx);
    let look = Look {
        key,
        style,
        chip: text_size(12.0, cx) - compact,
        width,
        kept: (folds && !open).then_some(fold.kept),
    };
    let text = draw::text(&islands, &look, window, cx);
    let appearance = cx.global::<Appearance>();
    let selector = format!("{key}-bubble");
    let text_selector = format!("{key}-text");
    let text = div().debug_selector(move || text_selector).child(text);
    // `ChatBubble`'s user face over `CHAT_BODY_CLASS`.
    let bubble = div()
        .debug_selector(move || selector)
        .relative()
        .min_w_0()
        .max_w_full()
        .rounded(css(12.0))
        .border(css(1.0))
        .border_color(appearance.token("msg-user-border"))
        .bg(wallpaper::glass(appearance.token("msg-user-bg"), cx))
        .px(css(16.0))
        .py(css(12.0))
        .text_size(css(size))
        .line_height(relative(1.6))
        .text_color(appearance.token("msg-user-text"))
        .child(text)
        .when(folds, |el| {
            el.child(div().flex().child(Toggle {
                key: key.to_owned(),
                open,
                compact: widths.compact,
            }))
        });
    over.push(bubble.into_any_element());
    Some(column(over))
}

/// The message's column, set right: what stands over the bubble, then the bubble.
fn column(parts: Vec<AnyElement>) -> AnyElement {
    let column = div()
        .flex()
        .flex_col()
        .items_end()
        .gap(css(8.0))
        .min_w_0()
        .max_w(css(COLUMN))
        .children(parts);
    div()
        .flex()
        .w_full()
        .justify_end()
        .child(column)
        .into_any_element()
}

#[cfg(test)]
pub(super) mod tests;
