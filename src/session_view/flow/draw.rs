//! The bubble's text drawn row by row: words, links and chips in one line.

use std::ops::Range;

use gpui::{
    AnyElement, App, FontWeight, IntoElement, ParentElement as _, Pixels, ShapedLine, SharedString,
    Styled as _, TextStyle, Window, div, prelude::FluentBuilder as _, relative,
};

use super::{Atom, CHIP, atoms, clamp, rows};
use crate::{
    chat_quotes::Quote,
    i18n::Translator,
    linkify,
    session_view::{
        chip::{self, Chip, Stand},
        link_chip::{self, Act, Found, Want},
        links::{self, Link, Shared},
    },
    units::css,
    user_message::sent::{self, Kind, Part, Pasted, Reference, Span, Token},
};

/// How the text is drawn; `chip` is the chip label's size in CSS pixels.
pub struct Look<'a> {
    pub key: &'a str,
    pub style: TextStyle,
    pub chip: f32,
    pub width: Pixels,
    /// `line-clamp-N` while the message is folded; a folded message draws no link.
    pub kept: Option<usize>,
}

/// The bubble's text style at `size`; no element between the window and the text sets a face.
pub fn style(size: Pixels, window: &Window) -> TextStyle {
    let mut style = window.text_style();
    (style.font_size, style.line_height) = (size.into(), relative(1.6));
    style
}

/// `text` shaped as one line of `style`.
pub fn shape(text: &str, style: &TextStyle, window: &Window) -> ShapedLine {
    let size = style.font_size.to_pixels(window.rem_size());
    let run = style.to_run(text.len());
    window
        .text_system()
        .shape_line(text.to_owned().into(), size, &[run], None)
}

/// One logical line: its text with a `CHIP` for each chip, and the chips and links by byte.
#[derive(Default)]
struct Line {
    text: String,
    chips: Vec<(usize, Chip)>,
    links: Vec<Link>,
    /// Image paths: underlined, and `break-all`.
    paths: Vec<Range<usize>>,
}

/// The message's saved ranges in the drawn text's own units; no `slash` leaves the heuristic.
#[derive(Default)]
pub struct Ranges {
    pub pasted: Vec<Pasted>,
    pub slash: Option<Vec<Span>>,
    pub references: Vec<Reference>,
}

/// The lines being read, and how many links they hold so far.
struct Reading<'a> {
    lines: Vec<Line>,
    links: usize,
    linked: bool,
    known: (&'a Found, &'a Translator),
    wants: Vec<Want>,
}

impl Reading<'_> {
    fn line(&mut self) -> &mut Line {
        self.lines.last_mut().expect("a line is open")
    }

    fn chip(&mut self, chip: Chip) {
        let line = self.line();
        line.chips.push((line.text.len(), chip));
        line.text.push(CHIP);
    }

    /// Words that draw as they stand, line by line.
    fn plain(&mut self, text: &str) {
        for (nth, piece) in text.split('\n').enumerate() {
            if nth > 0 {
                self.lines.push(Line::default());
            }
            self.line().text.push_str(&piece.replace('\r', ""));
        }
    }

    /// `renderContentWithoutPastedText`'s nodes for a text token.
    fn text(&mut self, text: &str, legacy: bool) {
        for part in sent::parts(text, legacy) {
            if part == Part::Text("\n".to_owned()) {
                self.lines.push(Line::default());
                continue;
            }
            let Part::Text(text) = &part else {
                self.chip(chip::of(&part).expect("a part that is no text draws a chip"));
                continue;
            };
            // `renderTextWithLinks`: each piece is read alone; Blink draws no CR.
            let linked = self.linked;
            let mut from = 0;
            for found in linkify::find(text).into_iter().filter(|_| linked) {
                let range = found.range.clone();
                self.line()
                    .text
                    .push_str(&text[from..range.start].replace('\r', ""));
                let start = self.line().text.len();
                from = range.start;
                match &found.link {
                    linkify::Link::Url => {
                        let link = Link {
                            nth: self.links,
                            range: start..start + range.len(),
                            url: text[range].to_owned().into(),
                        };
                        self.line().links.push(link);
                        self.links += 1;
                    }
                    linkify::Link::Img => self.line().paths.push(start..start + range.len()),
                    linkify::Link::Session(to) => {
                        let label = to.label.as_deref();
                        let chip = link_chip::session(&to.href, label, self.known, &mut self.wants);
                        self.link(chip, label.unwrap_or(&to.href));
                        from = range.end;
                    }
                    linkify::Link::Project(to) => {
                        let label = to.label.as_deref();
                        self.link(
                            link_chip::project(&to.href, label),
                            label.unwrap_or(&to.href),
                        );
                        from = range.end;
                    }
                }
            }
            self.line().text.push_str(&text[from..].replace('\r', ""));
        }
    }

    /// A link's chip, or the words of one that names nothing.
    fn link(&mut self, chip: Option<Chip>, words: &str) {
        match chip {
            Some(chip) => self.chip(chip),
            None => self.plain(words),
        }
    }

    /// `SentAgentReferenceChip`: the link chips of an unfolded message, else the inert face.
    fn reference(&mut self, reference: &Reference) {
        let chip = match &reference.kind {
            _ if !self.linked => None,
            Kind::Message { text } => Some((text.as_deref(), true)),
            Kind::Session { title } => Some((title.as_deref(), true)),
            Kind::Project { name, .. } => Some((Some(name.as_str()), false)),
            _ => None,
        };
        let href = &reference.href;
        match chip {
            Some((label, true)) => {
                let chip = link_chip::session(href, label, self.known, &mut self.wants);
                self.link(chip, label.unwrap_or(href));
            }
            Some((label, false)) => {
                self.link(link_chip::project(href, label), label.unwrap_or(href))
            }
            None => {
                let label = sent::label(reference, self.known.1);
                self.chip(chip::reference(reference, &label));
            }
        }
    }
}

