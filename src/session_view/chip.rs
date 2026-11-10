//! `InlineReferenceChip`'s inert face in a sent message, with its tip.

use std::time::Duration;

use ely_gpui_component::{primitives::IconName, theme::ActiveTheme as _};
use gpui::{
    AnyElement, App, ElementId, Font, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, SharedString, StatefulInteractiveElement as _, Styled as _,
    TextRun, TextStyle, Window, div, prelude::FluentBuilder as _,
};

use crate::{
    appearance_settings::text_size,
    chat_quotes::{self, Quote},
    file_presentation, lucide,
    theme::Appearance,
    theme_import::color::js_space,
    tip,
    units::{css, to_css},
    user_message::sent::{Kind, Part, Reference},
};

/// The border and `px-2`, on both sides.
const INSET: f32 = 18.0;
/// The icon's 14px box and `gap-1.5`.
const ICON: f32 = 20.0;

pub struct Chip {
    label: SharedString,
    /// The label as given, for `ariaLabel`.
    name: SharedString,
    icon: Option<&'static [u8]>,
    tip: SharedString,
    face: Face,
    /// The slash chip keeps the message's own colour.
    command: bool,
    /// `ariaLabel`.
    named: bool,
    /// A quote between text islands: `mx-1`, and neither `top-[-1px]` nor `-my-[1px]`.
    island: bool,
}

/// How the tip sets its text.
#[derive(Clone)]
enum Face {
    Plain,
    /// `tooltipMono`: a path.
    Mono,
    /// The pasted text itself, in a frame that scrolls.
    Pasted,
    /// `QuoteChip`'s tip: the quote, then its source's path and label.
    Quote(Option<(SharedString, SharedString)>),
}

/// `FileTypeIcon`; Ely's `FileVideoCamera` is not lucide 0.468's `FileVideo`.
fn file_icon(name: &str) -> &'static [u8] {
    match file_presentation::glyph(file_presentation::kind(name, "")) {
        "FileCode" => IconName::FileCode.svg(),
        "FileText" => IconName::FileText.svg(),
        "FileSpreadsheet" => IconName::FileSpreadsheet.svg(),
        "FileChartColumn" => lucide::FILE_CHART_COLUMN,
        "FileImage" => IconName::FileImage.svg(),
        "FileAudio" => IconName::FileAudio.svg(),
        "FileVideo" => lucide::FILE_VIDEO,
        "FileArchive" => IconName::FileArchive.svg(),
        "Database" => IconName::Database.svg(),
        "File" => IconName::File.svg(),
        other => panic!("no icon for the glyph {other}"),
    }
}

fn chip(label: &str, icon: Option<&'static [u8]>, tip: &str, face: Face) -> Chip {
    // `truncate` is `white-space: nowrap`: white space collapses to single spaces.
    let drawn: Vec<&str> = label
        .split([' ', '\t', '\n', '\r', '\x0c'])
        .filter(|word| !word.is_empty())
        .collect();
    Chip {
        label: drawn.join(" ").into(),
        name: label.to_owned().into(),
        icon,
        tip: tip.to_owned().into(),
        face,
        command: false,
        named: false,
        island: false,
    }
}

/// The slash chip, of the heuristic or of a stored range.
pub fn command(label: &str) -> Chip {
    Chip {
        command: true,
        ..chip(label, None, label, Face::Plain)
    }
}

/// The chip a part of `renderContentWithoutPastedText` draws; text draws none.
pub fn of(part: &Part) -> Option<Chip> {
    Some(match part {
        Part::Text(_) => return None,
        Part::Command(label) => command(label),
        Part::Agent(name) => chip(name, Some(IconName::Sparkles.svg()), name, Face::Plain),
        Part::Dir { label, path } => chip(label, Some(IconName::Folder.svg()), path, Face::Mono),
        Part::File { name, path } => Chip {
            named: true,
            ..chip(name, Some(file_icon(name)), path, Face::Mono)
        },
    })
}

/// A pasted run's chip: its saved words, and the pasted text as the tip.
pub fn pasted(display: &str, text: &str) -> Chip {
    Chip {
        named: true,
        ..chip(display, Some(IconName::FileText.svg()), text, Face::Pasted)
    }
}

/// `QuoteChip`: the quote on one line; `island` sets it between text islands.
pub fn quote(quote: &Quote, island: bool) -> Chip {
    let source = chat_quotes::source_label(quote).map(|label| {
        let path = quote.source_path.clone().unwrap_or_default();
        (path.into(), label.into())
    });
    // `compactText`: JavaScript's white space, which is wider than CSS's.
    let compact: Vec<&str> = quote.text.split(js_space).collect();
    Chip {
        name: quote.text.clone().into(),
        named: true,
        island,
        ..chip(
            &compact.join(" "),
            Some(lucide::MESSAGE_SQUARE_QUOTE),
            &quote.text,
            Face::Quote(source),
        )
    }
}

