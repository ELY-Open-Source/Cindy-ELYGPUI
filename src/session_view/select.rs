//! The stream's text selection, as Blink keeps one over `select-text`: a press and a drag.

use std::ops::Range;

use gpui::{
    AnyElement, App, Bounds, DispatchPhase, Global, InteractiveElement as _, IntoElement as _,
    MouseButton, MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, SharedString, Stateful,
    Styled as _, Window, canvas, div, fill, point, px,
};

use icu_properties::{CodePointMapData, props::BidiClass};
use icu_segmenter::GraphemeClusterSegmenter;

use crate::{theme::Appearance, units::css};

mod granular;
#[cfg(test)]
mod tests;

use granular::{Block, Unit};
pub use granular::{block, lines};

/// A place in the text: its block and a byte of the block's text.
#[derive(Clone, PartialEq, Debug)]
pub struct Point {
    pub id: SharedString,
    pub at: usize,
}

/// A glyph's box in a run: its bytes and its x from the run's left; `rtl` starts at its right.
#[derive(Clone, Debug, PartialEq)]
struct Cell {
    bytes: Range<usize>,
    x: Range<f32>,
    rtl: bool,
}

/// A drawn stretch of text: where it stands and what a pointer finds in it.
struct Run {
    id: SharedString,
    start: usize,
    text: SharedString,
    bounds: Bounds<Pixels>,
    /// Its line box's top and bottom.
    line: Range<Pixels>,
    /// The table cell that holds it.
    scope: Option<Bounds<Pixels>>,
    /// Its glyphs' boxes, left to right.
    cells: Vec<Cell>,
    /// The bytes of it the selection holds.
    picked: Option<Range<usize>>,
    /// The handoff card that holds it.
    card: Option<usize>,
    /// A flex item's text: over its box is its start, under it its end.
    item: bool,
}

/// A handoff card's boxes: the card, its circle, its lines and its arrow.
#[derive(Default, Clone, Copy)]
struct Card {
    bounds: Bounds<Pixels>,
    disc: Bounds<Pixels>,
    lines: Bounds<Pixels>,
    arrow: Bounds<Pixels>,
}

/// A box of a handoff card.
pub enum Part {
    Card,
    Disc,
    Lines,
    Arrow,
}

/// Whether `c` reads right to left by its own class; none for a neutral, which follows its place.
fn reads_rtl(c: char) -> Option<bool> {
    match CodePointMapData::<BidiClass>::new().get(c) {
        BidiClass::RightToLeft | BidiClass::ArabicLetter => Some(true),
        BidiClass::LeftToRight | BidiClass::EuropeanNumber | BidiClass::ArabicNumber => Some(false),
        _ => None,
    }
}

/// A shaped line's cells, left to right, one per grapheme: `glyphs` are byte and x, as gpui has them.
fn cells(mut glyphs: Vec<(usize, f32)>, text: &str, width: f32) -> Vec<Cell> {
    let bounds: Vec<usize> = GraphemeClusterSegmenter::new().segment_str(text).collect();
    // A mark's glyph inside a grapheme is no place of its own.
    glyphs.retain(|glyph| bounds.contains(&glyph.0));
    glyphs.sort_by(|left, right| left.1.total_cmp(&right.1));
    let mut seen = Vec::new();
    glyphs.retain(|glyph| {
        !seen.contains(&glyph.0) && {
            seen.push(glyph.0);
            true
        }
    });
    seen.sort_unstable();
    let x_of = |byte: usize| {
        glyphs
            .iter()
            .find(|glyph| glyph.0 == byte)
            .map(|glyph| glyph.1)
    };
    let mut out = Vec::new();
    for (nth, (at, x)) in glyphs.iter().enumerate() {
        let next = seen.iter().copied().find(|start| start > at);
        let before = seen.iter().rev().copied().find(|start| start < at);
        // A neutral leans as the glyphs about it stand: its next byte at its left reads leftwards.
        let leans = match next.and_then(x_of) {
            Some(after) => after < *x,
            None => before.and_then(x_of).is_some_and(|before| before > *x),
        };
        let (end, right) = (
            next.unwrap_or(text.len()),
            glyphs.get(nth + 1).map_or(width, |next| next.1),
        );
        // A ligature's letters share its box in equal parts, as Blink places carets in it.
        let parts: Vec<usize> = bounds
            .iter()
            .copied()
            .filter(|bound| (*at..end).contains(bound))
            .collect();
        let each = (right - x) / parts.len() as f32;
        let edge = |slot: usize| match slot == parts.len() {
            true => right,
            false => x + each * slot as f32,
        };
        let rtl = text[*at..]
            .chars()
            .next()
            .and_then(reads_rtl)
            .unwrap_or(leans);
        for (k, from) in parts.iter().enumerate() {
            let slot = if rtl { parts.len() - 1 - k } else { k };
            out.push(Cell {
                bytes: *from..parts.get(k + 1).copied().unwrap_or(end),
                x: edge(slot)..edge(slot + 1),
                rtl,
            });
        }
    }
    out.sort_by(|left, right| left.x.start.total_cmp(&right.x.start));
    out
}

