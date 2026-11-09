//! `historyView.test.ts` and `historyViewLoading.test.ts`, the controller's cases.

use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
};

use futures::{
    FutureExt,
    channel::oneshot,
    future::{LocalBoxFuture, join_all},
};
use gpui::TestAppContext;

use super::*;
use crate::{
    history_view::{Row, project},
    rich_answer::js::{Js, Obj},
};

mod details;
mod locate;
mod pages;

type Reply<P> = LocalBoxFuture<'static, Result<P, String>>;
type PageReply = Reply<Page<Row>>;
type DetailReply = Reply<DetailPage<Row>>;
/// Replies a test settles in order.
pub type Senders<P> = Rc<RefCell<Vec<oneshot::Sender<Result<P, String>>>>>;

/// `row(id, role, content)`: one second apart from 1700000000000.
pub fn row(id: usize, role: &str, content: &str) -> Row {
    let mut out = Obj::new();
    out.insert("id".into(), Js::Str(id.to_string()));
    out.insert("clientId".into(), Js::Str(format!("c{id}")));
    out.insert("role".into(), Js::Str(role.into()));
    out.insert("content".into(), Js::Str(content.into()));
    let at = crate::js_date::iso(1_700_000_000_000 + id as i64 * 1000).unwrap();
    out.insert("createdAt".into(), Js::Str(at));
    Js::Obj(out)
}

pub fn page(items: Vec<Item<Row>>, has_more: bool, next: Option<&str>) -> Page<Row> {
    Page {
        items,
        has_more,
        next_cursor: next.map(str::to_owned),
    }
}

pub fn ready<P: 'static>(value: Result<P, String>) -> Reply<P> {
    async move { value }.boxed_local()
}

/// A reply the test settles later.
pub fn gate<P: 'static>() -> (oneshot::Sender<Result<P, String>>, Reply<P>) {
    let (send, receive) = oneshot::channel();
    (
        send,
        async move { receive.await.unwrap_or_else(|_| Err("dropped".into())) }.boxed_local(),
    )
}

type PageFn = Box<dyn FnMut(Option<String>) -> PageReply>;
type DetailFn = Box<dyn FnMut(Ref, Option<String>) -> DetailReply>;

/// A transport that answers queued replies first, then its default.
pub struct Fake {
    pub pages: RefCell<VecDeque<PageReply>>,
    pub page_default: RefCell<PageFn>,
    pub details: RefCell<VecDeque<DetailReply>>,
    pub detail_default: RefCell<DetailFn>,
    pub page_calls: RefCell<Vec<Option<String>>>,
    pub detail_calls: RefCell<Vec<Option<String>>>,
}

impl Fake {
    pub fn new(page: impl FnMut(Option<String>) -> PageReply + 'static) -> Rc<Self> {
        Rc::new(Fake {
            pages: RefCell::default(),
            page_default: RefCell::new(Box::new(page)),
            details: RefCell::default(),
            detail_default: RefCell::new(Box::new(|_, _| ready(Ok(detail(Vec::new(), None))))),
            page_calls: RefCell::default(),
            detail_calls: RefCell::default(),
        })
    }

    pub fn fixed(items: Vec<Item<Row>>) -> Rc<Self> {
        Fake::new(move |_| ready(Ok(page(items.clone(), false, None))))
    }
}

pub fn detail(messages: Vec<Row>, next: Option<&str>) -> DetailPage<Row> {
    DetailPage {
        messages,
        has_more: next.is_some(),
        next_cursor: next.map(str::to_owned),
    }
}

impl Transport<Row> for Fake {
    fn page(&self, before: Option<String>) -> PageReply {
        self.page_calls.borrow_mut().push(before.clone());
        match self.pages.borrow_mut().pop_front() {
            Some(reply) => reply,
            None => (self.page_default.borrow_mut())(before),
        }
    }

    fn details(&self, r: Ref, after: Option<String>) -> DetailReply {
        self.detail_calls.borrow_mut().push(after.clone());
        match self.details.borrow_mut().pop_front() {
            Some(reply) => reply,
            None => (self.detail_default.borrow_mut())(r, after),
        }
    }
}

