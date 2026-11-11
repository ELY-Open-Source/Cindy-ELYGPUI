//! A block's inline text as rows: each face shaped in its own font, then drawn piece by piece.

use std::ops::Range;

use gpui::{
    AnyElement, Div, FontStyle, FontWeight, Hsla, InteractiveElement as _, IntoElement as _,
    ParentElement as _, Pixels, SharedString, Styled as _, TextStyle, Window, div,
    prelude::FluentBuilder as _,
};

use super::{
    Face,
    marker::{self, CHECK},
    text::{Chip, Inline, Mark, Metrics, metrics},
};
use crate::{
    session_view::flow::{Atom, CHIP, atoms, draw::shape, rows_where},
    units::css,
};

/// What the rows are drawn with; lengths are CSS pixels unless a name says otherwise.
pub struct Look<'a> {
    pub path: &'a str,
    pub face: &'a Face,
    /// Inline code's face: `text-14` in the code font, the block's line ratio.
    pub code: Face,
    pub mono: SharedString,
    /// `--md-strong-fg`, and `--msg-md-inline-code-bg`.
    pub strong: Hsla,
    pub fill: Hsla,
    pub width: Pixels,
    pub zoom: f32,
}

/// The rows, and the first one's baseline under the block's top, in window pixels.
pub struct Drawn {
    pub rows: Vec<AnyElement>,
    pub baseline: f32,
}

/// Blink's `FontHeight` with its leading: the half above the text is floored.
fn extent(at: &Metrics, line: f32) -> (f32, f32) {
    let (ascent, descent) = (at.ascent.round(), at.descent.round());
    let above = ascent + ((line - ascent - descent) / 2.0).floor();
    (above, line - above)
}

/// A row that holds inline code: how far its text sits under its top, and its height.
fn grown((above, below): (f32, f32), (over, under): (f32, f32)) -> (f32, f32) {
    ((over - above).max(0.0), above.max(over) + below.max(under))
}

/// The stretches of `range` that hold one face each.
fn pieces(inline: &Inline, range: &Range<usize>) -> Vec<(Range<usize>, Mark)> {
    let ends = inline.spans.iter().skip(1).map(|(start, _)| *start);
    let ends = ends.chain([inline.text.len()]);
    let held = inline
        .spans
        .iter()
        .zip(ends)
        .filter_map(|((start, mark), end)| {
            let (start, end) = ((*start).max(range.start), end.min(range.end));
            (start < end).then_some((start..end, *mark))
        });
    held.collect()
}

