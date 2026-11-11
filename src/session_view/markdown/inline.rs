//! A block's inline text as rows: each face shaped in its own font, then drawn piece by piece.

mod file_box;

use std::ops::Range;

use file_box::{At, Draw, ICON, Part};

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
    session_view::{
        flow::{Atom, CHIP, atoms, draw::shape, rows_where},
        inline_image,
        target_link::Anchors,
    },
    units::css,
};

/// Tailwind's preflight for `sup`: `font-size: 75%`, and `top: -0.5em` of that.
const SUP: (f32, f32) = (0.75, 0.5);

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
    /// `overflow-wrap: anywhere` on the whole text, as a table's cell has it.
    pub anywhere: bool,
    /// What each `a` draws as.
    pub anchors: &'a Anchors,
}

/// A code box a wrap left: whether the dropped space was all it held, and the link that holds it.
type Stub = (bool, Mark);

/// A row to draw: its place among the block's rows, its bytes, and where the next row starts.
struct Row {
    nth: usize,
    range: Range<usize>,
    next: usize,
    stub: Option<Stub>,
}

/// The rows, the first one's baseline under the block's top and their height, in window pixels.
pub struct Drawn {
    pub rows: Vec<AnyElement>,
    pub baseline: f32,
    pub height: f32,
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
    /// The face's size in CSS pixels; `sup` has `line-height: 0`, so the line stays.
    fn size(&self, mark: Mark) -> f32 {
        let face = if Self::mono(mark) {
            &self.code
        } else {
            self.face
        };
        face.size * if mark.sup { SUP.0 } else { 1.0 }
    }

    /// Inline code and a file chip are `font-mono text-14`.
    fn mono(mark: Mark) -> bool {
        mark.code.is_some() || mark.file.is_some()
    }

    /// Whether the text's `a` draws as a link.
    fn linked(&self, inline: &Inline, mark: Mark) -> bool {
        mark.link
            .is_some_and(|link| self.anchors.marked(inline.links[link]))
    }

    fn style(&self, mark: Mark, window: &Window) -> TextStyle {
        let face = if Self::mono(mark) {
            &self.code
        } else {
            self.face
        };
        let mut style = window.text_style();
        style.font_size = css(self.size(mark)).into();
        style.line_height = css(face.line).into();
        style.font_weight = if mark.strong {
            FontWeight::BOLD
        } else {
            face.weight
        };
        if mark.italic {
            style.font_style = FontStyle::Italic;
        }
        if Self::mono(mark) {
            style.font_family = self.mono.clone();
        }
        style
    }

    /// Inline code's `px-[0.4em]` and `py-[0.2em]`, in window pixels.
    fn pad(&self) -> (f32, f32) {
        let em = self.code.size * self.zoom;
        (0.4 * em, 0.2 * em)
    }

    fn chip_width(&self, inline: &Inline, chip: Chip) -> f32 {
        match chip {
            Chip::Check(_) => CHECK * self.zoom,
            Chip::Pill => 2.0 * self.pad().0,
            Chip::Icon(_) => (ICON.0 + ICON.1) * self.zoom,
            Chip::Link(link) => self.anchors.link_size(inline.links[link]).0,
            Chip::Image(image) => self.image_width(inline, image),
        }
    }

    /// An image's button in this line, in window pixels: it shrinks to the line's width.
    fn image_width(&self, inline: &Inline, image: usize) -> f32 {
        let picture = self.anchors.picture(inline.images[image]);
        picture.size(f32::from(self.width) / self.zoom).0 * self.zoom
    }

    /// An image's reach over and under the baseline, its margins counted.
    fn image_extent(&self, inline: &Inline, image: usize) -> (f32, f32) {
        let picture = self.anchors.picture(inline.images[image]);
        let (over, under) = picture.reach(f32::from(self.width) / self.zoom);
        (over * self.zoom, under * self.zoom)
    }

