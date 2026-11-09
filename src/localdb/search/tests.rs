use serde_json::{Value, json};

use super::{
    PREVIEW_CHARS, fts,
    fuzzy::title_match,
    merge::{ContentHit, Item, MatchKind, Sort, Summary, merge},
    snippet::content_preview,
    visible::visible_text,
};
use crate::{
    cindy_json::js_number,
    rich_answer::js::{self, Js},
};

/// A case's content as `JSON.parse` hands it to Cindy.
fn parsed(value: &Value) -> Js {
    js::parse(&value.to_string()).expect("case content parses")
}

fn golden() -> (Value, Value) {
    let parse = |raw: &str| serde_json::from_str::<Value>(raw).expect("golden JSON");
    (
        parse(include_str!("../../../tests/fixtures/search/cases.json")),
        parse(include_str!("../../../tests/fixtures/search/expected.json")),
    )
}

fn pairs<'a>(cases: &'a Value, expected: &'a Value, key: &str) -> Vec<(&'a Value, &'a Value)> {
    let (cases, expected) = (
        cases[key].as_array().unwrap(),
        expected[key].as_array().unwrap(),
    );
    assert_eq!(cases.len(), expected.len(), "{key}");
    cases.iter().zip(expected).collect()
}

#[test]
fn titles_match_as_fuzzy_title_match_does() {
    let (cases, expected) = golden();
    for (case, want) in pairs(&cases, &expected, "fuzzy") {
        let (title, query) = (case[0].as_str().unwrap(), case[1].as_str().unwrap());
        let got = title_match(title, query)
            .map(|m| json!({ "score": js_number(m.score), "indices": m.indices }));
        assert_eq!(
            got.as_ref().unwrap_or(&Value::Null),
            want,
            "{title:?} {query:?}"
        );
    }
}

#[test]
fn previews_and_snippets_match_normalize_conversation_content_preview() {
    let (cases, expected) = golden();
    for (case, want) in pairs(&cases, &expected, "preview") {
        let [role, content, query, max, meta] = [0, 1, 2, 3, 4].map(|i| &case[i]);
        let max = max.as_u64().map_or(PREVIEW_CHARS, |m| m as usize);
        let got = content_preview(
            role.as_str().unwrap(),
            &parsed(content),
            query.as_str().unwrap(),
            max,
            meta.as_str(),
        );
        let got = json!({
            "preview": got.preview,
            "snippet": got.snippet,
            "keywordMatchedVisibleText": got.keyword_matched,
        });
        assert_eq!(&got, want, "{case}");
    }
}

#[test]
fn visible_text_matches_visible_message_text() {
    let (cases, expected) = golden();
    for (case, want) in pairs(&cases, &expected, "visible") {
        let got = visible_text(
            case[0].as_str().unwrap(),
            &parsed(&case[1]),
            case[2].as_bool().unwrap(),
        );
        assert_eq!(got, want.as_str().unwrap(), "{case}");
    }
}

fn ms(value: &Value) -> i64 {
    let at: jiff::Timestamp = value.as_str().unwrap().parse().unwrap();
    at.as_millisecond()
}

fn summary(value: &Value) -> Summary {
    Summary {
        id: value["id"].as_str().unwrap().to_owned(),
        updated_at: ms(&value["updatedAt"]),
        ..Summary::default()
    }
}

fn shown(item: &Item) -> Value {
    let kind = match item.kind {
        MatchKind::Title => "title",
        MatchKind::Content => "content",
        MatchKind::Both => "both",
    };
    let hits: Vec<&str> = item.hits.iter().map(|h| h.message_id.as_str()).collect();
    json!({
        "id": item.session.id,
        "matchKind": kind,
        "titleMatchIndices": item.title_indices,
        "titleScore": item.title_score.map(js_number),
        "contentHit": hits.first(),
        "contentHits": hits,
        "rankScore": js_number(item.rank),
    })
}

#[test]
fn results_merge_and_sort_as_cindy_merges_them() {
    let (cases, expected) = golden();
    for (case, want) in pairs(&cases, &expected, "merge") {
        let titles = || {
            case["titleMatches"].as_array().unwrap().iter().map(|t| {
                let indices = t["indices"].as_array().unwrap();
                let m = super::fuzzy::TitleMatch {
                    score: t["score"].as_f64().unwrap(),
                    indices: indices
                        .iter()
                        .map(|i| i.as_u64().unwrap() as usize)
                        .collect(),
                };
                (summary(&t["session"]), m)
            })
        };
        let contents = || {
            case["contentHits"].as_array().unwrap().iter().map(|c| {
                let hit = ContentHit {
                    message_id: c["hit"]["messageId"].as_str().unwrap().to_owned(),
                    created_at: ms(&c["hit"]["createdAt"]),
                    score: c["hit"]["score"].as_f64().unwrap(),
                    ..ContentHit::default()
                };
                (summary(&c["session"]), hit)
            })
        };
        let limit = case["limit"].as_u64().unwrap() as usize;
        for (key, sort) in [
            ("relevance", Sort::Relevance),
            ("activityDesc", Sort::ActivityDesc),
            ("activityAsc", Sort::ActivityAsc),
        ] {
            let got = merge(titles().collect(), contents().collect(), limit, sort);
            let got: Vec<Value> = got.iter().map(shown).collect();
            assert_eq!(Value::from(got), want[key], "{} {key}", case["name"]);
        }
    }
}

#[test]
fn fts_queries_and_fusion_match_chat_history_search_pure() {
    let (cases, expected) = golden();
    let (cases, want) = (&cases["fts"], &expected["fts"]);
    for (query, want) in pairs(cases, want, "tokens") {
        let query = query.as_str().unwrap();
        let got = json!([fts::tokens(query), fts::messages_match(query)]);
        assert_eq!(&got, want, "{query:?}");
    }
    for (case, want) in pairs(cases, want, "visible") {
        let (text, query) = (case[0].as_str().unwrap(), case[1].as_str().unwrap());
        assert_eq!(
            Value::from(fts::visible_matches(text, query)),
            *want,
            "{case}"
        );
    }
    for (ids, want) in pairs(cases, want, "fuse") {
        let ids: Vec<String> = serde_json::from_value(ids.clone()).unwrap();
        let got: Vec<Value> = fts::fuse(&ids)
            .iter()
            .map(|e| json!([e.message_id, js_number(e.score), e.fts_rank]))
            .collect();
        assert_eq!(Value::from(got), *want, "{ids:?}");
    }
}
