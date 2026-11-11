//! A block's text: the inline nodes' with white space collapsed, and a code line's rows.

use gpui::{Pixels, TextStyle, Window};

use crate::{
    markdown::hast::{Element, Hast, Prop},
    session_view::{
        flow::{Atom, CHIP, atoms, draw::shape, rows_where},
        target_link::file::FileFace,
    },
};

/// The face an inline element gives its text.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mark {
    pub strong: bool,
    pub italic: bool,
    pub strike: bool,
    /// Which of the text's `a` nodes holds it.
    pub link: Option<usize>,
    pub sup: bool,
    /// Which file chip's box holds the text, the innermost.
    pub file: Option<usize>,
    /// Which inline code holds the text.
    pub code: Option<usize>,
}

/// An atom that is no text: a task's check, or an inline code that holds none.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Chip {
    Check(bool),
    Pill,
    /// A resolved file's `FileTypeIcon`.
    Icon(&'static [u8]),
    /// A deep link's chip, by its `a` among the text's links.
    Link(usize),
}

/// What an `a` or an inline `code` draws as in place of its own face.
pub enum Swap {
    File(FileFace),
    /// `SessionLinkChip` or `ProjectLinkChip`: one atom, whatever the link holds.
    Chip,
    /// A deep link that names nothing: `<span>{label ?? href}</span>`.
    Words(String),
}

/// The swap of an `a` or an inline `code`, by its node.
pub type Files<'a> = &'a dyn Fn(*const Element) -> Option<Swap>;

/// A file chip's box: its node, the box and the link about it, and the text it holds.
pub struct Held {
    pub node: *const Element,
    pub parent: Option<usize>,
    pub link: Option<usize>,
    pub range: std::ops::Range<usize>,
}

/// A run of inline nodes under `white-space: normal`: a `CHIP` per chip, a line feed per `br`.
pub struct Inline {
    pub text: String,
    /// Where each face starts; it holds to the next one.
    pub spans: Vec<(usize, Mark)>,
    pub chips: Vec<(usize, Chip)>,
    /// The `a` nodes read, by address.
    pub links: Vec<*const Element>,
    pub files: Vec<Held>,
    /// The collapsed space not yet written, in its first space's face.
    gap: Option<Mark>,
    /// Whether a space, or a line's start, came last: the spaces after it collapse.
    spaced: bool,
    codes: usize,
}

impl Default for Inline {
    fn default() -> Self {
        Self {
            text: String::new(),
            spans: Vec::new(),
            chips: Vec::new(),
            links: Vec::new(),
            files: Vec::new(),
            gap: None,
            spaced: true,
            codes: 0,
        }
    }
}

/// The white space Blink collapses (`logs/review/9.6e-blink-whitespace.log`).
fn collapses(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0b' | '\x0c')
}

impl Inline {
    fn push(&mut self, c: char, mark: Mark) {
        if self.spans.last().is_none_or(|(_, last)| *last != mark) {
            self.spans.push((self.text.len(), mark));
        }
        self.text.push(c);
    }

    fn end_gap(&mut self) {
        if let Some(mark) = self.gap.take() {
            self.push(' ', mark);
        }
    }

    fn pill(&mut self, mark: Mark) {
        self.chips.push((self.text.len(), Chip::Pill));
        self.push(CHIP, mark);
    }

    /// A line's end drops the spaces before it, through empty codes; a code left empty keeps its box.
    pub fn end(&mut self) {
        self.end_gap();
        let mut start = self.text.len();
        for (at, c) in self.text.char_indices().rev() {
            let pill = c == CHIP && self.chips.contains(&(at, Chip::Pill));
            if c != ' ' && !pill {
                break;
            }
            start = at;
        }
        let held = (start > 0).then(|| self.mark(start - 1).code).flatten();
        let tail = self.text[start..].char_indices();
        let tail: Vec<_> = tail.map(|(at, c)| (c, self.mark(start + at))).collect();
        self.text.truncate(start);
        self.spans.retain(|(at, _)| *at < start);
        self.chips.retain(|(at, _)| *at < start);
        for (c, mark) in tail {
            if c == CHIP || mark.code.is_some_and(|code| held != Some(code)) {
                self.pill(Mark { code: None, ..mark });
            }
        }
    }

    /// The face of the byte at `at`.
    pub fn mark(&self, at: usize) -> Mark {
        let next = self.spans.partition_point(|(start, _)| *start <= at);
        self.spans[next - 1].1
    }