/// What the held button does: it selects, or it holds the selection it pressed on.
#[derive(Default, Clone, Copy, PartialEq)]
enum Press {
    #[default]
    None,
    Selecting,
    Held {
        moved: bool,
    },
}

/// The selection and what the last frame drew, in document order.
#[derive(Default)]
pub struct Selection {
    anchor: Option<Point>,
    focus: Option<Point>,
    press: Press,
    runs: Vec<Run>,
    line: Option<Bounds<Pixels>>,
    /// The table cell being drawn.
    scope: Option<Bounds<Pixels>>,
    met: (bool, bool),
    inside: bool,
    cards: Vec<Card>,
    /// `user-select: none` boxes whose label alone is text: a quote chip's.
    quiet: Vec<(Bounds<Pixels>, [Pixels; 2])>,
    /// A quote chip's icon, which is `user-select: text` inside that box.
    loud: Vec<Bounds<Pixels>>,
    /// The card being drawn.
    card: Option<usize>,
    /// The blocks that told their whole text, in no order.
    blocks: Vec<Block>,
    /// Links' and images' boxes: Shift with a press there extends nothing.
    links: Vec<Bounds<Pixels>>,
    /// What the last press selected by, and the word or paragraph it selected first.
    unit: Unit,
    base: Option<(Point, Point)>,
}

impl Global for Selection {}

impl Run {
    fn point(&self, at: usize) -> Point {
        Point {
            id: self.id.clone(),
            at: self.start + at,
        }
    }

    /// Its last place: no caret stands after a line feed on its own row.
    fn end(&self) -> usize {
        match &*self.text {
            "\n" => 0,
            text => text.len(),
        }
    }

    /// The byte a pointer `x` from the run's left selects to: the nearer edge of its glyph.
    fn hit(&self, x: f32) -> usize {
        if &*self.text == "\n" {
            return 0;
        }
        let off = |cell: &Cell| (cell.x.start - x).max(x - cell.x.end).max(0.0);
        let nearest = self.cells.iter().min_by(|a, b| off(a).total_cmp(&off(b)));
        let Some(cell) = nearest else { return 0 };
        let left = x < (cell.x.start + cell.x.end) / 2.0;
        if left != cell.rtl {
            cell.bytes.start
        } else {
            cell.bytes.end
        }
    }

    /// The place of a pointer in the run's row: an item's text ends at its own box.
    fn place(&self, position: gpui::Point<Pixels>) -> Point {
        let x = position.x - self.bounds.left();
        match self.item {
            true if position.y < self.bounds.top() => self.point(0),
            true if position.y >= self.bounds.bottom() => self.point(self.text.len()),
            _ => self.point(self.hit(x.into())),
        }
    }

    /// Whether the glyph under `x` is lit.
    fn lit(&self, x: f32) -> bool {
        let under = self.cells.iter().find(|cell| cell.x.contains(&x));
        let picked = self.picked.as_ref();
        under.is_some_and(|cell| picked.is_some_and(|picked| picked.contains(&cell.bytes.start)))
    }

    /// The lit stretches, left to right, each with its opacity: `fades` are the run's fading bytes.
    fn stretches(&self, fades: &[(Range<usize>, f32)]) -> Vec<(Range<f32>, f32)> {
        let mut out: Vec<(Range<f32>, f32)> = Vec::new();
        let Some(picked) = &self.picked else {
            return out;
        };
        for cell in self
            .cells
            .iter()
            .filter(|cell| picked.contains(&cell.bytes.start))
        {
            let fade = fades
                .iter()
                .find(|(range, _)| range.contains(&cell.bytes.start));
            let by = fade.map_or(1.0, |(_, by)| *by);
            match out.last_mut() {
                Some((x, last)) if x.end == cell.x.start && *last == by => x.end = cell.x.end,
                _ => out.push((cell.x.clone(), by)),
            }
        }
        out
    }
}

