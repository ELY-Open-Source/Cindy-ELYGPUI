//! A GFM table: Blink's automatic columns, collapsed borders, a scroller under a table too wide.

use gpui::{
    AnyElement, ElementId, FontWeight, InteractiveElement as _, IntoElement as _,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, div,
    prelude::FluentBuilder as _,
};

use super::{
    Face, Laid, Walk,
    marker::Marker,
    text::{Inline, Mark},
};
use crate::{
    appearance_settings::text_size,
    markdown::hast::{Element, Hast, Prop},
    theme::Appearance,
    units::css,
};

/// A cell's `px-3` and its halves of the two collapsed borders beside it.
const CELL: f32 = 25.0;

/// Blink's `LayoutUnit`: 64ths of a pixel.
fn units(px: f32) -> i64 {
    (px * 64.0).ceil() as i64
}

/// `DistributeInlineSizeToComputedInlineSizeAuto` over automatic columns of (min, max) content.
pub fn columns(target: f32, sizes: &[(f32, f32)]) -> Vec<f32> {
    let sizes: Vec<_> = sizes
        .iter()
        .map(|(min, max)| (units(*min), units(*max)))
        .collect();
    let (min, max): (Vec<i64>, Vec<i64>) = sizes.iter().copied().unzip();
    let (low, high): (i64, i64) = (min.iter().sum(), max.iter().sum());
    let target = units(target).max(low);
    // Each column takes its share of the room; what the division leaves goes to the last.
    let grow = |base: &[i64], shares: Vec<i64>| {
        let room = target - base.iter().sum::<i64>();
        let total: i64 = shares.iter().sum();
        let even = room / base.len() as i64;
        let share = |share: i64| {
            if total > 0 {
                room * share / total
            } else {
                even
            }
        };
        let mut out: Vec<i64> = (base.iter().zip(&shares))
            .map(|(base, s)| base + share(*s))
            .collect();
        *out.last_mut().expect("a table has a column") += target - out.iter().sum::<i64>();
        out
    };
    let out = if low >= target {
        min
    } else if high == target {
        max
    } else if high > target {
        grow(&min, sizes.iter().map(|(min, max)| max - min).collect())
    } else {
        grow(&max, max.clone())
    };
    out.into_iter().map(|units| units as f32 / 64.0).collect()
}

fn held(node: &Element) -> impl Iterator<Item = &Element> {
    node.children.iter().filter_map(|child| match child {
        Hast::Element(element) => Some(element),
        _ => None,
    })
}

