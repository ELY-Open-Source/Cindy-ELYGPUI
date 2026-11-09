//! `chatHistorySearch.pure.ts`: the `messages_fts` query, its visible-text check and the fusion.

use indexmap::IndexMap;

use super::merge::{COLLATOR, sign};
use crate::localdb::cjk::{letter_or_number, mark, seg};

/// `RRF_K`.
const RRF_K: f64 = 60.0;
const TOKEN_CAP: usize = 32;
const TOKEN_CHAR_CAP: usize = 256;
/// `FTS_MATCH_CHAR_CAP`, in UTF-16 units.
const MATCH_CHAR_CAP: usize = 2_048;

/// `FusedEntry` of the FTS arm.
#[derive(Debug, Clone, PartialEq)]
pub struct Fused {
    pub message_id: String,
    pub score: f64,
    /// 1-based.
    pub fts_rank: usize,
}

/// `FTS_TOKEN_RE`: a letter or digit, then letters, digits, marks and variation selectors.
fn runs(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut run = String::new();
    for c in text.chars() {
        let tail = !run.is_empty() && (mark(c) || ('\u{E0100}'..='\u{E01EF}').contains(&c));
        if letter_or_number(c) || tail {
            run.push(c);
        } else if !run.is_empty() {
            out.push(std::mem::take(&mut run));
        }
    }
    if !run.is_empty() {
        out.push(run);
    }
    out
}

/// `extractMessagesFtsTokens`: unique, each cut to 256 code points, 32 at most.
pub fn tokens(query: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for token in runs(query) {
        let token: String = token.chars().take(TOKEN_CHAR_CAP).collect();
        if !out.contains(&token) {
            out.push(token);
        }
    }
    out.truncate(TOKEN_CAP);
    out
}

/// `tokenizeVisibleMessagesFtsText`: segmented as the index is, then lowered.
fn visible_tokens(text: &str) -> Vec<String> {
    runs(&seg(text)).iter().map(|t| t.to_lowercase()).collect()
}

/// `visibleTextMatchesMessagesFtsQuery`: a query token appears as adjacent visible tokens.
pub fn visible_matches(visible: &str, query: &str) -> bool {
    if visible.is_empty() {
        return false;
    }
    let haystack = visible_tokens(visible);
    if haystack.is_empty() {
        return false;
    }
    tokens(query).iter().any(|token| {
        let needle = visible_tokens(token);
        !needle.is_empty()
            && needle.len() <= haystack.len()
            && haystack.windows(needle.len()).any(|w| w == needle)
    })
}

/// `buildMessagesFtsMatch(query, 'OR')`: quoted segmented tokens within 2048 units.
pub fn messages_match(query: &str) -> Option<String> {
    let mut out = String::new();
    for token in tokens(query) {
        let quoted = format!("\"{}\"", seg(&token).replace('"', "\"\""));
        let next = if out.is_empty() {
            quoted
        } else {
            format!("{out} OR {quoted}")
        };
        if next.encode_utf16().count() > MATCH_CHAR_CAP {
            break;
        }
        out = next;
    }
    (!out.is_empty()).then_some(out)
}

/// `fuseRRF` over the FTS arm: `1 / (k + rank)`, best first, ties by id.
pub fn fuse(ranked: &[String]) -> Vec<Fused> {
    let mut by_id: IndexMap<&str, Fused> = IndexMap::new();
    for (i, id) in ranked.iter().enumerate() {
        let entry = by_id.entry(id).or_insert_with(|| Fused {
            message_id: id.clone(),
            score: 0.0,
            fts_rank: 0,
        });
        entry.score += 1.0 / (RRF_K + i as f64 + 1.0);
        entry.fts_rank = i + 1;
    }
    let mut fused: Vec<Fused> = by_id.into_values().collect();
    fused.sort_by(|a, b| {
        sign(b.score - a.score).then_with(|| COLLATOR.compare(&a.message_id, &b.message_id))
    });
    fused
}
