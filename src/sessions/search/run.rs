//! `useConversationSearch`'s search effect: a keyword pass after 250ms and a hybrid pass after
//! 900ms on the database worker; only the latest request publishes its merged page.

use std::{cmp::Ordering, time::Duration};

use futures::channel::oneshot;
use gpui::{App, Task};

use super::{Search, trimmed};
use crate::{
    i18n::Translator,
    localdb::{
        LocalDb, State,
        search::{
            conversations::{self, Request, Response},
            mcp_app::TrustLabels,
            merge::{COLLATOR, Item, MatchKind, Summary, sign},
        },
    },
    theme_import::color::js_trim,
};

const DEBOUNCE: Duration = Duration::from_millis(250);
const SEMANTIC_DEBOUNCE: Duration = Duration::from_millis(900);
/// `SEARCH_LIMIT`.
pub const LIMIT: usize = 24;
/// `UNNAMED_LABEL_MAX_LENGTH`, in UTF-16 units.
const UNNAMED_LABEL_MAX: usize = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Status {
    #[default]
    Idle,
    Searching,
    Done,
    Error,
}

/// The effect's dependencies.
#[derive(Debug, Clone, PartialEq)]
struct Key {
    query: String,
    sort: &'static str,
    status: &'static str,
    agent: &'static str,
    last_activity: &'static str,
}

/// `status`, `response` and the request in flight.
#[derive(Default)]
pub struct Run {
    seq: u64,
    key: Option<Key>,
    status: Status,
    response: Option<Response>,
    pages: Pages,
    /// Dropping them clears the timers.
    passes: Vec<Task<()>>,
}

#[derive(Default)]
struct Pages {
    keyword: Option<Response>,
    semantic: Option<Response>,
    keyword_settled: bool,
    semantic_settled: bool,
}

#[derive(Debug, Clone, Copy)]
enum Pass {
    Keyword,
    Hybrid,
}

/// The search's state and the page it shows.
pub fn state(cx: &App) -> (Status, Option<&Response>) {
    let run = &cx.global::<Search>().run;
    (run.status, run.response.as_ref())
}

fn key(cx: &App) -> Key {
    let o = &cx.global::<Search>().options;
    Key {
        query: trimmed(cx).to_owned(),
        sort: o.sort,
        status: o.status,
        agent: o.agent,
        last_activity: o.last_activity,
    }
}

/// Runs the effect again once the query or an option changed.
pub fn sync(cx: &mut App) {
    let key = key(cx);
    let run = &mut cx.global_mut::<Search>().run;
    if run.key.as_ref() == Some(&key) {
        return;
    }
    run.seq += 1;
    run.key = Some(key.clone());
    run.passes.clear();
    run.pages = Pages::default();
    if key.query.is_empty() {
        (run.status, run.response) = (Status::Idle, None);
        log::debug!(target: "search", "idle");
        return;
    }
    run.status = Status::Searching;
    let seq = run.seq;
    log::info!(target: "search", "search {seq} for {} chars, sort {}", key.query.chars().count(), key.sort);
    let passes = [(Pass::Keyword, DEBOUNCE), (Pass::Hybrid, SEMANTIC_DEBOUNCE)]
        .map(|(pass, delay)| start(seq, pass, delay, key.clone(), cx));
    cx.global_mut::<Search>().run.passes = passes.into();
}

fn start(seq: u64, pass: Pass, delay: Duration, key: Key, cx: &mut App) -> Task<()> {
    cx.spawn(async move |cx| {
        cx.background_executor().timer(delay).await;
        let sort = key.sort;
        let job = cx.update(|cx| dispatch(pass, key, cx));
        let page = match job {
            Ok(answer) => match answer.await {
                Ok(Ok(page)) => Some(page),
                Ok(Err(error)) => {
                    log::warn!(target: "search", "{pass:?} pass failed: {error}");
                    None
                }
                Err(_) => {
                    log::warn!(target: "search", "{pass:?} pass: the database closed");
                    None
                }
            },
            Err(reason) => {
                log::warn!(target: "search", "{pass:?} pass not sent: {reason}");
                None
            }
        };
        cx.update(|cx| settle(seq, pass, page, sort, cx));
    })
}

/// `parseUnnamedLabel`: a blank label is none, a long one refused.
fn unnamed_label(label: String) -> Result<Option<String>, &'static str> {
    if js_trim(&label).is_empty() {
        return Ok(None);
    }
    if label.encode_utf16().count() > UNNAMED_LABEL_MAX {
        return Err("unnamedLabel must be at most 120 characters");
    }
    Ok(Some(label))
}

