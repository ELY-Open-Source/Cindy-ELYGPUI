//! A block's text: the inline nodes' with white space collapsed, and a code line's rows.

use gpui::{Pixels, TextStyle, Window};

use crate::{
    markdown::hast::Hast,
    session_view::flow::{Atom, CHIP, atoms, draw::shape, rows},
};

/// The text of a run of inline nodes under `white-space: normal`, a task check as a `CHIP`.
#[derive(Default)]
pub struct Inline {
    pub text: String,
    /// Each check's byte, and whether it is checked.
    pub checks: Vec<(usize, bool)>,
    gap: bool,
}

/// The white space Blink collapses (`logs/review/9.6e-blink-whitespace.log`).
fn collapses(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0b' | '\x0c')
}

impl Inline {
    fn end_gap(&mut self) {
        if std::mem::take(&mut self.gap) {
            self.text.push(' ');
        }
    }

    /// Emphasis, links, breaks and images come with 9.6f to 9.6i: a node reads as its text.
    pub fn read(&mut self, node: &Hast) {
        match node {
            Hast::Text(value) => {
                for c in value.chars() {
                    if collapses(c) {
                        self.gap = !self.text.is_empty();
                    } else {
                        self.end_gap();
                        self.text.push(c);
                    }
                }
            }
            Hast::Element(element) if element.tag == "input" => {
                self.end_gap();
                let checked =
                    element.prop("checked") == Some(&crate::markdown::hast::Prop::Flag(true));
                self.checks.push((self.text.len(), checked));
                self.text.push(CHIP);
            }
            Hast::Element(element) => element.children.iter().for_each(|child| self.read(child)),
            Hast::Raw => {}
        }
    }
}

/// Where a line's text sits, in window pixels: gpui centres the font's box in the line.
pub struct Metrics {
    pub line: f32,
    pub ascent: f32,
    /// The baseline, from the line's top.
    pub baseline: f32,
}

pub fn metrics(style: &TextStyle, window: &Window) -> Metrics {
    let rem = window.rem_size();
    let size = style.font_size.to_pixels(rem);
    // As gpui's text snaps its own line.
    let line = window.pixel_snap(style.line_height.to_pixels(size.into(), rem));
    let probe = shape("x", style, window);
    Metrics {
        line: line.into(),
        ascent: probe.ascent.into(),
        baseline: ((line - probe.ascent - probe.descent) / 2.0 + probe.ascent).into(),
    }
}

/// A part of a code row: its text, or a tab's advance in window pixels.
#[derive(Debug, PartialEq)]
pub enum Piece {
    Text(String),
    Tab(f32),
}

/// Blink's `TabSize` at `tab-size: 4`: a stop nearer than half a space is passed.
fn tab(x: f32, space: f32) -> f32 {
    let stop = 4.0 * space;
    let advance = stop - x % stop;
    if advance < space / 2.0 {
        advance + stop
    } else {
        advance
    }
}

/// The atoms of `line` from `from`, each tab as wide as its stop from the row's start asks.
fn tabbed(line: &str, atoms: &[Atom], from: usize, space: f32) -> Vec<Atom> {
    let mut x = 0.0;
    let rest = atoms.iter().filter(|atom| atom.at >= from);
    let moved = rest.map(|atom| {
        let advance = match line.as_bytes()[atom.at] {
            b'\t' => tab(x, space),
            _ => atom.advance,
        };
        x += advance;
        Atom {
            at: atom.at - from,
            advance,
        }
    });
    moved.collect()
}

fn pieces(line: &str, atoms: &[Atom]) -> Vec<Piece> {
    let mut out = Vec::new();
    for (at, c) in line.char_indices() {
        match (c, out.last_mut()) {
            ('\t', _) => {
                let atom = atoms.iter().find(|atom| atom.at == at);
                out.push(Piece::Tab(
                    atom.expect("a tab is shaped as a space").advance,
                ));
            }
            (c, Some(Piece::Text(text))) => text.push(c),
            (c, _) => out.push(Piece::Text(c.into())),
        }
    }
    out
}

/// One code line's rows at `width`: `pre-wrap` and `break-all`, a tab to its row's next stop.
pub fn code(line: &str, style: &TextStyle, width: Pixels, window: &Window) -> Vec<Vec<Piece>> {
    // A tab is shaped as a space, so that it has a glyph of its own.
    let shaped = atoms(&shape(&line.replace('\t', " "), style, window));
    let space = f32::from(shape(" ", style, window).width);
    let (mut out, mut from) = (Vec::new(), 0);
    loop {
        let rest = &line[from..];
        let atoms = tabbed(line, &shaped, from, space);
        let all = 0..rest.len();
        let broken = rows(rest, &atoms, width.into(), std::slice::from_ref(&all));
        // A tab's width counts from its own row's start, so a row after a wrap is read again.
        let again = broken
            .get(1)
            .filter(|next| rest[next.start..].contains('\t'));
        let kept = if again.is_some() { 1 } else { broken.len() };
        for row in &broken[..kept] {
            let within: Vec<Atom> = atoms
                .iter()
                .filter(|atom| row.contains(&atom.at))
                .map(|atom| Atom {
                    at: atom.at - row.start,
                    advance: atom.advance,
                })
                .collect();
            out.push(pieces(&rest[row.clone()], &within));
        }
        match again {
            Some(next) => from += next.start,
            None => return out,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Inline, tab};
    use crate::markdown::hast::Hast;

    fn collapsed(text: &str) -> String {
        let mut inline = Inline::default();
        inline.read(&Hast::Text(text.into()));
        inline.text
    }

    #[test]
    fn white_space_collapses_as_blink_collapses_it() {
        // `logs/review/9.6e-blink-whitespace.log`.
        for gap in [
            " ", "  ", "\t", "\n", "\r", "\x0c", "\r\n", " \n ", "\x0b", "\n\n\n",
        ] {
            assert_eq!(collapsed(&format!("a{gap}b")), "a b", "{gap:?}");
        }
        assert_eq!(collapsed(" a"), "a");
        assert_eq!(collapsed("a "), "a");
        assert_eq!(collapsed("\ta\t"), "a");
        for kept in [
            "a\u{a0}b",
            "a\u{a0} b",
            "a\u{3000}b",
            "a\u{2003}b",
            "a\u{200b}b",
        ] {
            assert_eq!(collapsed(kept), kept);
        }
    }

    #[test]
    fn a_gap_joins_across_nodes() {
        let mut inline = Inline::default();
        for part in [" a ", " ", " b", " "] {
            inline.read(&Hast::Text(part.into()));
        }
        assert_eq!(inline.text, "a b");
    }

    #[test]
    fn a_tab_reaches_its_rows_next_stop() {
        // `logs/review/9.6e-blink-items.log`: after one, four and five letters.
        assert_eq!(tab(8.0, 8.0), 24.0);
        assert_eq!(tab(32.0, 8.0), 32.0);
        assert_eq!(tab(40.0, 8.0), 24.0);
        // Blink passes a stop nearer than half a space.
        assert_eq!(tab(29.0, 8.0), 35.0);
        assert_eq!(tab(28.0, 8.0), 4.0);
    }
}