/// A stretch of the bubble: the user's text with its ranges, or a quote between two.
pub enum Island<'a> {
    Text(&'a str, Ranges),
    Quote(&'a Quote),
}

/// `renderContent`'s nodes line by line; a final line feed opens no line.
fn lines(islands: &[Island], linked: bool, known: (&Found, &Translator)) -> (Vec<Line>, Vec<Want>) {
    let mut reading = Reading {
        lines: vec![Line::default()],
        links: 0,
        linked,
        known,
        wants: Vec::new(),
    };
    for island in islands {
        let (body, ranges) = match island {
            Island::Text(body, ranges) => (body, ranges),
            Island::Quote(quote) => {
                reading.chip(chip::quote(quote, Stand::Island));
                continue;
            }
        };
        let slash = ranges.slash.as_deref().unwrap_or_default();
        for token in sent::tokens(body, &ranges.pasted, slash, &ranges.references) {
            match &token {
                Token::Text(text) => reading.text(text, ranges.slash.is_none()),
                Token::Slash(text) => reading.chip(chip::command(&sent::slash_label(text))),
                Token::Pasted { text, display } => reading.chip(chip::pasted(display, text)),
                Token::Reference { index, .. } => reading.reference(&ranges.references[*index]),
            }
        }
    }
    let mut out = reading.lines;
    if out.len() > 1 && out.last().is_some_and(|line| line.text.is_empty()) {
        out.pop();
    }
    (out, reading.wants)
}

/// The line's atoms, each chip as wide as its face, and the rows they break into.
pub fn broken(
    (text, paths): (&str, &[Range<usize>]),
    chips: &[(usize, Pixels)],
    (style, width): (&TextStyle, Pixels),
    window: &Window,
) -> (Vec<Atom>, Vec<Range<usize>>) {
    let mut atoms = atoms(&shape(text, style, window));
    for (at, width) in chips {
        let atom = atoms.iter_mut().find(|atom| atom.at == *at);
        atom.expect("a chip's stand-in is shaped").advance = f32::from(*width);
    }
    let rows = rows(text, &atoms, width.into(), paths);
    (atoms, rows)
}

/// Blink lays out no CR and no FF in `pre-wrap` (`logs/review/9.5q-r3-blink-controls.log`).
pub fn laid(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, '\r' | '\x0c'))
        .collect()
}

/// Rows of text: each with its leading semibold bytes, and how many there are unclamped.
pub struct Pre {
    pub rows: Vec<(String, usize)>,
    pub total: usize,
}

