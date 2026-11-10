//! `userMessageCollapse.ts`'s coarse filter: the text no bubble width can fold.

use crate::{rich_answer::js::utf16_len, theme_import::color::js_trim};

/// `LONG_USER_MESSAGE_VISUAL_LINE_THRESHOLD`.
pub const LONG: usize = 14;

const MIN_UNITS_PER_LINE: f64 = 24.0;

/// `split(LINE_BREAK_RE)`.
fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.split("\r\n").flat_map(|part| part.split(['\r', '\n']))
}

/// `mayExceedVisualLineThreshold`: false for text no bubble width can fold.
pub fn may_exceed(content: &str, threshold: usize) -> bool {
    let trimmed = js_trim(content);
    let logical = lines(trimmed).count() as f64;
    !trimmed.is_empty()
        && logical + utf16_len(trimmed) as f64 * 2.0 / MIN_UNITS_PER_LINE > threshold as f64
}
