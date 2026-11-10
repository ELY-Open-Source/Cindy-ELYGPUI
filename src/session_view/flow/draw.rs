//! The bubble's text drawn row by row: words, links and chips in one line.

use std::ops::Range;

use gpui::{
    AnyElement, App, IntoElement, ParentElement as _, Pixels, ShapedLine, SharedString,
    Styled as _, TextStyle, Window, div, prelude::FluentBuilder as _, relative,
};

use super::{Atom, CHIP, atoms, clamp, rows};
use crate::{
    linkify,
    session_view::{
        chip::{self, Chip},
        links::{self, Link, Shared},
    },
    units::css,
    user_message::sent::{self, Part},
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
fn shape(text: &str, style: &TextStyle, window: &Window) -> ShapedLine {
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
}

/// `renderContentWithoutPastedText`'s nodes line by line; a final line feed opens no line.
fn lines(body: &str, legacy: bool, linked: bool) -> Vec<Line> {
    let (mut out, mut nth) = (vec![Line::default()], 0);
    for part in sent::parts(body, legacy) {
        if part == Part::Text("\n".to_owned()) {
            out.push(Line::default());
            continue;
        }
        let line = out.last_mut().expect("a line is open");
        let Part::Text(text) = &part else {
            let chip = chip::of(&part).expect("a part that is no text draws a chip");
            line.chips.push((line.text.len(), chip));
            line.text.push(CHIP);
            continue;
        };
        // `renderTextWithLinks`: each piece is read alone; Blink draws no CR.
        let found = linkify::find(text).into_iter().filter(|_| linked);
        for found in found.filter(|found| matches!(found.link, linkify::Link::Url)) {
            let gone = text[..found.range.start].matches('\r').count();
            let start = line.text.len() + found.range.start - gone;
            line.links.push(Link {
                nth,
                range: start..start + found.range.len(),
                url: text[found.range].to_owned().into(),
            });
            nth += 1;
        }
        line.text.push_str(&text.replace('\r', ""));
    }
    if out.len() > 1 && out.last().is_some_and(|line| line.text.is_empty()) {
        out.pop();
    }
    out
}

/// The line's atoms, each chip as wide as its face, and the rows they break into.
fn broken(
    text: &str,
    chips: &[(usize, Pixels)],
    (style, width): (&TextStyle, Pixels),
    window: &Window,
) -> (Vec<Atom>, Vec<Range<usize>>) {
    let mut atoms = atoms(&shape(text, style, window));
    for (at, width) in chips {
        let atom = atoms.iter_mut().find(|atom| atom.at == *at);
        atom.expect("a chip's stand-in is shaped").advance = f32::from(*width);
    }
    let rows = rows(text, &atoms, width.into());
    (atoms, rows)
}

/// The rows the plain text takes at `width`, as Cindy's hidden mirror measures them.
pub fn count(body: &str, style: &TextStyle, width: Pixels, window: &Window) -> usize {
    let text = body.replace('\r', "");
    let text = text.strip_suffix('\n').unwrap_or(&text);
    let rows = |line| broken(line, &[], (style, width), window).1.len();
    text.split('\n').map(rows).sum()
}

/// Where a row's parts sit: a chip's middle lies half an x-height over the baseline.
struct Metrics {
    line: Pixels,
    /// A row with a chip: its height, and its text's and its chip's tops.
    tall: Pixels,
    text: Pixels,
    chip: Pixels,
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
    // The chip's margin box is 24px: `leading-5`, `py-0.5`, the border, `-my-[1px]`.
    let top = baseline - x_height / 2.0 - css(12.0).to_pixels(rem);
    let (text, chip) = ((-top).max(Pixels::ZERO), top.max(Pixels::ZERO));
    Metrics {
        line,
        tall: (text + line).max(chip + css(24.0).to_pixels(rem)),
        text,
        chip: chip - css(1.0).to_pixels(rem),
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
    let reaches = |link: &&Link| link.range.start < range.end && range.start < link.range.end;
    let held = |link: &Link| Link {
        nth: link.nth,
        range: link.range.start.max(range.start) - range.start
            ..link.range.end.min(range.end) - range.start,
        url: link.url.clone(),
    };
    let held: Vec<Link> = line.links.iter().filter(reaches).map(held).collect();
    match shared.filter(|_| !held.is_empty()) {
        _ if text.is_empty() => None,
        Some(shared) => Some(links::piece(key, text, held, shared, window)),
        None => Some(SharedString::from(text).into_any_element()),
    }
}

/// The message's text: `body` through `sent::parts`, broken at `look.width`.
pub fn text(
    body: &str,
    legacy: bool,
    look: &Look,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let lines = lines(body, legacy, look.kept.is_none());
    let links = lines.iter().map(|line| line.links.len()).sum();
    let shared = (links > 0).then(|| links::shared(look.key, links, window, cx));
    let (mut all, mut shaped, mut chips, mut wrapped) = (Vec::new(), Vec::new(), 0, false);
    for (n, line) in lines.iter().enumerate() {
        let width =
            |(at, chip): &(usize, Chip)| (*at, chip.width(look.chip, &look.style, window, cx));
        let widths: Vec<(usize, Pixels)> = line.chips.iter().map(width).collect();
        let (atoms, rows) = broken(&line.text, &widths, (&look.style, look.width), window);
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
        let top = match held.is_empty() {
            true => Pixels::ZERO,
            false => metrics.text,
        };
        let say = |range: Range<usize>, tail: &str| {
            let words = words(line, range, tail, look.key, shared.as_ref(), window);
            words.map(|words| div().flex_none().mt(top).child(words).into_any_element())
        };
        for (k, (at, chip)) in &held {
            parts.extend(say(from..*at, ""));
            let id = format!("{}-chip-{}", look.key, first + k);
            parts.push(chip.element(id, metrics.chip, look.chip, window, cx));
            from = at + CHIP.len_utf8();
        }
        parts.extend(say(from..end, tail));
        let height = match held.is_empty() {
            true => metrics.line,
            false => metrics.tall,
        };
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
