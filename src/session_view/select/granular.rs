//! Blink's double and triple clicks, and extending by them.

use std::cmp::Ordering;

use gpui::{AnyElement, IntoElement as _, Pixels, SharedString, Styled as _, canvas};

use super::{Point, Run, Selection};
use icu_properties::{CodePointMapData, props::BidiClass};

use crate::{session_view::flow::CHIP, word_break};

/// Blink's `selectTrailingWhitespaceEnabled`: on for Windows alone (`settings.json5`).
const TRAILING: bool = cfg!(target_os = "windows");

/// What a press selects by: its click count's granularity.
#[derive(Default, Clone, Copy, PartialEq, Debug)]
pub enum Unit {
    #[default]
    Char,
    Word,
    Paragraph,
}

impl Unit {
    pub fn of(clicks: usize) -> Self {
        match clicks {
            0 | 1 => Unit::Char,
            2 => Unit::Word,
            _ => Unit::Paragraph,
        }
    }
}

/// A block's whole text and where its paragraphs start, beyond its first.
pub struct Block {
    id: SharedString,
    text: String,
    cuts: Vec<usize>,
}

impl Block {
    pub fn new(id: &str, text: &str, cuts: Vec<usize>) -> Self {
        Self {
            id: SharedString::from(id.to_owned()),
            text: text.to_owned(),
            cuts,
        }
    }
}

/// Registers block `id`'s `text` with its paragraph `cuts`; it draws nothing.
pub fn block(id: &str, text: &str, cuts: Vec<usize>) -> AnyElement {
    let mut block = Some(Block::new(id, text, cuts));
    let mark = canvas(
        |_, _, _| (),
        move |_, (), _, cx| {
            let block = block.take().expect("a block paints once");
            cx.global_mut::<Selection>().blocks.push(block);
        },
    );
    mark.absolute().size_0().into_any_element()
}

/// The paragraph cuts of a code block or a laid text: after each line feed but the last.
pub fn lines(text: &str) -> Vec<usize> {
    let feeds = text.match_indices('\n').map(|(at, _)| at + 1);
    feeds.filter(|at| *at < text.len()).collect()
}

/// `SkipWhitespace`: past the spaces after `at`, a no-break one too, up to a line feed.
pub(super) fn skip_spaces(text: &str, at: usize) -> usize {
    let space = |c: char| match c.is_ascii() {
        true => c.is_ascii_whitespace() || c == '\x0b',
        false => CodePointMapData::<BidiClass>::new().get(c) == BidiClass::WhiteSpace,
    };
    let past = text[at..]
        .char_indices()
        .find(|(_, c)| *c == '\n' || !(space(*c) || *c == '\u{a0}'));
    past.map_or(text.len(), |(by, _)| at + by)
}

impl Selection {
    /// The block's text and cuts: as registered, else its runs with each gap a space.
    fn read(&self, id: &SharedString) -> (String, Vec<usize>) {
        if let Some(block) = self.blocks.iter().find(|block| block.id == *id) {
            return (block.text.clone(), block.cuts.clone());
        }
        let mut text = String::new();
        for run in self.runs.iter().filter(|run| run.id == *id) {
            if text.len() < run.start {
                text.push_str(&" ".repeat(run.start - text.len()));
            }
            if text.len() == run.start {
                text.push_str(&run.text);
            }
        }
        (text, Vec::new())
    }

    /// The block's last byte: its runs' end, short of a break drawn past its text.
    fn limit(&self, id: &SharedString) -> usize {
        let own = self.runs.iter().filter(|run| run.id == *id);
        let end = own.map(|run| run.start + run.text.len()).max().unwrap_or(0);
        let told = self.blocks.iter().find(|block| block.id == *id);
        told.map_or(end, |block| end.min(block.text.len()))
    }

    /// The run that holds `point`: a byte a wrap dropped is the next run's.
    fn rank(&self, point: &Point) -> Option<usize> {
        let own = |run: &&Run| run.id == point.id;
        let holds = |(_, run): &(usize, &Run)| {
            own(run) && run.start <= point.at && point.at <= run.start + run.text.len()
        };
        let after = |(_, run): &(usize, &Run)| own(run) && run.start > point.at;
        let runs = || self.runs.iter().enumerate();
        let found = runs().find(holds).or_else(|| runs().find(after));
        found
            .or_else(|| runs().rfind(|(_, run)| own(run)))
            .map(|(nth, _)| nth)
    }

    fn order(&self, one: &Point, other: &Point) -> Ordering {
        let key = |point: &Point| (self.rank(point), point.at);
        key(one).cmp(&key(other))
    }

    /// Whether `point` is the end of all the stream's text.
    fn last(&self, point: &Point) -> bool {
        let tail = self.runs.last();
        tail.is_some_and(|run| run.id == point.id && point.at >= self.limit(&point.id))
    }

    /// Whether `point`, hit at height `y`, ends a row that wraps on: not a paragraph's end.
    fn wraps(&self, point: &Point, y: Pixels, cuts: &[usize], text: &str) -> bool {
        let ends = |nth: usize, run: &Run| {
            let next = self.runs.get(nth + 1);
            run.id == point.id
                && run.line.contains(&y)
                && run.start + run.text.len() == point.at
                && next.is_some_and(|next| next.id == run.id && next.line.start > run.line.start)
        };
        let feed = text[..point.at.min(text.len())].ends_with('\n');
        let cut = cuts.contains(&point.at) || text[point.at.min(text.len())..].starts_with('\n');
        !feed
            && !cut
            && self
                .runs
                .iter()
                .enumerate()
                .any(|(nth, run)| ends(nth, run))
    }

