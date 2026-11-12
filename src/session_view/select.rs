//! The stream's text selection, as Blink keeps one over `select-text`: a press and a drag.

use std::ops::Range;

use gpui::{
    AnyElement, App, Bounds, DispatchPhase, Global, InteractiveElement as _, IntoElement as _,
    MouseButton, MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, SharedString, Stateful,
    Styled as _, Window, canvas, div, fill, point, px,
};

use icu_properties::{CodePointMapData, props::BidiClass};
use icu_segmenter::GraphemeClusterSegmenter;

use crate::theme::Appearance;

#[cfg(test)]
mod tests;

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
}

impl Global for Selection {}

impl Run {
    fn point(&self, at: usize) -> Point {
        Point {
            id: self.id.clone(),
            at: self.start + at,
        }
    }

    /// The byte a pointer `x` from the run's left selects to: the nearer edge of its glyph.
    fn hit(&self, x: f32) -> usize {
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
            let x = position.x - run.bounds.left();
            return Some(run.point(run.hit(x.into())));
        }
        // Between rows the next row's start; under them all the last one's end.
        let next = runs().find(|run| run.line.start > position.y);
        let last = runs().next_back().map(|run| run.point(run.text.len()));
        next.map(|run| run.point(0)).or(last)
    }

    fn down(&mut self, position: gpui::Point<Pixels>) {
        let lit = |run: &Run| {
            run.bounds.contains(&position) && run.lit((position.x - run.bounds.left()).into())
        };
        if self.runs.iter().any(lit) {
            self.press = Press::Held { moved: false };
            return;
        }
        let place = self.at(position);
        self.press = match place {
            Some(_) => Press::Selecting,
            None => Press::None,
        };
        (self.anchor, self.focus) = (place.clone(), place);
    }

    fn moved(&mut self, position: gpui::Point<Pixels>) {
        match self.press {
            Press::Selecting => self.focus = self.at(position),
            // Blink drags the selected text: the selection stays.
            Press::Held { .. } => self.press = Press::Held { moved: true },
            Press::None => {}
        }
    }

    fn up(&mut self) {
        if self.press == (Press::Held { moved: false }) {
            (self.anchor, self.focus) = (None, None);
        }
        self.press = Press::None;
    }
}

/// A frame's start: the stream's rows record anew.
pub fn begin(cx: &mut App) {
    let selection = cx.default_global::<Selection>();
    selection.runs.clear();
    (selection.line, selection.scope) = (None, None);
    (selection.met, selection.inside) = ((false, false), false);
}

/// `getSelection().removeAllRanges()`, and the press that drew it ends.
pub fn unselect(cx: &mut App) {
    let selection = cx.default_global::<Selection>();
    (selection.anchor, selection.focus, selection.press) = (None, None, Press::None);
}

/// A run drawn at `bounds`: recorded, and lit where the selection holds it.
fn record(
    (id, start, text): (SharedString, usize, SharedString),
    (bounds, cells): (Bounds<Pixels>, Vec<Cell>),
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

/// `child`, which draws `text`, bytes from `start` of block `id`: selectable, lit where selected.
pub fn text(
    (id, start): (&str, usize),
    text: &str,
    fades: Vec<(Range<usize>, f32)>,
    child: AnyElement,
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
                (bounds, cells),
                &fades,
                window,
                cx,
            );
        },
    );
    div()
        .relative()
        .child(under.absolute().inset_0())
        .child(child)
        .into_any_element()
}

/// `child`, which stands for `text` with no glyph: a tab's advance, or a row with no text.
pub fn space(id: &str, start: usize, text: &'static str, child: AnyElement) -> AnyElement {
    let id = SharedString::from(id.to_owned());
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
                (id.clone(), start, text.into()),
                (bounds, cells),
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
            selection.down(event.position);
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
