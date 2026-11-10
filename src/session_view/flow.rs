//! The bubble's inline flow: one logical line broken into rows, as gpui's text wraps a line.

use std::ops::Range;

use gpui::LineLayout;
use icu_properties::{CodePointMapData, props::LineBreak};
use icu_segmenter::LineSegmenter;

/// A chip's place in a line's text: Blink's own stand-in for an atomic inline.
pub const CHIP: char = '\u{fffc}';

/// A glyph or a chip of a line, in reading order: its byte and its advance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Atom {
    pub at: usize,
    pub advance: f32,
}

/// A shaped line's atoms in reading order; glyphs that share a byte join.
pub fn atoms(layout: &LineLayout) -> Vec<Atom> {
    let glyphs = layout.runs.iter().flat_map(|run| run.glyphs.iter());
    let mut placed: Vec<(usize, f32)> = glyphs
        .map(|glyph| (glyph.index, f32::from(glyph.position.x)))
        .collect();
    // The shaper lists right-to-left text from the right: x gives each glyph its advance.
    placed.sort_by(|a, b| a.1.total_cmp(&b.1));
    let ends = placed.iter().skip(1).map(|next| next.1);
    let ends: Vec<f32> = ends.chain([f32::from(layout.width)]).collect();
    let atom = |((at, x), end): (&(usize, f32), f32)| Atom {
        at: *at,
        advance: end - x,
    };
    let mut atoms: Vec<Atom> = placed.iter().zip(ends).map(atom).collect();
    atoms.sort_by_key(|atom| atom.at);
    atoms.dedup_by(|next, kept| {
        let same = next.at == kept.at;
        if same {
            kept.advance += next.advance;
        }
        same
    });
    atoms
}

/// Where a clamped `row` ends so that the ellipsis fits after it, as `line-clamp` cuts.
pub fn clamp(atoms: &[Atom], row: Range<usize>, ellipsis: f32, width: f32) -> usize {
    let mut used = ellipsis;
    for atom in atoms.iter().filter(|atom| row.contains(&atom.at)) {
        used += atom.advance;
        if used > width {
            return atom.at;
        }
    }
    row.end
}

/// White space that hangs past a row's end in `pre-wrap`.
pub fn hangs(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\u{3000}')
}

/// Blink's breaks between two characters of U+0021 to U+00FF (`scripts/blink-line-breaks.js`).
const PAIRS: &str = include_str!("../../resources/line-break-pairs.txt");

/// `ShouldBreakFast`: the pair table's answer, or `None` where the Unicode rules decide.
fn fast(before: Option<char>, last: char, ch: char) -> Option<bool> {
    let (low, high) = ('!', '\u{ff}');
    if last < low || ch < low {
        return Some(false);
    }
    if last == '-' && ch.is_ascii_digit() {
        return Some(before.is_some_and(|ch| ch.is_ascii_alphanumeric()));
    }
    if last > high || ch > high || (last == '-' && !ch.is_ascii()) {
        return None;
    }
    let side = high as usize - low as usize + 2;
    let at = (last as usize - low as usize) * side + (ch as usize - low as usize);
    Some(PAIRS.as_bytes()[at] == b'1')
}

/// UAX 14's LB20a, which ICU4X's rules lack: no break after a hyphen that starts a word.
fn starts_word(before: Option<char>, last: char, ch: char) -> bool {
    let class = |ch| CodePointMapData::<LineBreak>::new().get(ch);
    let letter = [
        LineBreak::Alphabetic,
        LineBreak::Ambiguous,
        LineBreak::HebrewLetter,
    ];
    let open = [
        LineBreak::MandatoryBreak,
        LineBreak::CarriageReturn,
        LineBreak::LineFeed,
        LineBreak::NextLine,
        LineBreak::Space,
        LineBreak::ZWSpace,
        LineBreak::ContingentBreak,
        LineBreak::Glue,
    ];
    let first = before.is_none_or(|before| open.contains(&class(before)));
    matches!(last, '-' | '\u{2010}') && letter.contains(&class(ch)) && first
}

/// `NextBreakablePosition`: where `line` may break; `all` are its `break-all` stretches.
fn breaks(line: &str, all: &[Range<usize>]) -> Vec<usize> {
    let segmenter = LineSegmenter::new_for_non_complex_scripts(Default::default());
    let unicode: Vec<usize> = segmenter.segment_str(line).collect();
    let mut added = Vec::new();
    for span in all {
        let inside = break_all::breaks(&line[span.clone()]);
        added.extend(inside.into_iter().map(|at| span.start + at));
    }
    let space = |ch: char| matches!(ch, ' ' | '\t');
    let (mut breaks, mut before, mut last) = (Vec::new(), None, None::<char>);
    for (at, ch) in line.char_indices() {
        let breaks_here = match last {
            _ if space(ch) => false,
            None => false,
            Some(last) if space(last) => true,
            Some(last) => match fast(before, last, ch) {
                Some(true) => true,
                _ if added.contains(&at) => true,
                Some(false) => false,
                None => unicode.binary_search(&at).is_ok() && !starts_word(before, last, ch),
            },
        };
        if breaks_here {
            breaks.push(at);
        }
        (before, last) = (last, Some(ch));
    }
    breaks
}

/// The rows of `line` at `width`, in bytes; white space at a wrap hangs; `all` is `break-all`.
pub fn rows(line: &str, atoms: &[Atom], width: f32, all: &[Range<usize>]) -> Vec<Range<usize>> {
    let breaks = breaks(line, all);
    let (mut rows, mut first) = (Vec::new(), 0);
    let (mut used, mut candidate, mut segment) = (0.0, None, None::<Range<usize>>);
    for (nth, atom) in atoms.iter().enumerate() {
        let moved = segment.as_ref().is_none_or(|s| !s.contains(&atom.at));
        let hung = line[atom.at..].starts_with(hangs);
        if !hung && moved {
            // A new stretch between break opportunities: the row may end before it.
            if segment.is_some() {
                candidate = Some((nth, used));
            }
            let next = breaks.partition_point(|at| *at <= atom.at);
            let from = next.checked_sub(1).map_or(0, |before| breaks[before]);
            segment = Some(from..breaks.get(next).copied().unwrap_or(line.len()));
        }
        used += atom.advance;
        if hung || used <= width || nth == first {
            continue;
        }
        let (cut, before) = candidate.take().unwrap_or((nth, used - atom.advance));
        let row = &line[atoms[first].at..atoms[cut].at];
        rows.push(atoms[first].at..atoms[first].at + row.trim_end_matches(hangs).len());
        (first, used) = (cut, used - before);
    }
    rows.push(atoms.get(first).map_or(0, |atom| atom.at)..line.len());
    rows
}

mod break_all;
pub mod draw;

#[cfg(test)]
mod tests;