/// `pre-wrap` rows of laid text; its first `bold` bytes are semibold, `kept` is `line-clamp`.
pub fn pre(
    text: &str,
    (bold, kept): (usize, Option<usize>),
    (style, width): (&TextStyle, Pixels),
    window: &Window,
) -> Pre {
    let size = style.font_size.to_pixels(window.rem_size());
    let shape = |line: &str, bold: usize| {
        let mut strong = style.to_run(bold);
        strong.font.weight = FontWeight::SEMIBOLD;
        let runs = [strong, style.to_run(line.len() - bold)];
        let line = SharedString::from(line.to_owned());
        window.text_system().shape_line(line, size, &runs, None)
    };
    // An empty `pre` holds no line; a final line feed opens none.
    let body = text.strip_suffix('\n').unwrap_or(text);
    let (mut all, mut at) = (Vec::new(), 0);
    for line in body.split('\n').filter(|_| !text.is_empty()) {
        let strong = bold.saturating_sub(at).min(line.len());
        let atoms = atoms(&shape(line, strong));
        let broken = rows(line, &atoms, width.into(), &[]);
        all.extend(broken.into_iter().map(|row| (line, strong, row)));
        at += line.len() + 1;
    }
    let total = all.len();
    let cut = kept.filter(|kept| total > *kept);
    all.truncate(cut.unwrap_or(total));
    let last = all.len().saturating_sub(1);
    let row = |(nth, (line, strong, row)): (usize, (&str, usize, Range<usize>))| {
        let held = strong.saturating_sub(row.start).min(row.len());
        if cut.is_none() || nth != last {
            return (line[row].to_owned(), held);
        }
        let dots = "\u{2026}";
        let room = f32::from(shape(dots, 0).width);
        let end = clamp(
            &atoms(&shape(line, strong)),
            row.clone(),
            room,
            width.into(),
        );
        (
            format!("{}{dots}", &line[row.start..end]),
            held.min(end - row.start),
        )
    };
    Pre {
        rows: all.into_iter().enumerate().map(row).collect(),
        total,
    }
}

/// The plain text's rows at `width`: `pre-wrap` with no chip and no link.
fn plain(body: &str, style: &TextStyle, width: Pixels, window: &Window) -> Vec<String> {
    let text = body.replace('\r', "");
    let text = text.strip_suffix('\n').unwrap_or(&text);
    let rows = |line: &str| {
        let rows = broken((line, &[]), &[], (style, width), window).1;
        let words = rows.into_iter().map(|row| line[row].to_owned());
        words.collect::<Vec<_>>()
    };
    text.split('\n').flat_map(rows).collect()
}

/// The rows the plain text takes at `width`, as Cindy's hidden mirror measures them.
pub fn count(body: &str, style: &TextStyle, width: Pixels, window: &Window) -> usize {
    plain(body, style, width, window).len()
}

/// Where a row's parts sit: a chip's middle lies half an x-height over the baseline.
struct Metrics {
    line: Pixels,
    middle: Pixels,
}

fn metrics(style: &TextStyle, window: &Window) -> Metrics {
    let rem = window.rem_size();
    let size = style.font_size.to_pixels(rem);
    // As gpui's text snaps its own line.
    let line = window.pixel_snap(style.line_height.to_pixels(size.into(), rem));
    let probe = shape("x", style, window);
    let baseline = (line - probe.ascent - probe.descent) / 2.0 + probe.ascent;
    let system = window.text_system();
    let x_height = system.x_height(system.resolve_font(&style.font()), size);
    Metrics {
        line,
        middle: baseline - x_height / 2.0,
    }
}

/// The words of `range`, then `tail`, with the links that reach into them.
fn words(
    line: &Line,
    range: Range<usize>,
    tail: &str,
    key: &str,
    shared: Option<&Shared>,
    window: &Window,
) -> Option<AnyElement> {
    let text = format!("{}{tail}", &line.text[range.clone()]);
    // The stretch of `span` these words hold, in their own bytes.
    let within = |span: &Range<usize>| {
        let (start, end) = (span.start.max(range.start), span.end.min(range.end));
        (start < end).then(|| start - range.start..end - range.start)
    };
    let held = |link: &Link| {
        Some(Link {
            nth: link.nth,
            range: within(&link.range)?,
            url: link.url.clone(),
        })
    };
    let held: Vec<Link> = line.links.iter().filter_map(held).collect();
    let paths: Vec<Range<usize>> = line.paths.iter().filter_map(within).collect();
    match shared.filter(|_| !held.is_empty()) {
        _ if text.is_empty() => None,
        Some(shared) => Some(links::piece(key, (text, paths), held, shared, window)),
        None if paths.is_empty() => Some(SharedString::from(text).into_any_element()),
        None => Some(links::marked(text, paths, window).into_any_element()),
    }
}