    /// A link chip's reach over and under the baseline: its middle half an x-height over it.
    fn link_extent(&self, inline: &Inline, link: usize, window: &Window) -> (f32, f32) {
        let style = self.style(Mark::default(), window);
        let size = style.font_size.to_pixels(window.rem_size());
        let system = window.text_system();
        let middle = f32::from(system.x_height(system.resolve_font(&style.font()), size)) / 2.0;
        let half = self.anchors.link_size(inline.links[link]).1 * self.zoom / 2.0;
        (half + middle, half - middle)
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
            atom.expect("a chip's stand-in is shaped").advance = self.chip_width(inline, *chip);
        }
        self.pad_files(inline, line, &mut all);
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
            .when(self.linked(inline, mark) || mark.file.is_some(), |el| {
                el.underline()
            })
            .when(mark.sup, |el| {
                let size = self.size(mark);
                el.relative().top(css(-SUP.1 * size)).text_size(css(size))
            });
        if !Self::mono(mark) && !mark.sup {
            return piece.mt(css(top / self.zoom)).child(text);
        }
        let own = metrics(&self.style(mark, window), window);
        // Text of another size or font stands on the row's baseline.
        let lift = css((top + root.baseline - own.baseline) / self.zoom);
        if !Self::mono(mark) {
            return piece.mt(lift).child(text);
        }
        let piece = piece
            .mt(lift)
            .font_family(self.mono.clone())
            .text_size(css(self.size(mark)))
            .line_height(css(self.code.line));
        if mark.code.is_none() {
            return piece.child(text);
        }
        // The box is the font's own height and the padding; its ends are the code's ends.
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
            .when(starts, |el| el.pl(css(pad_x / self.zoom)))
            .when(ends, |el| el.pr(css(pad_x / self.zoom)))
            .child(fill)
            .child(div().relative().child(text))
    }

    /// The code whose first space a wrap after `row` dropped.
    fn stub(inline: &Inline, row: &Range<usize>, next: usize) -> Option<Stub> {
        let at = (row.end..next).find(|at| inline.mark(*at).code.is_some())?;
        let mark = inline.mark(at);
        let opens = at == 0 || inline.mark(at - 1).code != mark.code;
        let whole = at + 1 == inline.text.len() || inline.mark(at + 1).code != mark.code;
        opens.then_some((whole, mark))
    }

    /// The link whose box a part is: none when a chip inside that link holds the part.
    fn link_of(inline: &Inline, mark: Mark) -> Option<usize> {
        let mut file = mark.file;
        while let Some(held) = file.map(|held| &inline.files[held]) {
            if held.link == mark.link {
                return None;
            }
            file = held.parent;
        }
        mark.link
    }

    /// `piece`, the box `name` of row `row`, with its link's listeners.
    fn boxed(
        &self,
        inline: &Inline,
        link: usize,
        piece: Div,
        (row, name): (usize, &str),
    ) -> AnyElement {
        let at = format!("r{row}-{name}");
        self.anchors.dress(inline.links[link], piece, &at)
    }

    /// One row: its pieces and chips side by side, on the line box Blink gives them.
    fn row(
        &self,
        inline: &Inline,
        Row {
            nth,
            range: row,
            next,
            stub,
        }: Row,
        root: &Metrics,
        window: &Window,
    ) -> (AnyElement, f32, f32) {
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
                root.line,
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
        let boxed = held.iter().any(|(_, mark)| Self::mono(*mark))
            || stub.is_some()
            || chips(&row).any(|(_, (_, chip))| *chip == Chip::Pill);
        let (mut top, mut height) = match boxed {
            true => grown(
                extent(root, line),
                extent(&code, self.code.line * self.zoom),
            ),
            false => (0.0, line),
        };
        // A link chip's margin box grows the row as Blink's line box grows.
        let above = top + root.baseline;
        let reach = chips(&row).filter_map(|(_, (_, chip))| match chip {
            Chip::Link(link) => Some(self.link_extent(inline, *link, window)),
            Chip::Image(image) => Some(self.image_extent(inline, *image)),
            _ => None,
        });
        let (over, under) = reach.fold((above, height - above), |tall, chip| {
            (tall.0.max(chip.0), tall.1.max(chip.1))
        });
        (top, height) = (top + over - above, over + under);
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
        };
        let place = At {
            nth,
            row: &row,
            next,
            top,
            root,
            code: &code,
        };
        let mut parts: Vec<Part> = Vec::new();
        for (range, mark) in held {
            let mut from = range.start;
            let say = |from: usize, to: usize, parts: &mut Vec<Part>| {
                if from < to {
                    let n = parts.len();
                    let selector = format!("{}-r{nth}-{n}", self.path);
                    let piece = self.piece(inline, (from..to, mark), (top, root), window);
                    let piece = piece.debug_selector(move || selector);
                    let link = Self::link_of(inline, mark);
                    parts.push((mark.file, Draw::held(link, piece, n.to_string())));
                }
            };
            for (n, (at, chip)) in chips(&range) {
                say(from, *at, &mut parts);
                let baseline = (top + root.baseline) / self.zoom;
                let part = match chip {
                    Chip::Check(checked) => {
                        let selector = format!("{}-check-{n}", self.path);
                        Draw::Plain(marker::check(*checked, baseline, selector))
                    }
                    Chip::Pill => {
                        let pill = pill(true, format!("{}-pill-{n}", self.path));
                        Draw::held(Self::link_of(inline, mark), pill, format!("pill-{n}"))
                    }
                    Chip::Link(link) => {
                        let node = inline.links[*link];
                        let over = self.link_extent(inline, *link, window).0;
                        let chip = div()
                            .flex_none()
                            .mt(css((top + root.baseline - over) / self.zoom))
                            .child(self.anchors.link_chip(node));
                        Draw::Plain(chip.into_any_element())
                    }
                    Chip::Image(image) => {
                        let over = self.image_extent(inline, *image).0;
                        let avail = f32::from(self.width) / self.zoom;
                        let atom = div()
                            .flex_none()
                            .mt(css((top + root.baseline - over) / self.zoom))
                            .pt(css(inline_image::MARGIN))
                            .child(self.anchors.picture(inline.images[*image]).atom(avail));
                        Draw::Plain(atom.into_any_element())
                    }
                    Chip::Icon(source) => {
                        let icon =
                            self.icon(source, mark, &place, format!("{}-icon-{n}", self.path));
                        Draw::held(Self::link_of(inline, mark), icon, format!("icon-{n}"))
                    }
                };
                parts.push((mark.file, part));
                from = at + CHIP.len_utf8();
            }
            say(from, range.end, &mut parts);
        }
        if let Some((whole, mark)) = stub {
            let pill = pill(whole, format!("{}-stub-{nth}", self.path));
            let pill = Draw::held(Self::link_of(inline, mark), pill, "stub".into());
            parts.push((mark.file, pill));
        }
        let row = div()
            .flex()
            .items_start()
            .h(css(height / self.zoom))
            .children(self.nest(inline, parts, 0, &place));
        (row.into_any_element(), top + root.baseline, height)
    }

    /// The min and max content widths in window pixels, a break allowed after every atom.
    pub fn sizes(&self, inline: &Inline, window: &Window) -> (f32, f32) {
        let (mut min, mut max, mut start) = (0.0_f32, 0.0_f32, 0);
        for text in inline.text.split('\n') {
            let line = start..start + text.len();
            start = line.end + 1;
            let mut atoms = self.atoms(inline, &line, window);
            // A space hangs and counts nothing; a code's box with no text joins the atom before.
            let (mut run, mut lead) = (0.0, true);
            for atom in &mut atoms {
                let at = line.start + atom.at;
                // An image asks what its own content asks, whatever the line's width.
                let image = inline.chips.iter().find_map(|(place, chip)| match chip {
                    Chip::Image(image) if *place == at => Some(inline.images[*image]),
                    _ => None,
                });
                if let Some(image) = image {
                    let (least, most) = self.anchors.picture(image).content();
                    (run, lead, atom.advance) = (least * self.zoom, false, most * self.zoom);
                    min = min.max(run);
                    continue;
                }
                let code = inline.mark(at).code;
                let edge = |at: Option<usize>| at.is_none_or(|at| inline.mark(at).code != code);
                if inline.chips.contains(&(at, Chip::Pill)) {
                    run += atom.advance;
                } else if !inline.text[at..].starts_with(' ') {
                    // Boxes that start a line join the atom after them.
                    run = atom.advance + if lead { run } else { 0.0 };
                    lead = false;
                } else if code.is_some() {
                    let next = Some(at + 1).filter(|next| *next < inline.text.len());
                    let ends = [edge(at.checked_sub(1)), edge(next)];
                    run += self.pad().0 * ends.iter().filter(|end| **end).count() as f32;
                }
                min = min.max(run);
            }
            max = max.max(atoms.iter().map(|atom| atom.advance).sum());
        }
        (min, max)
    }

    /// The text's lines, each broken at the width; a break at the end opens no line.
    pub fn draw(&self, inline: &Inline, window: &Window) -> Drawn {
        let root = metrics(&self.style(Mark::default(), window), window);
        let body = inline.text.strip_suffix('\n').unwrap_or(&inline.text);
        let (mut rows, mut baseline, mut start, mut height) = (Vec::new(), None, 0, 0.0);
        for text in body.split('\n') {
            let line = start..start + text.len();
            start = line.end + 1;
            let atoms = self.atoms(inline, &line, window);
            // A link's `overflow-wrap: anywhere`; other text overflows (`9.6f-blink-words.log`).
            let anywhere =
                |at: usize| self.anywhere || self.linked(inline, inline.mark(line.start + at));
            // An empty code's box stays with what stands before it, or at a line's start after it.
            let pill = |at: usize| inline.chips.contains(&(line.start + at, Chip::Pill));
            let lead = (atoms.iter())
                .find(|atom| !pill(atom.at))
                .map(|atom| atom.at);
            let stuck = |at: usize| pill(at) || (at > 0 && lead == Some(at));
            let all = Self::broken_all(inline, &line);
            let broken = rows_where(text, &atoms, self.width.into(), &all, anywhere, stuck);
            for (n, row) in broken.iter().enumerate() {
                let next = broken.get(n + 1).map_or(row.end, |next| next.start);
                let row = line.start + row.start..line.start + row.end;
                let next = line.start + next;
                let row = Row {
                    nth: rows.len(),
                    stub: Self::stub(inline, &row, next),
                    range: row,
                    next,
                };
                let (row, at, tall) = self.row(inline, row, &root, window);
                baseline.get_or_insert(at);
                height += tall;
                rows.push(row);
            }
        }
        Drawn {
            rows,
            baseline: baseline.expect("a line holds a row"),
            height,
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