impl Selection {
    /// The bytes of a run the selection holds; the runs come in document order.
    fn cover(&mut self, id: &SharedString, start: usize, len: usize) -> Option<Range<usize>> {
        let end = start + len;
        // A point a wrap dropped from every run counts at the next run's start.
        let hit = |point: &Option<Point>, met: &mut bool| {
            let point = point.as_ref().filter(|_| !*met)?;
            (point.id == *id && point.at <= end).then(|| {
                *met = true;
                point.at.max(start) - start
            })
        };
        let ends = (
            hit(&self.anchor, &mut self.met.0),
            hit(&self.focus, &mut self.met.1),
        );
        let held = match ends {
            (Some(one), Some(other)) => one.min(other)..one.max(other),
            (Some(at), None) | (None, Some(at)) => {
                self.inside = !self.inside;
                if self.inside { at..len } else { 0..at }
            }
            (None, None) if self.inside => 0..len,
            (None, None) => return None,
        };
        (!held.is_empty()).then_some(held)
    }

    /// A pointer level with card `nth`, or over it (`logs/review/9.6k1b-blink-grid.log`).
    fn in_card(&self, nth: usize, position: gpui::Point<Pixels>) -> Option<Point> {
        let card = &self.cards[nth];
        let mut own = self.runs.iter().filter(|run| run.card == Some(nth));
        let (first, last) = (own.clone().next()?, own.next_back()?);
        if position.x < card.disc.right() {
            return Some(first.point(0));
        }
        if position.x >= card.arrow.left() {
            return Some(last.point(last.text.len()));
        }
        let own = || self.runs.iter().filter(|run| run.card == Some(nth));
        // The nearer of the two lines; half way between them is the lower one.
        let far = |run: &&Run| {
            (run.line.start - position.y)
                .max(position.y - run.line.end)
                .max(Pixels::ZERO)
        };
        let line = own().reduce(|best, run| if far(&run) <= far(&best) { run } else { best })?;
        let row = own().filter(|run| run.line == line.line);
        let off = |run: &&Run| {
            (run.bounds.left() - position.x)
                .max(position.x - run.bounds.right())
                .max(Pixels::ZERO)
        };
        let item = row.reduce(|best, run| if off(&run) < off(&best) { run } else { best })?;
        // A badge is a run per letter: over it is its first one's start, under it its last one's end.
        let mut whole = own().filter(|run| run.id == item.id);
        if position.y < item.bounds.top() {
            return Some(whole.next()?.point(0));
        }
        if position.y >= item.bounds.bottom() {
            let last = whole.next_back()?;
            return Some(last.point(last.text.len()));
        }
        Some(item.point(item.hit((position.x - item.bounds.left()).into())))
    }

    /// The place a pointer at `position` selects to (`logs/review/9.6k1-blink-edges.log`).
    fn at(&self, position: gpui::Point<Pixels>) -> Option<Point> {
        // In a table's cell the pointer selects in that cell, whatever stands beside it.
        let scopes = self.runs.iter().filter_map(|run| run.scope);
        let scope = scopes.into_iter().find(|scope| scope.contains(&position));
        let runs = || {
            self.runs
                .iter()
                .filter(|run| scope.is_none() || run.scope == scope)
        };
        let level = |card: &usize| {
            let bounds = self.cards[*card].bounds;
            (bounds.top()..bounds.bottom()).contains(&position.y)
        };
        if let Some(card) = runs().filter_map(|run| run.card).find(level) {
            return self.in_card(card, position);
        }
        let row = runs().filter(|run| run.line.contains(&position.y));
        let off = |run: &&Run| {
            let (left, right) = (run.bounds.left(), run.bounds.right());
            (left - position.x)
                .max(position.x - right)
                .max(Pixels::ZERO)
        };
        // Beside a row's text the row's nearest end; ties go to the earlier run.
        let nearest = row.reduce(|best, run| if off(&run) < off(&best) { run } else { best });
        if let Some(run) = nearest {
            return Some(run.place(position));
        }
        // Between rows the next row's start; under them all the last one's end.
        let next = runs().find(|run| run.line.start > position.y);
        let last = runs().next_back().map(|run| run.point(run.end()));
        match next {
            Some(Run {
                card: Some(card), ..
            }) => self.in_card(*card, position),
            Some(run) => Some(run.point(0)),
            None => last,
        }
    }

