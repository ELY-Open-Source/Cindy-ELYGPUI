//! What stands before a list item's text: the disc, the number, and a task's check.

use gpui::{
    AnyElement, Hsla, InteractiveElement as _, IntoElement as _, ParentElement as _, PathBuilder,
    Styled as _, TextStyle, Window, canvas, div, point, rgba,
};

use super::text::metrics;
use crate::{session_view::flow::draw::shape, units::css};

/// Blink's `kCMarkerPaddingPx`.
const PADDING: f32 = 7.0;
/// A native checkbox's side, in CSS pixels.
pub const CHECK: f32 = 13.0;

pub enum Kind {
    Disc,
    Number(u32),
}

/// An item's marker until the item's first line takes it.
pub struct Marker {
    pub kind: Kind,
    /// The item's own text style.
    pub style: TextStyle,
    pub color: Hsla,
    /// How far the item's content edge lies left of the line's, in CSS pixels.
    pub x: f32,
    pub selector: String,
}

/// `list_marker.cc`'s disc at `ascent`: its left from the content's edge, its side, its drop.
fn disc(ascent: f32) -> (f32, f32, f32) {
    let ascent = ascent.round();
    let whole = ascent as i32;
    let side = (whole * 2 / 3 + 1) / 2;
    let drop = 3 * (whole - whole * 2 / 3) / 2;
    (-(ascent * 2.0 / 3.0 + PADDING), side as f32, drop as f32)
}

impl Marker {
    /// The marker beside a line whose baseline is `baseline` under its top, in window pixels.
    pub fn draw(self, baseline: f32, zoom: f32, window: &Window) -> AnyElement {
        let own = metrics(&self.style, window);
        let selector = self.selector;
        let mark = div().debug_selector(move || selector).absolute();
        match self.kind {
            Kind::Disc => {
                let (left, side, drop) = disc(own.ascent);
                mark.left(css(left / zoom - self.x))
                    .top(css((baseline - own.ascent.round() + drop) / zoom))
                    .size(css(side / zoom))
                    .rounded_full()
                    .bg(self.color)
                    .into_any_element()
            }
            // The marker's text ends where the content starts, its space kept.
            Kind::Number(number) => {
                let text = format!("{number}. ");
                let width = f32::from(shape(&text, &self.style, window).width);
                mark.left(css(-width / zoom - self.x))
                    .top(css((baseline - own.baseline) / zoom))
                    .whitespace_nowrap()
                    .font(self.style.font())
                    .text_size(self.style.font_size)
                    .line_height(self.style.line_height)
                    .text_color(self.color)
                    .child(text)
                    .into_any_element()
            }
        }
    }
}

/// Chromium's disabled light checkbox (`logs/review/9.6e-blink-checkbox.log`), on the baseline.
pub fn check(checked: bool, baseline: f32, selector: String) -> AnyElement {
    let face = div()
        .debug_selector(move || selector)
        .relative()
        .flex_none()
        .size(css(CHECK))
        .mt(css(baseline - CHECK))
        .rounded(css(2.0));
    let face = match checked {
        true => face.bg(rgba(0xc4c4_c4c6)).child(
            canvas(
                |_, _, _| (),
                |bounds, (), window, _| {
                    let (width, height) = (bounds.size.width, bounds.size.height);
                    let at = |x: f32, y: f32| bounds.origin + point(width * x, height * y);
                    let mut path = PathBuilder::stroke(height * 0.16);
                    path.move_to(at(0.2, 0.5));
                    path.line_to(at(0.4, 0.7));
                    path.line_to(at(0.8, 0.2));
                    let path = path.build().expect("a check mark of three points");
                    window.paint_path(path, rgba(0xebeb_ebe8));
                },
            )
            .size_full(),
        ),
        // The fill lies inside the border, which is drawn over the page.
        false => face
            .border(css(1.0))
            .border_color(rgba(0xbaba_baab))
            .child(div().size_full().rounded(css(1.0)).bg(rgba(0xf5f5_f5ad))),
    };
    face.into_any_element()
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_disc_is_blinks() {
        // `logs/review/9.6e-blink-items.log`: at an ascent of 14, 5 by 5 at -16, 7 down.
        let (left, side, drop) = super::disc(14.2);
        assert_eq!((left.round(), side, drop), (-16.0, 5.0, 7.0));
        assert_eq!(super::disc(18.0), (-19.0, 6.0, 9.0));
    }
}