    /// The paragraph about `at`: from its cut to the next, its line feed and all.
    fn paragraph(&self, point: &Point) -> (Point, Point) {
        let (_, cuts) = self.read(&point.id);
        let limit = self.limit(&point.id);
        let at = point.at.min(limit);
        let start = cuts.iter().copied().filter(|cut| *cut <= at).max();
        let end = cuts.iter().copied().filter(|cut| *cut > at).min();
        let place = |at: usize| Point {
            id: point.id.clone(),
            at: at.min(limit),
        };
        (place(start.unwrap_or(0)), place(end.unwrap_or(limit)))
    }

    /// `ChooseWordSide` for a hit at height `y`, then the word's ends.
    pub(super) fn word(&self, point: &Point, y: Pixels) -> (Point, Point) {
        let (text, cuts) = self.read(&point.id);
        let limit = self.limit(&point.id).min(text.len());
        let at = point.at.min(limit);
        let place = |at: usize| Point {
            id: point.id.clone(),
            at,
        };
        let mut breaks = vec![0];
        for word in word_break::words(&text[..limit]) {
            breaks.push(breaks.last().copied().unwrap_or(0) + word.len());
        }
        let previous = self.last(point) || self.wraps(&place(at), y, &cuts, &text);
        // At a paragraph's end the word is its break: a line feed, else nothing drawn.
        let feed = text[at..].starts_with('\n');
        if !previous && feed {
            return (place(at), self.paragraph(&place(at)).1);
        }
        if !previous && at == limit {
            return (place(at), place(at));
        }
        let (start, end) = match previous {
            true => (
                breaks.iter().rev().find(|cut| **cut < at).copied(),
                breaks.iter().find(|cut| **cut >= at).copied(),
            ),
            false => (
                breaks.iter().rev().find(|cut| **cut <= at).copied(),
                breaks.iter().find(|cut| **cut > at).copied(),
            ),
        };
        (place(start.unwrap_or(at)), place(end.unwrap_or(at)))
    }

    /// `PreviousPositionOf` the end after an empty line or a chip: the character before it.
    fn before_end(&self, point: &Point) -> Point {
        let (text, _) = self.read(&point.id);
        let chip = text[..point.at.min(text.len())].ends_with(CHIP);
        if !self.last(point) || !(point.at == 0 || chip) {
            return point.clone();
        }
        let from = point.at - if chip { CHIP.len_utf8() } else { 0 };
        let own = |run: &&Run| run.id == point.id && run.start >= from;
        let before = self.runs.iter().rev().find(|run| !own(run));
        // A chip's label is the flex item drawn in its place.
        let Some(run) = before.filter(|run| !chip || run.item) else {
            return point.clone();
        };
        let (text, _) = self.read(&run.id);
        let text = &text[..self.limit(&run.id).min(text.len())];
        let at = text.char_indices().next_back().map_or(0, |(at, _)| at);
        Point {
            id: run.id.clone(),
            at,
        }
    }

    /// The `unit` about `point`, hit at height `y`, start and end.
    fn around(&self, point: &Point, y: Pixels, unit: Unit) -> (Point, Point) {
        let point = &match unit {
            Unit::Char => point.clone(),
            _ => self.before_end(point),
        };
        match unit {
            Unit::Char => (point.clone(), point.clone()),
            Unit::Word => self.word(point, y),
            Unit::Paragraph => self.paragraph(point),
        }
    }

    /// A press counted `unit`: the word or paragraph under it, which a drag then extends.
    pub(super) fn pick(&mut self, point: Point, y: Pixels, unit: Unit) {
        let (start, mut end) = self.around(&point, y, unit);
        // Windows' double click takes the spaces after its word too.
        if TRAILING && unit == Unit::Word && start != end {
            let (text, _) = self.read(&end.id);
            end.at = skip_spaces(&text[..self.limit(&end.id).min(text.len())], end.at);
        }
        log::debug!(target: "session-view", "selection by {unit:?}: {} to {}", start.at, end.at);
        self.base = Some((start.clone(), end.clone()));
        (self.anchor, self.focus, self.unit) = (Some(start), Some(end), unit);
    }

    /// The selection extended to `point` in its unit, from the unit it started with.
    pub(super) fn extend(&mut self, point: Option<Point>, y: Pixels) {
        let Some(point) = point else {
            self.focus = None;
            return;
        };
        let Some((first, last)) = self.base.clone().filter(|_| self.unit != Unit::Char) else {
            self.focus = Some(point);
            return;
        };
        let (start, end) = self.around(&point, y, self.unit);
        (self.anchor, self.focus) = match self.order(&point, &first) {
            Ordering::Less => (Some(last), Some(start)),
            _ => match self.order(&end, &last) {
                Ordering::Greater => (Some(first), Some(end)),
                _ => (Some(first), Some(last)),
            },
        };
    }

    /// `selectAll`: from the first run's start to the last run's end.
    pub fn everything(&mut self) {
        let first = self.runs.first().map(|run| run.point(0));
        let last = self.runs.last().map(|run| Point {
            id: run.id.clone(),
            at: self.limit(&run.id),
        });
        log::debug!(target: "session-view", "select all: {}", first.is_some());
        (self.anchor, self.focus) = (first, last);
        (self.unit, self.base) = (Unit::Char, None);
    }
}