    /// A press of `clicks`, with Shift or not.
    fn down(&mut self, position: gpui::Point<Pixels>, clicks: usize, shift: bool) {
        let lit = |run: &Run| {
            run.bounds.contains(&position) && run.lit((position.x - run.bounds.left()).into())
        };
        // Shift extends, unless the press is on a link (`IsExtendingSelection`).
        let extending = shift && !self.links.iter().any(|link| link.contains(&position));
        // A single press on what is lit drags it.
        if clicks < 2 && !extending && self.runs.iter().any(lit) {
            self.press = Press::Held { moved: false };
            return;
        }
        // A card's bare face takes no caret and keeps the selection.
        let bare = |card: &&Card| {
            let parts = [card.disc, card.lines, card.arrow];
            card.bounds.contains(&position) && !parts.iter().any(|part| part.contains(&position))
        };
        let label = |run: &Run| run.item && run.bounds.contains(&position);
        let text =
            self.runs.iter().any(label) || self.loud.iter().any(|icon| icon.contains(&position));
        let quiet = self
            .quiet
            .iter()
            .any(|(chip, radii)| holds(chip, *radii, position))
            && !text;
        if self.cards.iter().any(|card| bare(&card)) || quiet {
            self.press = Press::None;
            return;
        }
        let place = self.at(position);
        self.press = match place {
            Some(_) => Press::Selecting,
            None => Press::None,
        };
        // A double click on a range keeps it, and a drag extends it.
        let range = self.anchor != self.focus;
        match (place, Unit::of(clicks)) {
            (_, Unit::Word) if range => {}
            (Some(place), unit @ (Unit::Word | Unit::Paragraph)) => {
                self.pick(place, position.y, unit)
            }
            (place, Unit::Char) if extending && self.anchor.is_some() => {
                self.extend(place, position.y)
            }
            (place, _) => {
                (self.anchor, self.focus) = (place.clone(), place);
                (self.unit, self.base) = (Unit::Char, None);
            }
        }
    }

    fn moved(&mut self, position: gpui::Point<Pixels>) {
        match self.press {
            Press::Selecting => self.extend(self.at(position), position.y),
            // Blink drags the selected text: the selection stays.
            Press::Held { .. } => self.press = Press::Held { moved: true },
            Press::None => {}
        }
    }

    fn up(&mut self) {
        if self.press == (Press::Held { moved: false }) {
            (self.anchor, self.focus, self.base) = (None, None, None);
        }
        self.press = Press::None;
    }
}

/// Whether a box round by `top` and `bottom` holds `at`: Blink hits nothing past a corner's arc.
fn holds(bounds: &Bounds<Pixels>, [top, bottom]: [Pixels; 2], at: gpui::Point<Pixels>) -> bool {
    let half = bounds.size.width.min(bounds.size.height) / 2.0;
    let radius = if at.y < bounds.center().y {
        top
    } else {
        bottom
    }
    .min(half);
    let x = at.x.clamp(bounds.left() + radius, bounds.right() - radius);
    let y = at.y.clamp(bounds.top() + radius, bounds.bottom() - radius);
    let (dx, dy, radius) = (f32::from(at.x - x), f32::from(at.y - y), f32::from(radius));
    bounds.contains(&at) && dx * dx + dy * dy <= radius * radius
}

/// A frame's start: the stream's rows record anew.
pub fn begin(cx: &mut App) {
    let selection = cx.default_global::<Selection>();
    selection.runs.clear();
    (selection.line, selection.scope) = (None, None);
    (selection.met, selection.inside) = ((false, false), false);
    selection.cards.clear();
    selection.quiet.clear();
    selection.loud.clear();
    selection.card = None;
    selection.blocks.clear();
    selection.links.clear();
}

/// `getSelection().removeAllRanges()`, and the press that drew it ends.
pub fn unselect(cx: &mut App) {
    let selection = cx.default_global::<Selection>();
    (selection.anchor, selection.focus, selection.press) = (None, None, Press::None);
    (selection.unit, selection.base) = (Unit::Char, None);
}

gpui::actions!(session_view, [SelectAll]);

/// Mod+A: Command on macOS, Control elsewhere.
const SELECT_ALL: &str = if cfg!(target_os = "macos") {
    "cmd-a"
} else {
    "ctrl-a"
};

/// Mod+A: `selectAll` over the stream, unless an Ely text field holds the focus.
pub fn install(cx: &mut App) {
    let binding = gpui::KeyBinding::new(SELECT_ALL, SelectAll, Some("!ElyInput"));
    cx.bind_keys([binding]);
    cx.on_action(|_: &SelectAll, cx| {
        cx.default_global::<Selection>().everything();
        cx.refresh_windows();
    });
}