pub fn view(fake: &Rc<Fake>, cx: &TestAppContext) -> View<Row> {
    View::new(
        fake.clone(),
        cx.foreground_executor().clone(),
        cx.executor(),
    )
}

#[gpui::test]
async fn regrouping_keeps_loaded_history_only_over_the_same_source_start(cx: &mut TestAppContext) {
    for overlap in [true, false] {
        let prefix = project(&[row(0, "user", "Earlier")], false, false);
        let rows = [
            row(1, "assistant", "Looking into it"),
            row(2, "thinking", "details"),
            row(3, "assistant", "Checking another part"),
            row(4, "thinking", "more"),
        ];
        let running = project(&rows, true, false);
        let mut done = rows.to_vec();
        done.push(row(5, "assistant", "Done"));
        let completed = project(&done, false, false);
        assert_ne!(completed[0].key(), running[0].key());
        let replacement = if overlap {
            completed.clone()
        } else {
            project(&[row(9, "user", "Replaced")], false, false)
        };
        let fake = Fake::new(move |_| ready(Ok(page(running.clone(), true, Some("latest")))));
        let view = view(&fake, cx);
        view.refresh(false, false).await;
        fake.pages
            .borrow_mut()
            .push_back(ready(Ok(page(prefix.clone(), true, Some("older")))));
        view.refresh(true, false).await;
        fake.pages
            .borrow_mut()
            .push_back(ready(Ok(page(replacement.clone(), true, Some("new")))));
        if !overlap {
            fake.pages
                .borrow_mut()
                .push_back(ready(Ok(page(Vec::new(), false, None))));
        }
        view.refresh(false, false).await;
        let snapshot = view.snapshot();
        let want = if overlap {
            [prefix, completed].concat()
        } else {
            replacement
        };
        assert_eq!(snapshot.items, want);
        assert_eq!(snapshot.next_cursor.as_deref(), overlap.then_some("older"));
        assert_eq!(snapshot.has_more, overlap);
    }
}

#[gpui::test]
async fn an_ack_over_an_in_flight_read_reads_again_and_keeps_expansion(cx: &mut TestAppContext) {
    for outcome in ["success", "failure", "blur", "unavailable"] {
        let items = project(&[row(1, "thinking", "detail")], true, false);
        let fake = Fake::fixed(items.clone());
        let view = view(&fake, cx);
        view.refresh(false, false).await;
        let key = items[0].key().to_owned();
        view.set_expanded(&key, true);
        let (send, reply) = gate();
        fake.pages.borrow_mut().push_back(reply);
        let old = view.refresh(false, false);
        let ack = view.refresh(false, true);
        let repeated = view.refresh(false, true);
        if outcome == "blur" {
            view.set_active(false);
        }
        let _ = send.send(match outcome {
            "failure" => Err("temporary timeout".into()),
            "unavailable" => Err("[UNSUPPORTED_CAPABILITY] oversized".into()),
            _ => Ok(page(items.clone(), false, None)),
        });
        join_all([old, ack, repeated]).await;
        let reads = fake.page_calls.borrow().len();
        assert_eq!(
            reads,
            if matches!(outcome, "blur" | "unavailable") {
                2
            } else {
                3
            },
            "{outcome}"
        );
        if matches!(outcome, "success" | "failure") {
            assert!(view.snapshot().expanded.contains(&key));
            assert_eq!(view.snapshot().error, None);
        }
    }
}

