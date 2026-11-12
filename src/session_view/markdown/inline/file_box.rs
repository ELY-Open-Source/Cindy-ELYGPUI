//! `FileTargetChip`'s box about its parts on a row, and a resolved file's icon.

use std::ops::Range;

use gpui::{
    AnyElement, Div, InteractiveElement as _, IntoElement as _, ParentElement as _, Styled as _,
    div, prelude::FluentBuilder as _,
};

use super::Look;
use crate::{
    lucide,
    session_view::{
        flow::Atom,
        markdown::text::{Held, Inline, Mark, Metrics},
    },
    units::css,
};

/// The chip's `px-1` and `py-0.5`.
const PAD: (f32, f32) = (4.0, 2.0);
/// `FileTypeIcon` at 14 with `mr-1`, set `-0.125em` under the baseline.
pub const ICON: (f32, f32, f32) = (14.0, 4.0, 0.125);

/// Where a row's parts stand: the row, where the next starts, and its text's place.
pub struct At<'a> {
    pub nth: usize,
    pub row: &'a Range<usize>,
    pub next: usize,
    pub top: f32,
    pub root: &'a Metrics,
    pub code: &'a Metrics,
}

/// A part's element: a link's box is dressed once the row stands in order, since the first is its Tab stop.
pub enum Draw {
    Plain(AnyElement),
    Linked(usize, Box<Div>, String),
}

impl Draw {
    pub fn held(link: Option<usize>, piece: Div, name: String) -> Self {
        match link {
            Some(link) => Self::Linked(link, Box::new(piece), name),
            None => Self::Plain(piece.into_any_element()),
        }
    }
}

/// A part of a row with the file box that holds it.
pub type Part = (Option<usize>, Draw);

impl Look<'_> {
    /// The boxes about a part, outermost first.
    fn chain(inline: &Inline, mut file: Option<usize>) -> Vec<usize> {
        let mut out = Vec::new();
        while let Some(held) = file {
            out.insert(0, held);
            file = inline.files[held].parent;
        }
        out
    }

    /// A chip's padding, at the first and the last atom its box holds on the line.
    pub(super) fn pad_files(&self, inline: &Inline, line: &Range<usize>, atoms: &mut [Atom]) {
        let pad = PAD.0 * self.zoom;
        for Held { range, .. } in &inline.files {
            let end = range.end.min(inline.text.len());
            let at = |atom: &Atom| atom.at + line.start;
            if line.contains(&range.start)
                && let Some(first) = atoms.iter_mut().find(|atom| at(atom) >= range.start)
            {
                first.advance += pad;
            }
            if end > line.start
                && end <= line.end
                && let Some(last) = atoms.iter_mut().rev().find(|atom| at(atom) < end)
            {
                last.advance += pad;
            }
        }
    }

    /// The chips' `break-all`, in the line's own bytes.
    pub(super) fn broken_all(inline: &Inline, line: &Range<usize>) -> Vec<Range<usize>> {
        let within = inline.files.iter().filter_map(|held| {
            let start = held.range.start.max(line.start);
            let end = held.range.end.min(line.end);
            (start < end).then(|| start - line.start..end - line.start)
        });
        within.collect()
    }

    /// The icon before a resolved file's label.
    pub(super) fn icon(&self, source: &'static [u8], mark: Mark, at: &At, name: String) -> Div {
        let under = ICON.2 * self.size(mark);
        let color = if mark.strong {
            self.strong
        } else {
            self.face.color
        };
        div()
            .debug_selector(move || name)
            .flex_none()
            .w(css(ICON.0 + ICON.1))
            .mt(css((at.top + at.root.baseline) / self.zoom + under - ICON.0))
            .child(lucide::icon(source, ICON.0, 2.0).text_color(color))
    }

    /// The box of file `held` about its `inner` parts on this row.
    fn fragment(
        &self,
        inline: &Inline,
        held: usize,
        inner: Vec<AnyElement>,
        at: &At,
    ) -> AnyElement {
        let Held {
            node,
            range,
            link,
            fade,
            ..
        } = &inline.files[held];
        let end = range.end.min(inline.text.len());
        let (starts, ends) = (range.start >= at.row.start, end <= at.next);
        let chip = self.anchors.chip(*node);
        let (ascent, descent) = (at.code.ascent.round(), at.code.descent.round());
        let fill = div()
            .absolute()
            .left_0()
            .right_0()
            .top(css((at.top + at.root.baseline - ascent) / self.zoom - PAD.1))
            .h(css((ascent + descent) / self.zoom + 2.0 * PAD.1))
            .when(starts, |el| el.rounded_l(css(6.0)))
            .when(ends, |el| el.rounded_r(css(6.0)))
            .bg(chip.fill);
        let selector = format!("{}-file-{held}-r{}", self.path, at.nth);
        let el = div()
            .debug_selector(move || selector)
            .relative()
            .flex()
            .flex_none()
            .items_start()
            .when(starts, |el| el.pl(css(PAD.0)))
            .when(ends, |el| el.pr(css(PAD.0)))
            .when_some(*fade, |el, by| el.opacity(by))
            .child(fill)
            .children(inner);
        let fragment = chip.fragment(el, at.nth);
        // A chip inside a link is the link's box, as the anchor holds the chip.
        match *link {
            Some(link) => {
                let outer = div().flex_none().child(fragment);
                self.boxed(inline, link, outer, (at.nth, &format!("file-{held}")))
            }
            None => fragment,
        }
    }

    /// The row's parts, each run a file box holds drawn inside that box.
    pub(super) fn nest(
        &self,
        inline: &Inline,
        parts: Vec<Part>,
        depth: usize,
        at: &At,
    ) -> Vec<AnyElement> {
        let (mut out, mut run, mut open) = (Vec::new(), Vec::new(), None);
        let close = |open: Option<usize>, run: &mut Vec<Part>, out: &mut Vec<AnyElement>| {
            if let Some(held) = open {
                let inner = self.nest(inline, std::mem::take(run), depth + 1, at);
                out.push(self.fragment(inline, held, inner, at));
            }
        };
        for (file, part) in parts {
            let held = Self::chain(inline, file).get(depth).copied();
            if held != open {
                close(open, &mut run, &mut out);
                open = held;
            }
            match held {
                Some(_) => run.push((file, part)),
                None => out.push(match part {
                    Draw::Plain(part) => part,
                    Draw::Linked(link, piece, name) => {
                        self.boxed(inline, link, *piece, (at.nth, &name))
                    }
                }),
            }
        }
        close(open, &mut run, &mut out);
        out
    }
}
