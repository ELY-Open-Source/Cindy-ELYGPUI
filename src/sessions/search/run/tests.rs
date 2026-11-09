use std::time::Duration;

use gpui::{TestAppContext, VisualTestContext};
use rusqlite::Connection;

use super::{LIMIT, Status, fanout, state, sync, unnamed_label};
use crate::{
    localdb::{
        self,
        search::{
            conversations::{
                Response,
                tests::{say, schema, task},
            },
            merge::{Item, MatchKind, Summary},
        },
    },
    sessions::{
        search::Search,
        view_tests::{click, open},
    },
};

const KEYWORD: &str = "本次请求已禁用语义检索, 仅用 FTS。";
const HYBRID: &str = "聊天记录语义索引未启用, 本次仅用 FTS 全文检索。";

fn item(id: &str, kind: MatchKind, rank: f64, updated: i64, sent: Option<i64>) -> Item {
    Item {
        session: Summary {
            id: id.to_owned(),
            source: "desktop".to_owned(),
            updated_at: updated,
            user_send_at: sent,
            ..Summary::default()
        },
        kind,
        title_indices: Vec::new(),
        title_score: None,
        hits: Vec::new(),
        rank,
    }
}

fn ids(response: &Response) -> Vec<&str> {
    response
        .results
        .iter()
        .map(|r| r.session.id.as_str())
        .collect()
}

#[test]
fn the_fanout_drops_cindy_make_rows_keeps_the_best_copy_and_sorts_as_the_renderer() {
    let mut make = item("m", MatchKind::Title, 9.0, 1, None);
    make.session.source = "cindy-make-merge".to_owned();
    let page = Response {
        query: "q".to_owned(),
        results: vec![
            item("c", MatchKind::Content, 5.0, 1, None),
            make,
            item("a", MatchKind::Content, 1.0, 10, None),
            item("t", MatchKind::Both, 0.5, 1, None),
            item("a", MatchKind::Content, 5.0, 2, Some(20)),
        ],
        vector_skip_reason: Some(KEYWORD),
        pool_capped: false,
    };
    let merged = fanout(&page, true, "relevance");
    assert_eq!(
        ids(&merged),
        ["t", "a", "c"],
        "the later send breaks the rank tie"
    );
    assert_eq!(merged.results[1].session.user_send_at, Some(20));
    assert_eq!(
        (merged.query.as_str(), merged.vector_skip_reason),
        ("q", Some(KEYWORD))
    );
    assert!(merged.pool_capped, "the keyword page's cap counts");
    assert_eq!(ids(&fanout(&page, false, "activityAsc")), ["t", "c", "a"]);
    assert_eq!(ids(&fanout(&page, false, "activityDesc")), ["a", "t", "c"]);
    let many = Response {
        results: (0..30)
            .map(|i| item(&format!("{i:02}"), MatchKind::Title, 1.0, 1, None))
            .collect(),
        ..Response::default()
    };
    let merged = fanout(&many, false, "relevance");
    assert_eq!(merged.results.len(), LIMIT);
    assert_eq!(merged.results[0].session.id, "00");
    assert!(!merged.pool_capped);
}

#[test]
fn the_unnamed_label_is_sent_as_written() {
    assert_eq!(
        unnamed_label(" Untitled ".into()),
        Ok(Some(" Untitled ".into()))
    );
    assert_eq!(unnamed_label("  ".into()), Ok(None));
    assert!(unnamed_label("x".repeat(121)).is_err());
    assert_eq!(unnamed_label("中".repeat(120)), Ok(Some("中".repeat(120))));
}

/// The search row open and a database serving `fill`'s tasks, if any.
fn search<'a>(
    cx: &'a mut TestAppContext,
    dir: &std::path::Path,
    fill: Option<fn(&Connection)>,
) -> &'a mut VisualTestContext {
    let cx = open(cx, dir);
    if let Some(fill) = fill {
        let db = Connection::open_in_memory().unwrap();
        schema(&db);
        fill(&db);
        cx.update(|_, cx| localdb::serve(db, cx));
    }
    click(cx, "nav-search-open");
    cx
}