    /// An image reads as its text until 9.6i.
    pub fn read(&mut self, node: &Hast, mark: Mark, files: Files) {
        let Hast::Element(element) = node else {
            let Hast::Text(value) = node else { return };
            for c in value.chars() {
                match collapses(c) {
                    true if self.spaced => {}
                    true => (self.gap, self.spaced) = (Some(mark), true),
                    false => {
                        self.end_gap();
                        self.push(c, mark);
                        self.spaced = false;
                    }
                }
            }
            return;
        };
        let file = match element.tag {
            "a" | "code" => match files(element) {
                Some(Swap::File(file)) => Some(file),
                Some(Swap::Chip) => {
                    self.end_gap();
                    self.links.push(element);
                    let chip = Chip::Link(self.links.len() - 1);
                    self.chips.push((self.text.len(), chip));
                    self.push(CHIP, mark);
                    self.spaced = false;
                    return;
                }
                Some(Swap::Words(words)) => return self.read(&Hast::Text(words), mark, files),
                None => None,
            },
            _ => None,
        };
        let boxed = file.is_some_and(|file| file.boxed);
        let mut inner = match element.tag {
            "em" => Mark {
                italic: true,
                ..mark
            },
            "strong" => Mark {
                strong: true,
                ..mark
            },
            "del" => Mark {
                strike: true,
                ..mark
            },
            "a" => {
                self.links.push(element);
                Mark {
                    link: Some(self.links.len() - 1),
                    ..mark
                }
            }
            "sup" => Mark { sup: true, ..mark },
            // A resolved inline code is the chip's box, not inline code's.
            "code" if !boxed => Mark {
                code: Some(self.codes),
                ..mark
            },
            _ => mark,
        };
        if boxed {
            self.end_gap();
            let held = Held {
                node: element,
                parent: mark.file,
                link: mark.link,
                range: self.text.len()..self.text.len(),
            };
            self.files.push(held);
            inner.file = Some(self.files.len() - 1);
        }
        if let Some(icon) = file.and_then(|file| file.icon) {
            self.end_gap();
            self.chips.push((self.text.len(), Chip::Icon(icon)));
            self.push(CHIP, inner);
            self.spaced = false;
        }
        let before = self.text.len();
        match element.tag {
            "input" => {
                self.end_gap();
                let checked = element.prop("checked") == Some(&Prop::Flag(true));
                self.chips.push((self.text.len(), Chip::Check(checked)));
                self.push(CHIP, mark);
                self.spaced = false;
            }
            "br" => {
                self.end();
                self.push('\n', mark);
                self.spaced = true;
            }
            _ => element
                .children
                .iter()
                .for_each(|child| self.read(child, inner, files)),
        }
        if let Some(held) = inner.file.filter(|_| boxed) {
            self.files[held].range.end = self.text.len();
        }
        if element.tag == "code" && !boxed {
            self.codes += 1;
            // With no text and no space of its own the box is its padding alone.
            if self.text.len() == before && self.gap != Some(inner) {
                self.end_gap();
                self.pill(mark);
            }
        }
    }
}

/// Where a line's text sits, in window pixels: gpui centres the font's box in the line.
pub struct Metrics {
    pub line: f32,
    pub ascent: f32,
    pub descent: f32,
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
        descent: probe.descent.into(),
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
        // `break-all` alone: what it does not break overflows (`9.6f-blink-words.log`).
        let broken = rows_where(
            rest,
            &atoms,
            width.into(),
            std::slice::from_ref(&all),
            |_| false,
            |_| false,
        );
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
    use super::{Chip, Inline, Mark, tab};
    use crate::markdown::hast::Hast;

