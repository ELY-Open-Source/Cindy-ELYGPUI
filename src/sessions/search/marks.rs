//! `highlightSegments` and `renderKeywordHighlights` as gpui text highlights; a highlight has no
//! padding or radius, so the marks' `px-0.5` and `rounded-[2px]` are not drawn.

use std::ops::Range;

use gpui::{App, FontWeight, HighlightStyle, StyledText};

use crate::{localdb::search::snippet::keyword_ranges, theme::Appearance};

/// The byte offset of a UTF-16 offset on a code point boundary.
fn byte_at(text: &str, unit: usize) -> usize {
    let mut units = 0;
    for (at, c) in text.char_indices() {
        if units >= unit {
            return at;
        }
        units += c.len_utf16();
    }
    text.len()
}

fn styled(text: &str, spans: Vec<Range<usize>>, style: HighlightStyle) -> StyledText {
    let highlights = spans
        .into_iter()
        .map(|r| (byte_at(text, r.start)..byte_at(text, r.end), style));
    StyledText::new(text.to_owned()).with_highlights(highlights)
}

/// `SEARCH_MARK_CLASS`: `bg-search-match-bg font-medium text-search-match-fg`.
fn keyword_style(cx: &App) -> HighlightStyle {
    let appearance = cx.global::<Appearance>();
    HighlightStyle {
        color: Some(appearance.hsl_token("search-match-fg")),
        background_color: Some(appearance.hsl_token("search-match-bg")),
        font_weight: Some(FontWeight::MEDIUM),
        ..HighlightStyle::default()
    }
}

/// `renderKeywordHighlights(text, query)`.
pub fn keywords(text: &str, query: &str, cx: &App) -> StyledText {
    let spans = keyword_ranges(text, query)
        .into_iter()
        .map(|r| r.start..r.end)
        .collect();
    styled(text, spans, keyword_style(cx))
}

/// `highlightSegments`' spans: UTF-16 indices widened to whole code points, then merged.
fn segments(title: &str, indices: &[usize]) -> Vec<Range<usize>> {
    let units: Vec<u16> = title.encode_utf16().collect();
    let mut spans: Vec<Range<usize>> = Vec::new();
    for &i in indices.iter().filter(|&&i| i < units.len()) {
        let (mut start, mut end) = (i, i + 1);
        match units[i] {
            0xDC00..=0xDFFF => start = start.saturating_sub(1),
            0xD800..=0xDBFF => end += 1,
            _ => {}
        }
        match spans.last_mut() {
            Some(prev) if start <= prev.end => {
                prev.start = prev.start.min(start);
                prev.end = prev.end.max(end);
            }
            _ => spans.push(start..end),
        }
    }
    spans
}

/// `highlightSegments(title, indices)` with its default mark: semibold in `msg-assistant-text`.
pub fn title(title: &str, indices: &[usize], cx: &App) -> StyledText {
    let style = HighlightStyle {
        color: Some(cx.global::<Appearance>().token("msg-assistant-text")),
        font_weight: Some(FontWeight::SEMIBOLD),
        ..HighlightStyle::default()
    };
    styled(title, segments(title, indices), style)
}

#[cfg(test)]
mod tests {
    use super::{byte_at, segments};

    #[test]
    fn indices_widen_to_code_points_and_merge() {
        assert_eq!(segments("abcd", &[0, 1, 3, 9]), [0..2, 3..4]);
        assert_eq!(
            segments("a😀b", &[2]),
            vec![1..3],
            "a low surrogate takes its pair"
        );
        assert_eq!(segments("a😀b", &[1, 3]), vec![1..4]);
        assert!(segments("", &[0]).is_empty());
    }

    #[test]
    fn utf16_offsets_map_to_bytes() {
        let text = "中😀x";
        assert_eq!([0, 1, 3, 4].map(|u| byte_at(text, u)), [0, 3, 7, 8]);
    }
}