impl Look<'_> {
    fn style(&self, mark: Mark, window: &Window) -> TextStyle {
        let face = if mark.code.is_some() {
            &self.code
        } else {
            self.face
        };
        let mut style = window.text_style();
        style.font_size = css(face.size).into();
        style.line_height = css(face.line).into();
        style.font_weight = if mark.strong {
            FontWeight::BOLD
        } else {
            face.weight
        };
        if mark.italic {
            style.font_style = FontStyle::Italic;
        }
        if mark.code.is_some() {
            style.font_family = self.mono.clone();
        }
        style
    }

    /// Inline code's `px-[0.4em]` and `py-[0.2em]`, in window pixels.
    fn pad(&self) -> (f32, f32) {
        let em = self.code.size * self.zoom;
        (0.4 * em, 0.2 * em)
    }

    fn chip_width(&self, chip: Chip) -> f32 {
        match chip {
            Chip::Check(_) => CHECK * self.zoom,
            Chip::Pill => 2.0 * self.pad().0,
        }
    }

    /// The line's atoms from its own start: each face's glyphs, a code's padding at its ends.
    fn atoms(&self, inline: &Inline, line: &Range<usize>, window: &Window) -> Vec<Atom> {
        let mut all = Vec::new();
        for (range, mark) in pieces(inline, line) {
            let shaped = shape(
                &inline.text[range.clone()],
                &self.style(mark, window),
                window,
            );
            let mut shaped = atoms(&shaped);
            if mark.code.is_some() {
                shaped.first_mut().expect("a code's text").advance += self.pad().0;
                shaped.last_mut().expect("a code's text").advance += self.pad().0;
            }
            let moved = shaped.into_iter().map(|atom| Atom {
                at: atom.at + range.start - line.start,
                advance: atom.advance,
            });
            all.extend(moved);
        }
        for (at, chip) in inline.chips.iter().filter(|(at, _)| line.contains(at)) {
            let atom = all.iter_mut().find(|atom| atom.at + line.start == *at);
            atom.expect("a chip's stand-in is shaped").advance = self.chip_width(*chip);
        }
        all
    }

    /// One piece of a row in its face; `top` sets a tall row's text on the row's baseline.
    fn piece(
        &self,
        inline: &Inline,
        (range, mark): (Range<usize>, Mark),
        (top, root): (f32, &Metrics),
        window: &Window,
    ) -> Div {
        let text = inline.text[range.clone()].to_owned();
        let piece = div()
            .flex_none()
            .when(mark.strong, |el| {
                el.font_weight(FontWeight::BOLD).text_color(self.strong)
            })
            .when(mark.italic, |el| el.italic())
            .when(mark.strike, |el| el.line_through())
            .when(mark.link, |el| el.underline());
        if mark.code.is_none() {
            return piece.mt(css(top / self.zoom)).child(text);
        }
        // The box is the font's own height and the padding; its ends are the code's ends.
        let own = metrics(&self.style(mark, window), window);
        let (pad_x, pad_y) = self.pad();
        let starts = range.start == 0 || inline.mark(range.start - 1).code != mark.code;
        // A space a wrap dropped is no text: the code may end before it.
        let rest = inline.text[range.end..].char_indices();
        let mut rest = rest.take_while(|(at, _)| inline.mark(range.end + at).code == mark.code);
        let ends = rest.all(|(_, c)| c == ' ');
        let fill = div()
            .absolute()
            .left_0()
            .right_0()
            .top(css((own.baseline - own.ascent.round() - pad_y) / self.zoom))
            .h(css((own.ascent.round()
                + own.descent.round()
                + 2.0 * pad_y)
                / self.zoom))
            .when(starts, |el| el.rounded_l(css(6.0)))
            .when(ends, |el| el.rounded_r(css(6.0)))
            .bg(self.fill);
        piece
            .relative()
            .mt(css((top + root.baseline - own.baseline) / self.zoom))
            .when(starts, |el| el.pl(css(pad_x / self.zoom)))
            .when(ends, |el| el.pr(css(pad_x / self.zoom)))
            .font_family(self.mono.clone())
            .text_size(css(self.code.size))
            .line_height(css(self.code.line))
            .child(fill)
            .child(div().relative().child(text))
    }

    /// The code whose first space a wrap after `row` dropped: whether that space was all it held.
    fn stub(inline: &Inline, row: &Range<usize>, next: usize) -> Option<bool> {
        let at = (row.end..next).find(|at| inline.mark(*at).code.is_some())?;
        let code = inline.mark(at).code;
        let opens = at == 0 || inline.mark(at - 1).code != code;
        opens.then(|| at + 1 == inline.text.len() || inline.mark(at + 1).code != code)
    }

    /// One row: its pieces and chips side by side, on the line box Blink gives them.
    fn row(
        &self,
        inline: &Inline,
        (nth, row, stub): (usize, Range<usize>, Option<bool>),
        root: &Metrics,
        window: &Window,
    ) -> (AnyElement, f32) {
        let held = pieces(inline, &row);
        let chips = |range: &Range<usize>| {
            let range = range.clone();
            let within = inline.chips.iter().enumerate();
            within.filter(move |(_, (at, _))| range.contains(at))
        };
        let plain = matches!(&held[..], [(_, mark)] if *mark == Mark::default());
        if plain && stub.is_none() && chips(&row).next().is_none() {
            return (
                inline.text[row].to_owned().into_any_element(),
                root.baseline,
            );
        }
        let line = self.face.line * self.zoom;
        let code = metrics(
            &self.style(
                Mark {
                    code: Some(0),
                    ..Mark::default()
                },
                window,
            ),
            window,
        );
        let boxed = held.iter().any(|(_, mark)| mark.code.is_some())
            || stub.is_some()
            || chips(&row).any(|(_, (_, chip))| *chip == Chip::Pill);
        let (top, height) = match boxed {
            true => grown(
                extent(root, line),
                extent(&code, self.code.line * self.zoom),
            ),
            false => (0.0, line),
        };
        // A code's box with no text: both paddings, or the left one when its text follows.
        let pill = |whole: bool, selector: String| {
            let (pad_x, pad_y) = self.pad();
            let (ascent, descent) = (code.ascent.round(), code.descent.round());
            let width = if whole { 2.0 * pad_x } else { pad_x };
            div()
                .debug_selector(move || selector)
                .flex_none()
                .w(css(width / self.zoom))
                .mt(css((top + root.baseline - ascent - pad_y) / self.zoom))
                .h(css((ascent + descent + 2.0 * pad_y) / self.zoom))
                .rounded_l(css(6.0))
                .when(whole, |el| el.rounded_r(css(6.0)))
                .bg(self.fill)
                .into_any_element()
        };
        let mut parts = Vec::new();
        for (range, mark) in held {
            let mut from = range.start;
            let say = |from: usize, to: usize, parts: &mut Vec<AnyElement>| {
                if from < to {
                    let n = parts.len();
                    let selector = format!("{}-r{nth}-{n}", self.path);
                    let piece = self.piece(inline, (from..to, mark), (top, root), window);
                    parts.push(piece.debug_selector(move || selector).into_any_element());
                }
            };
            for (n, (at, chip)) in chips(&range) {
                say(from, *at, &mut parts);
                let baseline = (top + root.baseline) / self.zoom;
                parts.push(match chip {
                    Chip::Check(checked) => {
                        let selector = format!("{}-check-{n}", self.path);
                        marker::check(*checked, baseline, selector)
                    }
                    Chip::Pill => pill(true, format!("{}-pill-{n}", self.path)),
                });
                from = at + CHIP.len_utf8();
            }
            say(from, range.end, &mut parts);
        }
        if let Some(whole) = stub {
            parts.push(pill(whole, format!("{}-stub-{nth}", self.path)));
        }
        let row = div()
            .flex()
            .items_start()
            .h(css(height / self.zoom))
            .children(parts);
        (row.into_any_element(), top + root.baseline)
    }

    /// The text's lines, each broken at the width; a break at the end opens no line.
    pub fn draw(&self, inline: &Inline, window: &Window) -> Drawn {
        let root = metrics(&self.style(Mark::default(), window), window);
        let body = inline.text.strip_suffix('\n').unwrap_or(&inline.text);
        let (mut rows, mut baseline, mut start) = (Vec::new(), None, 0);
        for text in body.split('\n') {
            let line = start..start + text.len();
            start = line.end + 1;
            let atoms = self.atoms(inline, &line, window);
            // A link's `overflow-wrap: anywhere`; other text overflows (`9.6f-blink-words.log`).
            let anywhere = |at: usize| inline.mark(line.start + at).link;
            let broken = rows_where(text, &atoms, self.width.into(), &[], anywhere);
            for (n, row) in broken.iter().enumerate() {
                let next = broken.get(n + 1).map_or(row.end, |next| next.start);
                let row = line.start + row.start..line.start + row.end;
                let stub = Self::stub(inline, &row, line.start + next);
                let (row, at) = self.row(inline, (rows.len(), row, stub), &root, window);
                baseline.get_or_insert(at);
                rows.push(row);
            }
        }
        Drawn {
            rows,
            baseline: baseline.expect("a line holds a row"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Metrics, extent, grown};

    fn font(ascent: f32, descent: f32) -> Metrics {
        Metrics {
            line: 0.0,
            ascent,
            descent,
            baseline: 0.0,
        }
    }

    #[test]
    fn a_row_with_code_grows_as_blinks_line_box() {
        // `logs/review/9.6f-blink-inline.log`: 24.39 in a paragraph, 28 in an `h1`.
        let (text, code) = (
            extent(&font(14.2, 3.4), 24.0),
            extent(&font(12.0, 4.0), 22.4),
        );
        assert_eq!(text, (17.0, 7.0));
        let (top, height) = grown(text, code);
        assert_eq!(top, 0.0);
        assert!((height - 24.4).abs() < 0.001, "{height}");
        let head = extent(&font(18.0, 4.0), 28.0);
        assert_eq!(grown(head, extent(&font(12.0, 4.0), 19.6)), (0.0, 28.0));
        // Text smaller than the code sits lower in the row.
        assert_eq!(grown((10.0, 4.0), (15.0, 7.0)), (5.0, 22.0));
    }
}
