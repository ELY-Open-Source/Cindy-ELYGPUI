//! `mergeConversationSearchResults` and `sortConversationSearchResults`.

use std::{cmp::Ordering, sync::LazyLock};

use icu_collator::CollatorBorrowed;
use indexmap::IndexMap;

use super::fuzzy::TitleMatch;

/// `localeCompare` for ids, Cindy's main process on ICU's root order.
static COLLATOR: LazyLock<CollatorBorrowed<'static>> = LazyLock::new(|| {
    CollatorBorrowed::try_new(Default::default(), Default::default())
        .expect("the compiled root collation loads")
});

/// `ConversationSearchSortBy`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sort {
    Relevance,
    ActivityDesc,
    ActivityAsc,
}

/// The fields of `ConversationSearchSessionSummary` the order reads.
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    pub id: String,
    /// `updatedAt` in ms.
    pub updated_at: i64,
}

/// The fields of `ConversationSearchContentHit` the order reads.
#[derive(Debug, Clone, PartialEq)]
pub struct ContentHit {
    pub message_id: String,
    /// `createdAt` in ms.
    pub created_at: i64,
    pub score: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MatchKind {
    Title,
    Content,
    Both,
}

/// `ConversationSearchResultItem`; `contentHit` is the first of `hits`.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub session: Summary,
    pub kind: MatchKind,
    pub title_indices: Vec<usize>,
    pub title_score: Option<f64>,
    pub hits: Vec<ContentHit>,
    pub rank: f64,
}

/// A JavaScript difference in a comparator: `NaN` and zero both fall through.
fn sign(diff: f64) -> Ordering {
    diff.partial_cmp(&0.0).unwrap_or(Ordering::Equal)
}

pub fn merge(
    titles: Vec<(Summary, TitleMatch)>,
    contents: Vec<(Summary, ContentHit)>,
    limit: usize,
    by: Sort,
) -> Vec<Item> {
    let mut by_id: IndexMap<String, Item> = IndexMap::new();
    for (i, (session, m)) in titles.into_iter().enumerate() {
        let rank = 2_000_000.0 + m.score * 100.0 - i as f64;
        by_id.insert(
            session.id.clone(),
            Item {
                session,
                kind: MatchKind::Title,
                title_indices: m.indices,
                title_score: Some(m.score),
                hits: Vec::new(),
                rank,
            },
        );
    }
    for (i, (session, hit)) in contents.into_iter().enumerate() {
        if let Some(existing) = by_id.get_mut(&session.id) {
            let score = hit.score;
            insert_hit(&mut existing.hits, hit);
            if existing.title_score.is_some() {
                existing.kind = MatchKind::Both;
                existing.rank += 50_000.0 + score * 10.0;
            }
            continue;
        }
        let rank = 1_000_000.0 + hit.score * 10.0 - i as f64;
        by_id.insert(
            session.id.clone(),
            Item {
                session,
                kind: MatchKind::Content,
                title_indices: Vec::new(),
                title_score: None,
                hits: vec![hit],
                rank,
            },
        );
    }
    let mut results: Vec<Item> = by_id.into_values().collect();
    sort(&mut results, by);
    results.truncate(limit);
    results
}

/// `insertContentHit`: best score, then newest, then id.
fn insert_hit(hits: &mut Vec<ContentHit>, hit: ContentHit) {
    if hits.iter().any(|h| h.message_id == hit.message_id) {
        return;
    }
    hits.push(hit);
    hits.sort_by(|a, b| {
        sign(b.score - a.score)
            .then(b.created_at.cmp(&a.created_at))
            .then_with(|| COLLATOR.compare(&a.message_id, &b.message_id))
    });
}

/// `sortConversationSearchResults`.
pub fn sort(results: &mut [Item], by: Sort) {
    results.sort_by(|a, b| match by {
        Sort::ActivityDesc => b
            .session
            .updated_at
            .cmp(&a.session.updated_at)
            .then_with(|| relevance(a, b)),
        Sort::ActivityAsc => a
            .session
            .updated_at
            .cmp(&b.session.updated_at)
            .then_with(|| relevance(a, b)),
        Sort::Relevance => relevance(a, b),
    });
}

fn relevance(a: &Item, b: &Item) -> Ordering {
    let titled = |r: &Item| matches!(r.kind, MatchKind::Title | MatchKind::Both);
    titled(b)
        .cmp(&titled(a))
        .then(sign(b.rank - a.rank))
        .then(b.session.updated_at.cmp(&a.session.updated_at))
        .then_with(|| COLLATOR.compare(&a.session.id, &b.session.id))
}
