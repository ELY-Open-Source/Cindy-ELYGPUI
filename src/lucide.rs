//! Lucide icons at Cindy's `size` and `strokeWidth`: Ely's bundled SVGs with the stroke swapped.

use gpui::{Styled as _, Svg, svg};

use crate::units::css;

/// `lucide-react` 0.468.0's `MessagesSquare`, which Ely does not bundle.
pub const MESSAGES_SQUARE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 9a2 2 0 0 1-2 2H6l-4 4V4a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2z"/><path d="M18 9h2a2 2 0 0 1 2 2v11l-4-4h-6a2 2 0 0 1-2-2v-1"/></svg>"#;

/// `FileChartColumn`, which Ely does not bundle.
pub const FILE_CHART_COLUMN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/><path d="M8 18v-1"/><path d="M12 18v-6"/><path d="M16 18v-3"/></svg>"#;

/// `FileVideo`, which Ely does not bundle.
pub const FILE_VIDEO: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/><path d="m10 11 5 3-5 3v-6Z"/></svg>"#;

/// `Globe2`, lucide 0.468's `Earth`, which Ely does not bundle.
pub const EARTH: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21.54 15H17a2 2 0 0 0-2 2v4.54"/><path d="M7 3.34V5a3 3 0 0 0 3 3a2 2 0 0 1 2 2c0 1.1.9 2 2 2a2 2 0 0 0 2-2c0-1.1.9-2 2-2h3.17"/><path d="M11 21.95V18a2 2 0 0 0-2-2a2 2 0 0 1-2-2v-1a2 2 0 0 0-2-2H2.05"/><circle cx="12" cy="12" r="10"/></svg>"#;

/// `Plug`, which Ely does not bundle.
pub const PLUG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 22v-5"/><path d="M9 8V2"/><path d="M15 8V2"/><path d="M18 8v5a4 4 0 0 1-4 4h-4a4 4 0 0 1-4-4V8Z"/></svg>"#;

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

/// `lucide-react` 0.468.0 icons Ely lacks, stroke 2 as lucide draws them.
fn lucide(body: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{body}</svg>"#
    )
}

/// `CirclePlus`.
pub fn circle_plus() -> String {
    lucide(r#"<circle cx="12" cy="12" r="10"/><path d="M8 12h8"/><path d="M12 8v8"/>"#)
}

/// `MessageCircle`.
pub fn message_circle() -> String {
    lucide(r#"<path d="M7.9 20A9 9 0 1 0 4 16.1L2 22Z"/>"#)
}

pub fn users_round() -> String {
    lucide(
        r#"<path d="M18 21a8 8 0 0 0-16 0"/><circle cx="10" cy="8" r="5"/><path d="M22 20c0-3.37-2-6.5-4-8a5 5 0 0 0-.45-8.3"/>"#,
    )
}

/// Cindy's `brandMarkPaths.ts`: Claude Code, filled.
const CLAUDE_CODE: &str = "M20.998 10.949H24v3.102h-3v3.028h-1.487V20H18v-2.921h-1.487V20H15v-2.921H9V20H7.488v-2.921H6V20H4.487v-2.921H3V14.05H0v-3.1h3V5h17.998zM6 10.949h1.488V8.102H6zm10.51 0H18V8.102h-1.49z";
const CODEX_FLOWER: &str = "M9.064 3.344a4.6 4.6 0 0 1 2.285-.312q1.5.173 2.673 1.275.016.015.037.021a.1.1 0 0 0 .043 0 4.55 4.55 0 0 1 3.046.275l.047.022.116.057a4.58 4.58 0 0 1 2.188 2.399q.313.765.315 1.595a4.2 4.2 0 0 1-.134 1.223.12.12 0 0 0 .03.115q.89.91 1.183 2.17.433 2.138-.887 3.854l-.136.166a4.55 4.55 0 0 1-2.201 1.388.12.12 0 0 0-.081.076c-.191.551-.383 1.023-.74 1.494-.9 1.187-2.222 1.846-3.711 1.838q-1.78-.009-3.157-1.302a.11.11 0 0 0-.105-.024c-.388.125-.78.143-1.204.138a4.44 4.44 0 0 1-1.945-.466 4.54 4.54 0 0 1-1.61-1.335c-.152-.202-.303-.392-.414-.617a6 6 0 0 1-.37-.961 4.6 4.6 0 0 1-.014-2.298.1.1 0 0 0 .006-.056.1.1 0 0 0-.027-.048 4.5 4.5 0 0 1-1.034-1.651 3.9 3.9 0 0 1-.251-1.192 5.2 5.2 0 0 1 .141-1.6Q3.659 7.92 5.086 6.97q.318-.212.601-.33a6 6 0 0 1 .646-.227.1.1 0 0 0 .065-.066 4.5 4.5 0 0 1 .829-1.615 4.54 4.54 0 0 1 1.837-1.388";
const CODEX_PROMPT: &str = "M12.546 13.909a.637.637 0 0 0 0 1.272h3.636a.637.637 0 1 0 0-1.272zM8.462 9.23a.637.637 0 0 0-1.106.631l1.272 2.224-1.266 2.136a.636.636 0 1 0 1.095.649l1.454-2.455a.64.64 0 0 0 .005-.64z";

/// `ClaudeMark` in its mono variant.
pub fn claude_mark() -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><path fill="currentColor" fill-rule="evenodd" clip-rule="evenodd" d="{CLAUDE_CODE}"/></svg>"#
    )
}

/// `CodexMark` in its mono variant: the outline's stroke is 2 up to 14 px, else 1.6.
pub fn codex_mark(size: f32) -> String {
    let stroke = if size <= 14.0 { 2.0 } else { 1.6 };
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><g transform="translate(12 12) scale(1.1) translate(-12 -12)"><path fill="none" stroke="currentColor" stroke-width="{stroke}" stroke-linejoin="round" d="{CODEX_FLOWER}z"/><path fill="currentColor" stroke="currentColor" stroke-width="0.5" stroke-linejoin="round" d="{CODEX_PROMPT}"/></g></svg>"#
    )
}

/// A filled or self-stroked SVG drawn at `size`.
pub fn mark(markup: &str, size: f32) -> Svg {
    svg().data(markup.as_bytes()).size(css(size)).flex_none()
}