/// The message's text: `renderContent` over each island, broken at `look.width`.
pub fn text(islands: &[Island], look: &Look, window: &mut Window, cx: &mut App) -> AnyElement {
    let known = (link_chip::held(cx), cx.global::<Translator>());
    let (lines, wants) = lines(islands, look.kept.is_none(), known);
    if !wants.is_empty() {
        cx.defer(move |cx| link_chip::fetch(wants, cx));
    }
    let links = lines.iter().map(|line| line.links.len()).sum();
    // The links and the chips that act share one menu.
    let acts = |line: &Line| line.chips.iter().any(|(_, chip)| chip.act().is_some());
    let menu = links > 0 || lines.iter().any(acts);
    let shared = menu.then(|| links::shared(look.key, links, window, cx));
    let (mut all, mut shaped, mut chips, mut wrapped) = (Vec::new(), Vec::new(), 0, false);
    for (n, line) in lines.iter().enumerate() {
        let width =
            |(at, chip): &(usize, Chip)| (*at, chip.width(look.chip, &look.style, window, cx));
        let widths: Vec<(usize, Pixels)> = line.chips.iter().map(width).collect();
        let text = (line.text.as_str(), line.paths.as_slice());
        let (atoms, rows) = broken(text, &widths, (&look.style, look.width), window);
        wrapped |= rows.len() > 1;
        all.extend(rows.into_iter().map(|row| (n, row)));
        shaped.push((atoms, chips));
        chips += line.chips.len();
    }
    let cut = look.kept.filter(|kept| all.len() > *kept);
    all.truncate(cut.unwrap_or(all.len()));
    let metrics = metrics(&look.style, window);
    let dots = "\u{2026}";
    let ellipsis = shape(dots, &look.style, window);
    let last = all.len() - 1;
    let row = |(at, (n, row)): (usize, (usize, Range<usize>))| {
        let (line, (atoms, first)): (&Line, _) = (&lines[n], &shaped[n]);
        // `line-clamp` ends the last kept row with an ellipsis.
        let (end, tail) = match cut.is_some() && at == last {
            true => {
                let room = (f32::from(ellipsis.width), f32::from(look.width));
                (clamp(atoms, row.clone(), room.0, room.1), dots)
            }
            false => (row.end, ""),
        };
        let held = |(_, (at, _)): &(usize, &(usize, Chip))| (row.start..end).contains(at);
        let held: Vec<_> = line.chips.iter().enumerate().filter(held).collect();
        let (mut parts, mut from) = (Vec::new(), row.start);
        // Blink's line box: the text's line and each chip's margin box about the middle.
        let rem = window.rem_size();
        let tall = |chip: &Chip| css(chip.height()).to_pixels(rem);
        let over = |chip: &Chip| metrics.middle - tall(chip) / 2.0;
        let chips = held.iter().map(|(_, (_, chip))| chip);
        let top = chips
            .clone()
            .fold(Pixels::ZERO, |top, chip| top.max(-over(chip)));
        let height = chips.fold(top + metrics.line, |height, chip| {
            height.max(top + over(chip) + tall(chip))
        });
        let say = |range: Range<usize>, tail: &str, window: &Window| {
            let words = words(line, range, tail, look.key, shared.as_ref(), window);
            words.map(|words| div().flex_none().mt(top).child(words).into_any_element())
        };
        for (k, (at, chip)) in &held {
            parts.extend(say(from..*at, "", window));
            let id = format!("{}-chip-{}", look.key, first + k);
            if let Some(Act::Session(href)) = chip.act().filter(|_| shared.is_some()) {
                link_chip::drawn(&id, (n, *at), href, cx);
            }
            let place = (top + over(chip), look.chip);
            parts.push(chip.element(id, place, shared.as_ref(), window, cx));
            from = at + CHIP.len_utf8();
        }
        parts.extend(say(from..end, tail, window));
        div().flex().items_start().h(height).children(parts)
    };
    let rows: Vec<_> = all.into_iter().enumerate().map(row).collect();
    // A line that wraps fills the bubble, as shrink-to-fit gives it all the room.
    let column = div()
        .flex()
        .flex_col()
        .whitespace_nowrap()
        .when(wrapped, |el| el.w(look.width))
        .children(rows);
    match &shared {
        Some(shared) => links::host(look.key, shared, column.into_any_element(), cx),
        None => column.into_any_element(),
    }
}

#[cfg(test)]
mod tests;
