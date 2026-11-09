//! `fuzzyTitleMatch`: the query's characters in order in the title, in UTF-16 positions.

use crate::{localdb::cjk::letter_or_number, theme_import::color::js_trim};

const WORD_BOUNDARY: f64 = 15.0;
const CONSECUTIVE: f64 = 20.0;
const EXACT_CASE: f64 = 1.0;
const TRUE_PREFIX: f64 = 2.0;

#[derive(Debug, Clone, PartialEq)]
pub struct TitleMatch {
    pub score: f64,
    /// UTF-16 positions in the title.
    pub indices: Vec<usize>,
}

/// `isWordBoundary`: a half of a pair is no letter, as `\p{L}` tests one code unit.
fn word_boundary(text: &[u16], i: usize) -> bool {
    if i == 0 {
        return true;
    }
    let prev = text[i - 1];
    let letter = char::from_u32(u32::from(prev)).is_some_and(letter_or_number);
    if !letter {
        return true;
    }
    (97..=122).contains(&prev) && (65..=90).contains(&text[i])
}

pub fn title_match(title: &str, query: &str) -> Option<TitleMatch> {
    let trimmed = js_trim(query);
    if trimmed.is_empty() {
        return Some(TitleMatch {
            score: 0.0,
            indices: Vec::new(),
        });
    }
    if title.is_empty() {
        return None;
    }
    let units = |s: &str| s.encode_utf16().collect::<Vec<_>>();
    let (t, t_lower) = (units(title), units(&title.to_lowercase()));
    let (q, q_lower) = (units(trimmed), units(&trimmed.to_lowercase()));
    if q.len() > t.len() {
        return None;
    }
    let mut indices = Vec::with_capacity(q.len());
    let mut score = 0.0;
    let mut last: Option<usize> = None;
    let mut ti = 0;
    for (qi, &qu) in q.iter().enumerate() {
        // Lowering can change the length; a missing unit matches nothing, as `NaN`.
        let qc = q_lower.get(qi);
        while ti < t.len() && t_lower.get(ti) != qc {
            ti += 1;
        }
        if ti >= t.len() {
            return None;
        }
        indices.push(ti);
        let mut ch = 1.0;
        if last.is_some_and(|l| ti == l + 1) {
            ch += CONSECUTIVE;
        } else if word_boundary(&t, ti) {
            ch += WORD_BOUNDARY;
        }
        if t[ti] == qu {
            ch += EXACT_CASE;
        }
        if qi == 0 {
            ch *= 2.0;
            if ti == 0 {
                ch += TRUE_PREFIX;
            }
        }
        score += ch;
        last = Some(ti);
        ti += 1;
    }
    score += 16.0 / (t.len() as f64 + 16.0);
    Some(TitleMatch { score, indices })
}