impl Walk<'_> {
    /// `CopyAsImageBlock`'s `my-3` about the table; its toolbar comes with 9.16.
    pub(super) fn table(
        &self,
        table: &Element,
        face: &Face,
        (width, path): (f32, &str),
        markers: &mut Vec<Marker>,
    ) -> Laid {
        let size = text_size(14.0, self.cx);
        let body = Face {
            size,
            line: size * 1.6,
            ..face.clone()
        };
        let head = Face {
            weight: FontWeight::MEDIUM,
            ..body.clone()
        };
        let read = |cell: &Element| {
            let mut inline = Inline::default();
            (cell.children.iter()).for_each(|node| {
                inline.read(node, Mark::default(), &|node| self.anchors.file(node))
            });
            inline.end();
            inline
        };
        let rows = held(table).flat_map(held);
        let rows: Vec<Vec<(&Element, Inline)>> = rows
            .map(|row| held(row).map(|cell| (cell, read(cell))).collect())
            .collect();
        let face = |cell: &Element| if cell.tag == "th" { &head } else { &body };
        let mut sizes = vec![(CELL, CELL); rows[0].len()];
        for (column, (cell, inline)) in rows.iter().flat_map(|row| row.iter().enumerate()) {
            let look = self.look(face(cell), 0.0, path, true);
            let (min, max) = look.sizes(inline, self.window);
            let (low, high) = &mut sizes[column];
            *low = low.max(min / self.zoom + CELL);
            *high = high.max(max / self.zoom + CELL);
        }
        // The table is as wide as its container, or as its columns' least widths ask.
        let least: f32 = sizes.iter().map(|(min, _)| min).sum();
        let exact = columns(width.max(least + 1.0) - 1.0, &sizes);
        // Each column ends on a device pixel, so the cells' rounding does not add up.
        let rem = self.window.rem_size();
        let (mut edge, mut drawn) = (0.0, 0.0);
        let snapped = exact.iter().map(|column| {
            edge += column;
            let at = f32::from(self.window.pixel_snap(css(edge).to_pixels(rem))) / self.zoom;
            let wide = at - drawn;
            drawn = at;
            wide
        });
        let columns: Vec<f32> = snapped.collect();
        let appearance = self.cx.global::<Appearance>();
        let border = appearance.token("msg-table-border");
        let fill = appearance.token("msg-table-header-bg");
        // The first row's height: its tallest cell, with `py-2` and a border.
        let mut first = 17.0_f32;
        let mut cell = |(row, column): (usize, usize), cell: &Element, inline: &Inline| {
            let face = face(cell);
            let selector = format!("{path}-r{row}c{column}");
            let look = self.look(face, columns[column] - CELL, &selector, true);
            let lines = (!inline.text.is_empty()).then(|| {
                let drawn = look.draw(inline, self.window);
                if row == 0 {
                    first = first.max(drawn.height / self.zoom + 17.0);
                }
                let style = self.style(face, false);
                let lines = self.rows((face, &style, drawn.baseline), drawn.rows, &mut Vec::new());
                // `th` is `text-left`; a `td` follows its `align`.
                match cell.prop("align") {
                    Some(Prop::Text(side)) if cell.tag == "td" && side == "center" => {
                        lines.items_center()
                    }
                    Some(Prop::Text(side)) if cell.tag == "td" && side == "right" => {
                        lines.items_end()
                    }
                    _ => lines,
                }
            });
            let name = selector.clone();
            div()
                .debug_selector(move || name)
                .flex_none()
                .w(css(columns[column]))
                .flex()
                .flex_col()
                .justify_center()
                .border_r(css(1.0))
                .border_b(css(1.0))
                .border_color(border)
                .px(css(12.0))
                .py(css(8.0))
                .when(cell.tag == "th", |el| el.bg(fill))
                .children(lines)
        };
        let mut drawn = Vec::new();
        for (row, cells) in rows.iter().enumerate() {
            let cells = cells.iter().enumerate();
            let cells = cells.map(|(column, (node, inline))| cell((row, column), node, inline));
            drawn.push(div().flex().children(cells.collect::<Vec<_>>()));
        }
        // No cell is aligned to a baseline, so the row's is its cells' content bottom.
        let baseline = (first - 8.0) * self.zoom;
        let hung = markers.drain(..);
        let hung = hung.map(|marker| marker.draw(baseline, self.zoom, self.window));
        let hung: Vec<_> = hung.collect();
        let selector = format!("{path}-table");
        // The cells draw the borders after them; the table draws the first two.
        let grid = div()
            .debug_selector(move || selector)
            .flex_none()
            .w(css(columns.iter().sum::<f32>() + 1.0))
            .flex()
            .flex_col()
            .border_l(css(1.0))
            .border_t(css(1.0))
            .border_color(border)
            .children(drawn);
        let selector = path.to_owned();
        let scroller = div()
            .id(ElementId::Name(path.to_owned().into()))
            .debug_selector(move || selector)
            .flex()
            .overflow_x_scroll()
            .child(grid);
        // A marker hangs outside the scroller, which would clip it.
        let element: AnyElement = div()
            .relative()
            .child(scroller)
            .children(hung)
            .into_any_element();
        Laid::new((12.0, 12.0), Some(element))
    }
}

#[cfg(test)]
mod tests {
    use super::columns;

    fn close(got: Vec<f32>, want: &[f32]) {
        assert_eq!(got.len(), want.len());
        for (got, want) in got.iter().zip(want) {
            assert!((got - want).abs() < 0.03, "{got} for {want}");
        }
    }

    // `logs/review/9.6g-blink-cells.log`: each cell's min and max content, and Blink's columns.
    #[test]
    fn columns_are_blinks() {
        // The log rounds to hundredths; a hair less rounds up to the 64th Blink held.
        let cell = |min: f32, max: f32| (min + 24.995, max + 24.995);
        // Room past every max is shared by the max.
        let wide = [cell(9.34, 23.36), cell(10.11, 24.92), cell(10.13, 31.92)];
        close(columns(599.0, &wide), &[186.64, 192.66, 219.7]);
        close(
            columns(599.0, &[cell(10.89, 63.83), cell(0.0, 0.0)]),
            &[467.44, 131.56],
        );
        // Between the mins and the maxes each column grows by its own span.
        let between = [cell(9.34, 21.8), cell(9.34, 840.0)];
        close(columns(599.0, &between), &[42.17, 556.83]);
        let three = [cell(10.13, 1365.5), cell(10.13, 1365.5), cell(7.8, 7.8)];
        close(columns(599.0, &three), &[283.09, 283.09, 32.81]);
        // Under the mins every column keeps its min, and the table outgrows its container.
        let narrow = [cell(11.67, 101.11); 24];
        close(columns(599.0, &narrow), &[36.67; 24]);
        assert_eq!(
            columns(155.0, &[(50.0, 100.0), (30.0, 55.0)]),
            [100.0, 55.0]
        );
    }
}