#[gpui::test]
async fn a_reset_or_reactivation_awaits_its_own_read(cx: &mut TestAppContext) {
    for transition in ["reset", "reset twice", "reactivate"] {
        for old_failure in [false, true] {
            let reads: Senders<Page<Row>> = Rc::default();
            let sends = reads.clone();
            let fake = Fake::new(move |_| {
                let (send, reply) = gate();
                sends.borrow_mut().push(send);
                reply
            });
            let current_items = project(&[row(1, "user", "current")], false, false);
            let view = view(&fake, cx);
            let old = view.refresh(false, false);
            match transition {
                "reactivate" => {
                    view.set_active(false);
                    view.set_active(true);
                }
                _ => view.reset(),
            }
            let intermediate = view.refresh(false, false);
            if transition == "reset twice" {
                view.reset();
            }
            let settled = Rc::new(Cell::new(false));
            let flag = settled.clone();
            let current = view.refresh(false, false);
            let watched = current.clone();
            cx.foreground_executor()
                .spawn(async move {
                    watched.await;
                    flag.set(true)
                })
                .detach();
            let repeated = view.refresh(false, false);
            let first = reads.borrow_mut().remove(0);
            let _ = first.send(if old_failure {
                Err("timeout".into())
            } else {
                Ok(page(Vec::new(), false, None))
            });
            old.await;
            cx.run_until_parked();
            assert_eq!(reads.borrow().len(), 1, "{transition}");
            assert!(!settled.get());
            let s = view.snapshot();
            assert!(!s.ready && s.error.is_none());
            let second = reads.borrow_mut().remove(0);
            let _ = second.send(Ok(page(current_items.clone(), false, None)));
            join_all([intermediate, current, repeated]).await;
            let s = view.snapshot();
            assert!(s.ready && s.error.is_none());
            assert_eq!(s.items, current_items);
            assert_eq!(fake.page_calls.borrow().len(), 2);
        }
    }
}

#[gpui::test]
async fn a_blur_cancels_a_queued_reset_read_and_a_later_failure_shows(cx: &mut TestAppContext) {
    let fake = Fake::fixed(Vec::new());
    let (send, reply) = gate();
    fake.pages.borrow_mut().push_back(reply);
    let view = view(&fake, cx);
    let old = view.refresh(false, false);
    view.reset();
    let queued = view.refresh(false, false);
    view.set_active(false);
    let _ = send.send(Ok(page(Vec::new(), false, None)));
    join_all([old, queued]).await;
    assert_eq!(fake.page_calls.borrow().len(), 1);
    fake.pages
        .borrow_mut()
        .push_back(ready(Err("timeout".into())));
    view.set_active(true);
    view.refresh(false, false).await;
    let s = view.snapshot();
    assert!(!s.ready && !s.loading);
    assert_eq!(s.error.as_deref(), Some("timeout"));
    assert_eq!(fake.page_calls.borrow().len(), 2);
}

#[gpui::test]
async fn an_opposite_direction_request_waits_for_the_pending_page(cx: &mut TestAppContext) {
    for older in [false, true] {
        let items = project(&[row(2, "user", "current")], false, false);
        let fake = Fake::new(move |_| ready(Ok(page(items.clone(), true, Some("2")))));
        let view = view(&fake, cx);
        view.refresh(false, false).await;
        let (send, reply) = gate();
        fake.pages.borrow_mut().push_back(reply);
        let current = view.refresh(older, false);
        let queued = [view.refresh(!older, false), view.refresh(!older, false)];
        assert_eq!(fake.page_calls.borrow().len(), 2);
        let items = project(&[row(2, "user", "current")], false, false);
        let _ = send.send(Ok(page(items, true, Some("2"))));
        let [a, b] = queued;
        join_all([current, a, b]).await;
        let two = Some("2".to_owned());
        let want = if older {
            [None, two.clone(), None]
        } else {
            [None, None, two]
        };
        assert_eq!(*fake.page_calls.borrow(), want);
    }
}

#[gpui::test]
async fn leaving_drops_a_queued_older_read(cx: &mut TestAppContext) {
    let fake = Fake::fixed(Vec::new());
    let (send, reply) = gate();
    fake.pages.borrow_mut().push_back(reply);
    let view = view(&fake, cx);
    let current = view.refresh(false, false);
    let older = view.refresh(true, false);
    view.set_active(false);
    let _ = send.send(Ok(page(Vec::new(), true, Some("2"))));
    join_all([current, older]).await;
    assert_eq!(fake.page_calls.borrow().len(), 1);
}