/// A run drawn at `bounds`: recorded, and lit where the selection holds it.
fn record(
    (id, start, text): (SharedString, usize, SharedString),
    (bounds, cells, item): (Bounds<Pixels>, Vec<Cell>, bool),
    fades: &[(Range<usize>, f32)],
    window: &mut Window,
    cx: &mut App,
) {
    let color = cx.global::<Appearance>().token("text-selection-bg");
    let selection = cx.global_mut::<Selection>();
    let picked = selection.cover(&id, start, text.len());
    // A row taller than its text is the line box a pointer falls in.
    let holds = |around: &Bounds<Pixels>| {
        let slack = gpui::px(0.5);
        around.top() <= bounds.top() + slack && bounds.bottom() <= around.bottom() + slack
    };
    let line = selection.line.filter(holds).unwrap_or(bounds);
    let run = Run {
        id,
        start,
        text,
        bounds,
        line: line.top()..line.bottom(),
        scope: selection.scope.filter(holds),
        cells,
        picked,
        card: selection.card,
        item,
    };
    for (x, by) in run.stretches(fades) {
        let lit = Bounds::from_corners(
            point(bounds.left() + px(x.start), bounds.top()),
            point(bounds.left() + px(x.end), bounds.bottom()),
        );
        // The highlight is its word's: it fades in with it.
        window.paint_quad(fill(lit, color.opacity(by)));
    }
    cx.global_mut::<Selection>().runs.push(run);
}

/// The canvas under a drawn `text`, bytes from `start` of block `id`; its parent is `relative`.
fn under(
    (id, start): (&str, usize),
    text: &str,
    fades: Vec<(Range<usize>, f32)>,
    item: bool,
) -> AnyElement {
    let (id, shown) = (
        SharedString::from(id.to_owned()),
        SharedString::from(text.to_owned()),
    );
    let under = canvas(
        |_, _, _| (),
        move |bounds, (), window, cx| {
            let style = window.text_style();
            let size = style.font_size.to_pixels(window.rem_size());
            let run = style.to_run(shown.len());
            let shaped = window
                .text_system()
                .shape_line(shown.clone(), size, &[run], None);
            let glyphs = shaped.runs.iter().flat_map(|run| &run.glyphs);
            let glyphs = glyphs.map(|glyph| (glyph.index, f32::from(glyph.position.x)));
            let cells = cells(glyphs.collect(), &shown, shaped.width.into());
            record(
                (id.clone(), start, shown.clone()),
                (bounds, cells, item),
                &fades,
                window,
                cx,
            );
        },
    );
    under.absolute().inset_0().into_any_element()
}

/// `child`, which draws `text`, bytes from `start` of block `id`: selectable, lit where selected.
pub fn text(
    place: (&str, usize),
    text: &str,
    fades: Vec<(Range<usize>, f32)>,
    child: AnyElement,
) -> AnyElement {
    div()
        .relative()
        .child(under(place, text, fades, false))
        .child(child)
        .into_any_element()
}

/// A flex item's whole `text`, block `id`, under the text its `relative` parent draws and cuts.
pub fn label(id: &str, text: &str) -> AnyElement {
    under((id, 0), text, Vec::new(), true)
}

/// `child`, which stands for `text` as one box: a tab, a line feed, a row with no text, a letter.
pub fn space(id: &str, start: usize, text: &str, child: AnyElement) -> AnyElement {
    let (id, text) = (
        SharedString::from(id.to_owned()),
        SharedString::from(text.to_owned()),
    );
    let under = canvas(
        |_, _, _| (),
        move |bounds, (), window, cx| {
            let whole = Cell {
                bytes: 0..text.len(),
                x: 0.0..bounds.size.width.into(),
                rtl: false,
            };
            let cells = if text.is_empty() {
                Vec::new()
            } else {
                vec![whole]
            };
            record(
                (id.clone(), start, text.clone()),
                (bounds, cells, false),
                &[],
                window,
                cx,
            );
        },
    );
    div()
        .relative()
        .flex_none()
        .child(under.absolute().inset_0())
        .child(child)
        .into_any_element()
}

/// A table cell's box, for the text drawn in it; its parent is `relative`.
pub fn scope() -> AnyElement {
    let mark = canvas(
        |_, _, _| (),
        |bounds, (), _, cx| cx.global_mut::<Selection>().scope = Some(bounds),
    );
    mark.absolute().inset_0().into_any_element()
}

