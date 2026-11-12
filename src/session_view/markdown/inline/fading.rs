//! A text whose words fade in: one layout, painted part by part at each part's opacity.

use std::ops::Range;

use gpui::{
    AnyElement, Bounds, ContentMask, IntoElement as _, ParentElement as _, SharedString,
    Styled as _, TextAlign, canvas, div, point, px,
};

/// The stretches of a line, left to right, each with its opacity: `glyphs` are byte and x.
fn stretches(
    mut glyphs: Vec<(usize, f32)>,
    width: f32,
    parts: &[(Range<usize>, f32)],
) -> Vec<(Range<f32>, f32)> {
    // A right-to-left word stands where its glyphs do, not where its bytes are.
    glyphs.sort_by(|left, right| left.1.total_cmp(&right.1));
    let mut out: Vec<(Range<f32>, f32)> = Vec::new();
    for (nth, (at, x)) in glyphs.iter().enumerate() {
        let held = parts.iter().find(|(range, _)| range.contains(at));
        let by = held.expect("the parts cover the text").1;
        let end = glyphs.get(nth + 1).map_or(width, |next| next.1);
        match out.last_mut() {
            Some((range, last)) if *last == by => range.end = end,
            _ => out.push((*x..end, by)),
        }
    }
    out
}

/// `text` with each of `parts`, which cover it, at its opacity; the text's own element sizes it.
pub fn over(text: String, parts: Vec<(Range<usize>, f32)>) -> AnyElement {
    let shown: SharedString = text.clone().into();
    let paint = canvas(
        |_, _, _| (),
        move |bounds, (), window, cx| {
            let style = window.text_style();
            let size = style.font_size.to_pixels(window.rem_size());
            let tall = bounds.size.height;
            let system = window.text_system().clone();
            // The whole line is shaped each time, so no part changes another's glyphs.
            let shape = |by: f32| {
                let mut run = style.to_run(shown.len());
                run.color = run.color.opacity(by);
                system.shape_line(shown.clone(), size, &[run], None)
            };
            let line = shape(1.0);
            let glyphs = line.runs.iter().flat_map(|run| &run.glyphs);
            let glyphs = glyphs.map(|glyph| (glyph.index, f32::from(glyph.position.x)));
            let found = stretches(glyphs.collect(), f32::from(line.width), &parts);
            let last = found.len().saturating_sub(1);
            for (nth, (range, by)) in found.into_iter().enumerate() {
                // Ink past the text's own box, a slanted letter's, stays with the end's stretch.
                let from = match nth == 0 {
                    true => bounds.left() - tall,
                    false => bounds.left() + px(range.start),
                };
                let to = match nth == last {
                    true => bounds.right() + tall,
                    false => bounds.left() + px(range.end),
                };
                let corners = (
                    point(from, bounds.top() - tall),
                    point(to, bounds.bottom() + tall),
                );
                let mask = ContentMask {
                    bounds: Bounds::from_corners(corners.0, corners.1),
                };
                window.with_content_mask(Some(mask), |window| {
                    shape(by)
                        .paint(bounds.origin, tall, TextAlign::Left, None, window, cx)
                        .expect("a shaped line paints");
                });
            }
        },
    );
    div()
        .relative()
        .child(div().opacity(0.0).child(text))
        .child(paint.absolute().inset_0())
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::stretches;

    #[test]
    fn a_right_to_left_line_fades_where_its_glyphs_stand() {
        // Two words of two bytes about a space: the first word's glyphs stand at the right.
        let glyphs = vec![(3, 0.0), (4, 5.0), (2, 10.0), (0, 14.0), (1, 19.0)];
        let parts = [(0..3, 0.25), (3..5, 0.0)];
        let found = stretches(glyphs, 24.0, &parts);
        assert_eq!(found, [(0.0..10.0, 0.0), (10.0..24.0, 0.25)]);
    }

    #[test]
    fn a_left_to_right_line_keeps_its_parts() {
        let glyphs = vec![(0, 0.0), (1, 6.0), (2, 12.0), (3, 16.0)];
        let parts = [(0..2, 0.5), (2..3, 1.0), (3..4, 0.0)];
        let found = stretches(glyphs, 22.0, &parts);
        assert_eq!(
            found,
            [(0.0..12.0, 0.5), (12.0..16.0, 1.0), (16.0..22.0, 0.0)]
        );
    }
}