type Answer = oneshot::Receiver<rusqlite::Result<Response>>;

/// `localDb.conversations.search` with `semanticMode` for `pass`.
fn dispatch(pass: Pass, key: Key, cx: &App) -> Result<Answer, &'static str> {
    let State::Ready(db) = &cx.global::<LocalDb>().state else {
        return Err("the database is not open");
    };
    let t = cx.global::<Translator>();
    let unnamed = unnamed_label(t.t("ccAgent.common.unnamedSession", &[]))?;
    let labels = TrustLabels::new(t);
    let now = jiff::Timestamp::now().as_millisecond();
    Ok(db.call(move |conn| {
        let request = Request {
            query: &key.query,
            limit: Some(LIMIT as f64),
            sort: key.sort,
            keyword_only: matches!(pass, Pass::Keyword),
            status: key.status,
            agent_kind: key.agent,
            last_activity: key.last_activity,
            session_ids: None,
            working_dirs: None,
            unnamed_label: unnamed.as_deref(),
        };
        conversations::search(conn, &request, &labels, now)
    }))
}

/// `publishResults`: the hybrid page wins once it lands; an empty page waits for both passes.
fn settle(seq: u64, pass: Pass, page: Option<Response>, sort: &str, cx: &mut App) {
    let run = &mut cx.global_mut::<Search>().run;
    if run.seq != seq {
        log::debug!(target: "search", "{pass:?} pass of search {seq} is stale");
        return;
    }
    let pages = &mut run.pages;
    match pass {
        Pass::Keyword => (pages.keyword, pages.keyword_settled) = (page, true),
        Pass::Hybrid => (pages.semantic, pages.semantic_settled) = (page, true),
    }
    let settled = pages.keyword_settled && pages.semantic_settled;
    let Some(local) = pages.semantic.as_ref().or(pages.keyword.as_ref()) else {
        if settled {
            run.status = Status::Error;
            log::warn!(target: "search", "search {seq} failed");
            cx.refresh_windows();
        }
        return;
    };
    let keyword_capped = pages.keyword.as_ref().is_some_and(|p| p.pool_capped);
    let next = fanout(local, keyword_capped, sort);
    if next.results.is_empty() && !settled {
        return;
    }
    log::info!(target: "search", "search {seq} shows {} results after the {pass:?} pass", next.results.len());
    (run.response, run.status) = (Some(next), Status::Done);
    cx.refresh_windows();
}

/// `isCindyMakeSource`, with `CINDY_MAKE_ENABLED` false.
fn cindy_make(source: &str) -> bool {
    source == "cindy-make" || source == "cindy-make-merge"
}

/// `mergeConversationSearchFanout` over the local page and the keyword page's empty remote one.
fn fanout(local: &Response, keyword_capped: bool, sort: &str) -> Response {
    let mut kept: Vec<&Item> = Vec::new();
    for item in local
        .results
        .iter()
        .filter(|i| !cindy_make(&i.session.source))
    {
        match kept.iter().position(|k| k.session.id == item.session.id) {
            Some(at) if item.rank > kept[at].rank => kept[at] = item,
            Some(_) => {}
            None => kept.push(item),
        }
    }
    let mut results: Vec<Item> = kept.into_iter().cloned().collect();
    results.sort_by(|a, b| compare(a, b, sort));
    results.truncate(LIMIT);
    Response {
        query: local.query.clone(),
        results,
        vector_skip_reason: local.vector_skip_reason,
        pool_capped: local.pool_capped || keyword_capped,
    }
}

/// The renderer's `sessionActivityMs`: the later of the last send and the update.
fn activity(s: &Summary) -> i64 {
    s.user_send_at.unwrap_or(0).max(s.updated_at)
}

/// The renderer's `compareSearchResults`.
fn compare(a: &Item, b: &Item, sort: &str) -> Ordering {
    match sort {
        "activityDesc" => activity(&b.session)
            .cmp(&activity(&a.session))
            .then_with(|| relevance(a, b)),
        "activityAsc" => activity(&a.session)
            .cmp(&activity(&b.session))
            .then_with(|| relevance(a, b)),
        _ => relevance(a, b),
    }
}

fn relevance(a: &Item, b: &Item) -> Ordering {
    let titled = |r: &Item| matches!(r.kind, MatchKind::Title | MatchKind::Both);
    titled(b)
        .cmp(&titled(a))
        .then(sign(b.rank - a.rank))
        .then(activity(&b.session).cmp(&activity(&a.session)))
        .then_with(|| COLLATOR.compare(&a.session.id, &b.session.id))
}

#[cfg(test)]
pub(super) mod tests;