fn tasks(db: &Connection) {
    task(db, "t", "Fix login flow", 10);
    task(db, "c", "Other work", 20);
    say(db, "m1", "c", "\"the login page broke\"");
}

fn wait(cx: &mut VisualTestContext, ms: u64) {
    cx.executor().advance_clock(Duration::from_millis(ms));
    cx.run_until_parked();
}

/// The page's query, task ids and skip reason.
type Shown = (String, Vec<String>, Option<&'static str>);

fn now(cx: &mut VisualTestContext) -> (Status, Option<Shown>) {
    cx.update(|_, cx| {
        let (status, response) = state(cx);
        let shown = response.map(|r| {
            let found = r.results.iter().map(|i| i.session.id.clone()).collect();
            (r.query.clone(), found, r.vector_skip_reason)
        });
        (status, shown)
    })
}

fn shown(query: &str, found: &[&str], reason: &'static str) -> Option<Shown> {
    let found = found.iter().map(|s| (*s).to_owned()).collect();
    Some((query.to_owned(), found, Some(reason)))
}

#[gpui::test]
fn the_keyword_page_shows_at_250ms_and_the_hybrid_page_replaces_it(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = search(cx, dir.path(), Some(tasks));
    cx.simulate_input("login");
    assert_eq!(now(cx), (Status::Searching, None));
    wait(cx, 249);
    assert_eq!(now(cx), (Status::Searching, None));
    wait(cx, 1);
    assert_eq!(
        now(cx),
        (Status::Done, shown("login", &["t", "c"], KEYWORD))
    );
    cx.simulate_input(" ");
    assert_eq!(now(cx).0, Status::Done, "the trimmed query did not change");
    wait(cx, 650);
    assert_eq!(now(cx), (Status::Done, shown("login", &["t", "c"], HYBRID)));
}

#[gpui::test]
fn an_empty_page_waits_for_both_passes(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = search(cx, dir.path(), Some(tasks));
    cx.simulate_input("nothing");
    wait(cx, 250);
    assert_eq!(now(cx), (Status::Searching, None));
    wait(cx, 650);
    assert_eq!(now(cx), (Status::Done, shown("nothing", &[], HYBRID)));
}

#[gpui::test]
fn a_newer_query_starts_over_and_only_it_publishes(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = search(cx, dir.path(), Some(tasks));
    cx.simulate_input("fix");
    wait(cx, 200);
    cx.simulate_input(" flow");
    wait(cx, 50);
    assert_eq!(
        now(cx),
        (Status::Searching, None),
        "the first timers were cleared"
    );
    wait(cx, 200);
    assert_eq!(now(cx), (Status::Done, shown("fix flow", &["t"], KEYWORD)));
}

#[gpui::test]
fn an_option_change_searches_again_and_keeps_the_page_meanwhile(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = search(cx, dir.path(), Some(tasks));
    cx.simulate_input("login");
    wait(cx, 900);
    cx.update(|_, cx| {
        cx.global_mut::<Search>().options.status = "archived";
        sync(cx);
    });
    assert_eq!(
        now(cx),
        (Status::Searching, shown("login", &["t", "c"], HYBRID))
    );
    wait(cx, 900);
    let none = Some(("login".to_owned(), Vec::new(), None));
    assert_eq!(
        now(cx),
        (Status::Done, none),
        "no candidate is an empty page"
    );
    click(cx, "nav-search-clear");
    assert_eq!(now(cx), (Status::Idle, None));
}

#[gpui::test]
fn without_a_database_the_search_fails_once_both_passes_settle(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = search(cx, dir.path(), None);
    cx.simulate_input("login");
    wait(cx, 250);
    assert_eq!(now(cx), (Status::Searching, None));
    wait(cx, 650);
    assert_eq!(now(cx), (Status::Error, None));
}