    fn collapsed(text: &str) -> String {
        let mut inline = Inline::default();
        inline.read(&Hast::Text(text.into()), Mark::default(), &|_| None);
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
            inline.read(&Hast::Text(part.into()), Mark::default(), &|_| None);
        }
        assert_eq!(inline.text, "a b");
    }

    /// The reader over the renderer's own tree for `source`'s first paragraph.
    fn read(source: &str) -> Inline {
        use crate::markdown::hast::{Urls, render};
        let tree = render(source, true, Urls::Trusted);
        let Some(Hast::Element(paragraph)) = tree.first() else {
            panic!("a paragraph")
        };
        let mut inline = Inline::default();
        (paragraph.children.iter()).for_each(|node| inline.read(node, Mark::default(), &|_| None));
        inline.end();
        inline
    }

    #[test]
    fn a_break_that_starts_a_paragraph_has_a_face() {
        let inline = read("\\\n`x`");
        assert_eq!(inline.text, "\nx");
        assert_eq!((inline.mark(0).code, inline.mark(1).code), (None, Some(0)));
    }

    // `logs/review/9.6f-r1-review-evidence.log`
    #[test]
    fn a_code_whose_space_a_line_end_drops_is_a_pill() {
        let inline = read("a` `");
        assert_eq!(inline.text, "a\u{fffc}");
        assert_eq!(inline.chips, [(1, Chip::Pill)]);
        let inline = read("a` `  \nb");
        assert_eq!(inline.text, "a\u{fffc}\nb");
        assert_eq!(inline.chips, [(1, Chip::Pill)]);
        let inline = read("**a` `**");
        assert_eq!(inline.chips, [(1, Chip::Pill)]);
        assert!(inline.mark(1).strong && inline.mark(1).code.is_none());
        // A space no code holds leaves nothing.
        assert_eq!(read("a  \nb").chips, []);
        // `logs/review/9.6f-r2-review-evidence.log`: a code with text keeps one box.
        for source in ["`a `", "`a `  \nb"] {
            let inline = read(source);
            assert_eq!(inline.chips, [], "{source}");
            assert_eq!(inline.text.split('\n').next(), Some("a"), "{source}");
        }
        // The space before an empty code is the line's last, and goes.
        let inline = read("a ` `");
        assert_eq!(
            (inline.text.as_str(), &inline.chips[..]),
            ("a\u{fffc}", &[(1, Chip::Pill)][..])
        );
        let inline = read("`a ` ` `");
        assert_eq!(
            (inline.text.as_str(), &inline.chips[..]),
            ("a\u{fffc}", &[(1, Chip::Pill)][..])
        );
        assert_eq!((inline.mark(0).code, inline.mark(1).code), (Some(0), None));
        // Two codes that hold a space each are two boxes.
        let inline = read("a` ` ` `");
        assert_eq!(inline.text, "a\u{fffc}\u{fffc}");
        assert_eq!(inline.chips.len(), 2);
    }

    #[test]
    fn each_inline_element_gives_its_face() {
        let inline = read("a **b *c*** ~~d~~ [e `f`](https://x.y) g");
        assert_eq!(inline.text, "a b c d e f g");
        let face = |at: usize| inline.mark(at);
        let plain = Mark::default();
        assert_eq!(face(0), plain);
        assert_eq!(
            face(2),
            Mark {
                strong: true,
                ..plain
            }
        );
        assert_eq!(
            face(3),
            Mark {
                strong: true,
                ..plain
            },
            "the space inside"
        );
        let both = Mark {
            strong: true,
            italic: true,
            ..plain
        };
        assert_eq!(face(4), both);
        assert_eq!(face(5), plain);
        assert_eq!(
            face(6),
            Mark {
                strike: true,
                ..plain
            }
        );
        assert_eq!(
            face(8),
            Mark {
                link: Some(0),
                ..plain
            }
        );
        let code = Mark {
            link: Some(0),
            code: Some(0),
            ..plain
        };
        assert_eq!(face(10), code);
        assert_eq!(face(12), plain);
        assert_eq!(inline.spans.len(), 9);
    }

    #[test]
    fn a_break_ends_its_line_and_drops_the_space_about_it() {
        assert_eq!(read("one  \ntwo\\\n three").text, "one\ntwo\nthree");
    }

    #[test]
    fn code_that_holds_no_text_is_a_pill() {
        // Its space collapses into the one before, and so does the space after it.
        let inline = read("a ` ` b `x` `y`");
        assert_eq!(inline.text, "a \u{fffc}b x y");
        assert_eq!(inline.chips, [(2, Chip::Pill)]);
        assert_eq!(inline.mark(7).code, Some(1));
        assert_eq!(inline.mark(9).code, Some(2), "each code is its own box");
        // With no space before it the code keeps its own.
        let inline = read("a` `b");
        assert_eq!((inline.text.as_str(), inline.chips.len()), ("a b", 0));
        assert_eq!(inline.mark(1).code, Some(0));
        // A task's check is a chip too, and a space follows it.
        let tree =
            crate::markdown::hast::render("- [x] a", true, crate::markdown::hast::Urls::Trusted);
        let Some(Hast::Element(list)) = tree.first() else {
            panic!("a list")
        };
        let Some(Hast::Element(item)) =
            list.children.iter().find(|c| matches!(c, Hast::Element(_)))
        else {
            panic!("an item")
        };
        let mut inline = Inline::default();
        (item.children.iter()).for_each(|node| inline.read(node, Mark::default(), &|_| None));
        assert_eq!(inline.text, "\u{fffc} a");
        assert_eq!(inline.chips, [(0, Chip::Check(true))]);
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