/// `SentAgentReferenceChip`'s inert face; a message reference unfolded is 9.5l's link chip.
pub fn reference(reference: &Reference, label: &str, folded: bool) -> Option<Chip> {
    let icon = match reference.kind {
        Kind::Message { .. } if folded => {
            let said = Quote {
                text: label.to_owned(),
                ..Default::default()
            };
            return Some(quote(&said, false));
        }
        Kind::Message { .. } => return None,
        Kind::Session { .. } => IconName::CornerDownRight.svg(),
        Kind::Project { .. } => IconName::FolderOpen.svg(),
        Kind::BrowserTab { .. } => lucide::EARTH,
        Kind::DesktopWindow { .. } => IconName::Monitor.svg(),
        Kind::Bot { .. } => IconName::Bot.svg(),
        Kind::Todo { .. } => IconName::ListTodo.svg(),
        Kind::PluginResource { .. } => lucide::PLUG,
    };
    Some(Chip {
        named: true,
        ..chip(label, Some(icon), label, Face::Plain)
    })
}

/// `QuoteChip`'s tip: the whole quote in its marks, then where it came from.
fn quote_tip(
    text: &SharedString,
    source: Option<&(SharedString, SharedString)>,
    cx: &App,
) -> AnyElement {
    let muted = cx.global::<Appearance>().token("text-tertiary");
    let size = text_size(12.0, cx);
    let source = source.map(|(path, label)| {
        div()
            .flex()
            .items_center()
            .gap(css(4.0))
            .min_w_0()
            .text_size(css(text_size(11.0, cx)))
            .text_color(muted)
            .child(lucide::icon(file_icon(path), 12.0, 2.0).text_color(muted))
            .child(div().truncate().child(label.clone()))
    });
    div()
        .flex()
        .flex_col()
        .gap(css(4.0))
        .child(
            div()
                .text_size(css(size))
                .line_height(css(size * 1.5))
                .child(format!("\u{201c}{text}\u{201d}")),
        )
        .children(source)
        .into_any_element()
}

/// `max-w-[min(240px,55vw)]`.
fn most(window: &Window, cx: &App) -> Pixels {
    let most = (to_css(window.viewport_size().width, cx) * 0.55).min(240.0);
    css(most).to_pixels(window.rem_size())
}

impl Chip {
    /// The room the chip takes in a row at a `size` label; layout sizes the face itself.
    pub fn width(&self, size: f32, style: &TextStyle, window: &Window, cx: &App) -> Pixels {
        let rem = window.rem_size();
        let run = TextRun {
            font: Font {
                weight: FontWeight::NORMAL,
                ..style.font()
            },
            ..style.to_run(self.label.len())
        };
        let size_px = css(size).to_pixels(rem);
        let label = window
            .text_system()
            .shape_line(self.label.clone(), size_px, &[run], None);
        let icon = self.icon.map_or(0.0, |_| ICON);
        let face = (label.width + css(INSET + icon).to_pixels(rem)).min(most(window, cx));
        face + css(self.margin() * 2.0).to_pixels(rem)
    }

    fn margin(&self) -> f32 {
        f32::from(u8::from(self.island)) * 4.0
    }

    /// The margin box's height: `leading-5`, `py-0.5`, the border, less `-my-[1px]`.
    pub fn height(&self) -> f32 {
        24.0 + f32::from(u8::from(self.island)) * 2.0
    }

    /// The chip, its margin box `top` under the row's top.
    pub fn element(
        &self,
        id: String,
        top: Pixels,
        size: f32,
        window: &Window,
        cx: &App,
    ) -> AnyElement {
        let appearance = cx.global::<Appearance>();
        let color = appearance.token(match self.command {
            true => "msg-user-text",
            false => "text-primary",
        });
        let (selector, label, tip) = (id.clone(), self.label.clone(), self.tip.clone());
        let name = self.name.clone();
        div()
            .id(ElementId::Name(id.into()))
            .debug_selector(move || selector)
            .when(self.named, |el| el.aria_label(name))
            .map(|el| match self.island {
                true => el.mt(top).mx(css(4.0)),
                false => {
                    let lift = css(1.0).to_pixels(window.rem_size());
                    el.relative().top(css(-1.0)).mt(top - lift)
                }
            })
            .flex_none()
            .max_w(most(window, cx))
            .flex()
            .items_center()
            .gap(css(6.0))
            .rounded(css(9999.0))
            .border(css(1.0))
            .border_color(appearance.token("border-default"))
            .bg(appearance.token("surface-chip"))
            .px(css(8.0))
            .py(css(2.0))
            .text_size(css(size))
            .font_weight(FontWeight::NORMAL)
            .line_height(css(20.0))
            .text_color(color)
            .when_some(self.icon, |el, icon| {
                el.child(lucide::icon(icon, 14.0, 2.0).text_color(color))
            })
            .child(div().min_w_0().truncate().child(label.clone()))
            // `Tip` mounts nothing for empty text.
            .map(|el| match (tip.is_empty(), self.face.clone()) {
                (_, Face::Quote(source)) => el.hoverable_tooltip(tip::framed(move |_, cx| {
                    quote_tip(&tip, source.as_ref(), cx)
                })),
                (true, _) => el,
                (false, Face::Plain) => el.tooltip(tip::tip(tip)),
                (false, Face::Mono) => el.tooltip(tip::mono(tip)),
                (false, Face::Pasted) => el.hoverable_tooltip(tip::framed(move |_, cx| {
                    let family = cx.theme().mono_family.clone();
                    let text = tip.clone();
                    div().font_family(family).child(text).into_any_element()
                })),
            })
            .tooltip_show_delay(Duration::from_millis(300))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests;