#[gpui::test]
async fn a_budget_failure_keeps_the_raw_fallback_until_reset(cx: &mut TestAppContext) {
    for stage in ["initial", "older", "refresh"] {
        let fail = Rc::new(Cell::new(stage == "initial"));
        let failing = fail.clone();
        let fake = Fake::new(move |_| {
            ready(match failing.get() {
                true => Err("[UNSUPPORTED_CAPABILITY] History view scan budget exceeded".into()),
                false => Ok(page(
                    project(&[row(1, "user", "hello")], false, false),
                    true,
                    Some("1"),
                )),
            })
        });
        let view = view(&fake, cx);
        view.refresh(false, false).await;
        if !fail.get() {
            fail.set(true);
            view.refresh(stage == "older", false).await;
        }
        let s = view.snapshot();
        assert!(!s.ready && s.items.is_empty(), "{stage}");
        assert!(unavailable(s.error.as_deref().unwrap()));
        let calls = fake.page_calls.borrow().len();
        fail.set(false);
        view.refresh(false, false).await;
        view.set_active(false);
        view.set_active(true);
        view.refresh(true, false).await;
        cx.run_until_parked();
        assert_eq!(fake.page_calls.borrow().len(), calls);
        view.reset();
        view.refresh(false, false).await;
        assert!(view.snapshot().ready);
    }
}

#[gpui::test]
async fn invalidations_coalesce_into_one_read_after_the_current_one(cx: &mut TestAppContext) {
    let fake = Fake::fixed(project(&[row(0, "user", "cached")], false, false));
    let view = view(&fake, cx);
    view.refresh(false, false).await;
    let (send, reply) = gate();
    fake.pages.borrow_mut().push_back(reply);
    let syncing = view.refresh(false, false);
    view.invalidate();
    view.invalidate();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(500));
    cx.run_until_parked();
    let edited = project(&[row(0, "user", "edited")], false, false);
    *fake.page_default.borrow_mut() = {
        let edited = edited.clone();
        Box::new(move |_| ready(Ok(page(edited.clone(), false, None))))
    };
    let _ = send.send(Ok(page(
        project(&[row(0, "user", "cached")], false, false),
        false,
        None,
    )));
    syncing.await;
    cx.run_until_parked();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(500));
    cx.run_until_parked();
    assert_eq!(fake.page_calls.borrow().len(), 3);
    assert_eq!(view.snapshot().items, edited);
}

#[gpui::test]
async fn pagination_feedback_shows_only_for_an_older_page(cx: &mut TestAppContext) {
    let fake = Fake::new(|_| ready(Ok(page(Vec::new(), true, Some("older")))));
    let view = view(&fake, cx);
    let feedback: Rc<RefCell<Vec<bool>>> = Rc::default();
    let (seen, watched) = (feedback.clone(), view.clone());
    let _stop = view.subscribe(move || seen.borrow_mut().push(watched.is_loading_older()));
    view.refresh(false, false).await;
    for _ in 0..3 {
        view.refresh(false, false).await;
    }
    assert!(feedback.borrow().iter().all(|l| !l));
    let (send, reply) = gate();
    fake.pages.borrow_mut().push_back(reply);
    let latest = view.refresh(false, false);
    let older = view.refresh(true, false);
    assert!(view.snapshot().loading);
    assert!(!view.is_loading_older());
    let _ = send.send(Ok(page(Vec::new(), true, Some("older"))));
    latest.await;
    older.await;
    let tail: Vec<bool> = feedback
        .borrow()
        .iter()
        .rev()
        .take(4)
        .rev()
        .copied()
        .collect();
    assert_eq!(tail, [false, false, true, false]);
    assert_eq!(
        fake.page_calls.borrow().last().unwrap().as_deref(),
        Some("older")
    );
}

#[gpui::test]
async fn pagination_feedback_clears_after_a_failure_blur_or_reset(cx: &mut TestAppContext) {
    for end in ["failure", "blur", "reset"] {
        let fake = Fake::new(|_| ready(Ok(page(Vec::new(), true, Some("older")))));
        let view = view(&fake, cx);
        view.refresh(false, false).await;
        let (send, reply) = gate();
        fake.pages.borrow_mut().push_back(reply);
        let older = view.refresh(true, false);
        assert!(view.is_loading_older());
        match end {
            "failure" => drop(send.send(Err("unavailable".into()))),
            _ => {
                if end == "blur" {
                    view.set_active(false);
                } else {
                    view.reset();
                }
                assert!(!view.is_loading_older(), "{end}");
                let _ = send.send(Ok(page(Vec::new(), true, Some("older"))));
            }
        }
        older.await;
        assert!(!view.is_loading_older(), "{end}");
    }
}
