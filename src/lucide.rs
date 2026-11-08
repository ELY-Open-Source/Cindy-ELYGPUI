//! Lucide icons at Cindy's `size` and `strokeWidth`: Ely's bundled SVGs with the stroke swapped.

use gpui::{Styled as _, Svg, svg};

use crate::units::css;

/// `lucide-react` 0.468.0's `MessagesSquare`, which Ely does not bundle.
pub const MESSAGES_SQUARE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 9a2 2 0 0 1-2 2H6l-4 4V4a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2z"/><path d="M18 9h2a2 2 0 0 1 2 2v11l-4-4h-6a2 2 0 0 1-2-2v-1"/></svg>"#;

/// `<Icon size strokeWidth>`; the stroke scales with the icon, as lucide's default does.
pub fn icon(source: &[u8], size: f32, stroke: f32) -> Svg {
    const WIDTH: &str = r#"stroke-width="2""#;
    let markup = String::from_utf8_lossy(source);
    debug_assert!(
        markup.contains(WIDTH),
        "a lucide SVG names its stroke width"
    );
    let swapped = markup.replace(WIDTH, &format!(r#"stroke-width="{stroke}""#));
    svg().data(swapped.as_bytes()).size(css(size)).flex_none()
}