/// A box a press selects nothing in, `grow` past its parent, round by `top` and `bottom`.
fn mute(grow: f32, radii: [f32; 2]) -> AnyElement {
    let mark = canvas(
        |_, _, _| (),
        move |bounds, (), window, cx| {
            let radii = radii.map(|radius| css(radius).to_pixels(window.rem_size()));
            cx.global_mut::<Selection>().quiet.push((bounds, radii));
        },
    );
    mark.absolute().inset(css(-grow)).into_any_element()
}

/// A chip's box, 1px border and all, its corners `radius`: a press there selects nothing.
pub fn quiet(radius: f32) -> AnyElement {
    mute(1.0, [radius; 2])
}

/// A button's box, its corners `top` and `bottom`: a press keeps what is selected.
pub fn still(top: f32, bottom: f32) -> AnyElement {
    mute(0.0, [top, bottom])
}

/// A quote chip's icon: a press on it places a caret; its parent is `relative`.
pub fn loud() -> AnyElement {
    let mark = canvas(
        |_, _, _| (),
        |bounds, (), _, cx| cx.global_mut::<Selection>().loud.push(bounds),
    );
    mark.absolute().inset_0().into_any_element()
}

/// A link's box; its parent is `relative`.
pub fn link() -> AnyElement {
    let mark = canvas(
        |_, _, _| (),
        |bounds, (), _, cx| cx.global_mut::<Selection>().links.push(bounds),
    );
    mark.absolute().inset_0().into_any_element()
}

/// A box of the handoff card being drawn; its parent is `relative`.
pub fn card(part: Part) -> AnyElement {
    let whole = matches!(part, Part::Card);
    let mark = canvas(
        |_, _, _| (),
        move |bounds, (), _, cx| {
            let selection = cx.global_mut::<Selection>();
            if let Part::Card = part {
                selection.card = Some(selection.cards.len());
                selection.cards.push(Card {
                    bounds,
                    ..Card::default()
                });
            }
            let nth = selection.card.expect("a card's parts come after its box");
            let card = &mut selection.cards[nth];
            match part {
                Part::Card => {}
                Part::Disc => card.disc = bounds,
                Part::Lines => card.lines = bounds,
                // The arrow is the card's last part.
                Part::Arrow => (card.arrow, selection.card) = (bounds, None),
            }
        },
    );
    // The card's own box counts its 1px border.
    let out = if whole { -1.0 } else { 0.0 };
    mark.absolute().inset(css(out)).into_any_element()
}

/// A row's line box, for the text drawn after it; its parent is `relative`.
pub fn line() -> AnyElement {
    let mark = canvas(
        |_, _, _| (),
        |bounds, (), _, cx| cx.global_mut::<Selection>().line = Some(bounds),
    );
    mark.absolute().inset_0().into_any_element()
}

/// The scroller with the selection's press, drag and release.
pub fn listen(scroller: Stateful<gpui::Div>) -> Stateful<gpui::Div> {
    let held = canvas(
        |_, _, _| (),
        |_, (), window, cx| {
            if cx.global::<Selection>().press == Press::None {
                return;
            }
            // The drag goes on past the scroller's edge, and ends wherever the button lifts.
            window.on_mouse_event(|event: &MouseMoveEvent, phase, window, cx| {
                if phase == DispatchPhase::Capture {
                    cx.global_mut::<Selection>().moved(event.position);
                    window.refresh();
                }
            });
            window.on_mouse_event(|event: &MouseUpEvent, phase, window, cx| {
                if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                    cx.global_mut::<Selection>().up();
                    window.refresh();
                }
            });
        },
    );
    scroller
        // Under `select-text` a press on a link, a chip or a button selects too.
        .capture_any_mouse_down(|event, window, cx| {
            if event.button != MouseButton::Left {
                return;
            }
            let selection = cx.global_mut::<Selection>();
            selection.down(event.position, event.click_count, event.modifiers.shift);
            log::debug!(target: "session-view", "selection press: {}", selection.anchor.is_some());
            window.refresh();
        })
        .child(held.absolute().size_0())
}

/// Each run's selected text, in document order.
#[cfg(test)]
pub fn picked(cx: &App) -> Vec<String> {
    let runs = cx.global::<Selection>().runs.iter();
    let held = runs.filter_map(|run| Some(run.text[run.picked.clone()?].to_owned()));
    held.collect()
}
