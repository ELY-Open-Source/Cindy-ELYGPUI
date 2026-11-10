//! `userMessageCollapse.ts`'s estimates, which stand in until the bubble is measured.

use crate::{rich_answer::js::utf16_len, theme_import::color::js_trim};

/// `LONG_USER_MESSAGE_VISUAL_LINE_THRESHOLD`.
pub const LONG: usize = 14;

const UNITS_PER_LINE: usize = 60;
const MIN_UNITS_PER_LINE: f64 = 24.0;

/// `WIDE_CHAR_RE`.
fn wide(c: char) -> bool {
    matches!(c, '\u{1100}'..='\u{115f}' | '\u{2e80}'..='\u{303e}' | '\u{3041}'..='\u{4dbf}'
        | '\u{4e00}'..='\u{9fff}' | '\u{a000}'..='\u{a4cf}' | '\u{ac00}'..='\u{d7a3}'
        | '\u{f900}'..='\u{faff}' | '\u{fe30}'..='\u{fe4f}' | '\u{ff00}'..='\u{ff60}'
        | '\u{ffe0}'..='\u{ffe6}')
}

/// `split(LINE_BREAK_RE)`.
fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.split("\r\n").flat_map(|part| part.split(['\r', '\n']))
}

/// `estimateVisualLineCount`.
fn visual_lines(line: &str) -> usize {
    let units = utf16_len(line) + line.chars().filter(|c| wide(*c)).count();
    units.div_ceil(UNITS_PER_LINE).max(1)
}

/// `shouldAutoCollapseUserMessageContent`: the guess before the first measure.
pub fn should_collapse(content: &str, threshold: usize) -> bool {
    let trimmed = js_trim(content);
    let mut total = 0;
    !trimmed.is_empty()
        && lines(trimmed).any(|line| {
            total += visual_lines(line);
            total > threshold
        })
}

/// `mayExceedVisualLineThreshold`: false for text no bubble width can fold.
pub fn may_exceed(content: &str, threshold: usize) -> bool {
    let trimmed = js_trim(content);
    let logical = lines(trimmed).count() as f64;
    !trimmed.is_empty()
        && logical + utf16_len(trimmed) as f64 * 2.0 / MIN_UNITS_PER_LINE > threshold as f64
}
