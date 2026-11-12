//! `CCAgentSessionView` for a local task: its gate, its read and its visit.

use gpui::{App, Global};

use crate::{
    chat_store::Store,
    localdb::{LocalDb, State},
    router,
    sessions::{self, group::Session, query},
};

mod assistant_row;
mod attachments;
mod chip;
mod column;
mod entry_error;
mod flow;
mod handoff;
mod handoff_pill;
pub mod header;
mod hook_card;
mod images;
mod inline_image;
mod items;
mod jump;
mod link_chip;
mod links;
mod markdown;
mod orca_card;
mod source_labels;
mod stream;
mod streamed;
mod target_link;
mod user_row;
mod width;

pub use entry_error::element;
pub use images::focused as images_focused;

/// `sessionEntryFailure` for a local read, which is never `denied`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Failure {
    Missing,
    Failed,
}

/// `CindyMakeSessionGate`'s state for the routed task.
struct Shown {
    id: String,
    /// `knownSource` when the gate last looked.
    known: Option<String>,
    /// `loaded`: the source the gate's own read found.
    loaded: Option<String>,
    failure: Option<Failure>,
    /// The gate's latest read; a new one cancels the last.
    gate_read: u64,
    /// The content, mounted while the gate is open.
    content: Option<Content>,
}

/// `CCAgentSessionContent`: its `serverSession` and `createSessionRefreshSequence`'s latest.
struct Content {
    found: Option<Session>,
    read: u64,
    /// The task's view and its render items.
    stream: Option<items::Watch>,
    /// What is unfolded: a long message by its item key, a card's context by `<key>-context`.
    unfolded: std::collections::HashSet<String>,
    /// What its link chips read.
    links: link_chip::Found,
    /// Its stream's scroller, and where an in-document anchor lands.
    jump: jump::Jump,
    /// Its messages' images, by address.
    images: images::Held,
    /// Its assistant messages' trees, by item key.
    trees: assistant_row::Trees,
    /// Its streaming messages' renderers, by item key.
    streams: streamed::Streams,
    /// `handoffPillDismissed`.
    dismissed: bool,
}

#[derive(Default)]
pub struct SessionView {
    shown: Option<Shown>,
    reads: u64,
}

impl Global for SessionView {}

pub fn install(cx: &mut App) {
    streamed::install(cx);
    cx.set_global(SessionView::default());
    cx.observe_global::<router::Router>(sync).detach();
    cx.observe_global::<sessions::Sessions>(sync).detach();
    sync(cx);
}

/// `isCindyMakeSource`, with `CINDY_MAKE_ENABLED` false.
pub fn cindy_make(source: &str) -> bool {
    source == "cindy-make" || source == "cindy-make-merge"
}

/// The task the content shows: its read row, else the `active` bucket's.
pub fn session(cx: &App) -> Option<&Session> {
    let shown = cx.global::<SessionView>().shown.as_ref()?;
    let content = shown.content.as_ref()?;
    let listed = || sessions::find(&shown.id, cx).filter(|s| s.status == "active");
    content.found.as_ref().or_else(listed)
}

/// What the gate draws for the route's task.
#[derive(Debug, PartialEq)]
pub enum Gate {
    /// Nothing yet: the read is in flight.
    Waiting,
    Failed(Failure),
    Open,
}

pub fn gate(cx: &App) -> Option<Gate> {
    let shown = cx.global::<SessionView>().shown.as_ref()?;
    Some(match (&shown.content, shown.failure) {
        (Some(_), _) => Gate::Open,
        (None, Some(failure)) => Gate::Failed(failure),
        (None, None) => Gate::Waiting,
    })
}

fn next_read(cx: &mut App) -> u64 {
    let view = cx.global_mut::<SessionView>();
    view.reads += 1;
    view.reads
}

/// Follows the route and the list: the gate first, then the content it mounts.
fn sync(cx: &mut App) {
    let id = router::active_session(cx);
    let shown_id = cx
        .global::<SessionView>()
        .shown
        .as_ref()
        .map(|s| s.id.clone());
    if shown_id != id {
        unmount(cx);
        cx.global_mut::<SessionView>().shown = None;
        let Some(id) = id else { return };
        log::info!(target: "sessionView", "opened {id}");
        let known = known(&id, cx);
        cx.global_mut::<SessionView>().shown = Some(Shown {
            id,
            known: known.clone(),
            loaded: None,
            failure: None,
            gate_read: 0,
            content: None,
        });
        if known.is_none() {
            gate_read(cx);
        }
    } else if let Some(id) = id {
        let known = known(&id, cx);
        let shown = cx
            .global_mut::<SessionView>()
            .shown
            .as_mut()
            .expect("shown");
        if shown.known != known {
            log::info!(target: "sessionView", "{id}'s listed source is now {known:?}");
            shown.known = known.clone();
            // The effect's cleanup cancels the read in flight.
            shown.gate_read = 0;
            if known.is_none() {
                gate_read(cx);
            }
        }
    }
    settle(cx);
}

/// `knownSource`: the source any held bucket lists.
fn known(id: &str, cx: &App) -> Option<String> {
    sessions::find(id, cx).map(|s| s.source.clone())
}

/// The gate's redirect or body; an open gate mounts the content, a closed one unmounts it.
fn settle(cx: &mut App) {
    let Some(shown) = cx.global::<SessionView>().shown.as_ref() else {
        return;
    };
    let id = shown.id.clone();
    let source = shown.known.clone().or_else(|| shown.loaded.clone());
    if source.as_deref().is_some_and(cindy_make) {
        log::info!(target: "sessionView", "{id} is a Cindy Make task, so the route goes back");
        return router::navigate("/cc-agent", cx);
    }
    match (source.is_some(), shown.content.is_some()) {
        (true, false) => mount(&id, cx),
        (false, true) => unmount(cx),
        _ => {}
    }
}

/// The content's effects: its read, then the visit; its `active` row is never deleted.
fn mount(id: &str, cx: &mut App) {
    log::info!(target: "sessionView", "{id}'s content mounted");
    let shown = cx
        .global_mut::<SessionView>()
        .shown
        .as_mut()
        .expect("shown");
    shown.content = Some(Content {
        found: None,
        read: 0,
        stream: None,
        unfolded: Default::default(),
        links: Default::default(),
        jump: Default::default(),
        images: Default::default(),
        trees: Default::default(),
        streams: Default::default(),
        dismissed: false,
    });
    content_read(cx);
    let store = Store::global(cx);
    store.enter_view(id, cx);
    store.ensure_initial_messages(id, cx);
    let stream = items::watch(id, cx);
    let shown = cx.global_mut::<SessionView>().shown.as_mut();
    shown
        .and_then(|s| s.content.as_mut())
        .expect("mounted")
        .stream = stream;
    items::render(cx);
}

/// The content unmounts: `enterView`'s dispose runs and its reads stop counting.
fn unmount(cx: &mut App) {
    let Some(shown) = cx.global_mut::<SessionView>().shown.as_mut() else {
        return;
    };
    if shown.content.take().is_some() {
        let id = shown.id.clone();
        log::info!(target: "sessionView", "left {id}");
        Store::global(cx).leave_view(&id);
    }
}

/// The deleted effect: the task is purged and the route goes back.
fn leave_deleted(id: &str, cx: &mut App) {
    if session(cx).is_some_and(|s| s.status == "deleted") {
        log::info!(target: "sessionView", "{id} was deleted, so the route goes back");
        Store::global(cx).purge(id);
        router::navigate("/cc-agent", cx);
    }
}

/// The database changed: the content reads its task again, as a patch would reach it.
pub fn reread(cx: &mut App) {
    let mounted = cx.global::<SessionView>().shown.as_ref();
    if mounted.is_some_and(|s| s.content.is_some()) {
        content_read(cx);
    }
}

/// The gate's Retry: the failure clears while the gate reads again.
pub fn retry(cx: &mut App) {
    if let Some(shown) = cx.global::<SessionView>().shown.as_ref() {
        log::info!(target: "sessionView", "retry {}", shown.id);
        if shown.known.is_none() {
            gate_read(cx);
        }
    }
    cx.refresh_windows();
}

/// The gate's `getSessionFor`: its failure clears while it runs.
fn gate_read(cx: &mut App) {
    let seq = next_read(cx);
    let shown = cx
        .global_mut::<SessionView>()
        .shown
        .as_mut()
        .expect("shown");
    shown.failure = None;
    shown.gate_read = seq;
    let id = shown.id.clone();
    get(id, cx, move |row, cx| {
        let Some(shown) = cx.global_mut::<SessionView>().shown.as_mut() else {
            return;
        };
        if shown.gate_read != seq {
            log::debug!(target: "sessionView", "gate read {seq} of {} dropped", shown.id);
            return;
        }
        match row {
            Ok(Some(session)) => shown.loaded = Some(session.source),
            Ok(None) => shown.failure = Some(Failure::Missing),
            Err(_) => shown.failure = Some(Failure::Failed),
        }
        settle(cx);
    });
}

/// `sessionService.get` for the content: the latest read lands, a failure keeps the row.
fn content_read(cx: &mut App) {
    let seq = next_read(cx);
    let shown = cx
        .global_mut::<SessionView>()
        .shown
        .as_mut()
        .expect("shown");
    shown.content.as_mut().expect("mounted").read = seq;
    let id = shown.id.clone();
    get(id.clone(), cx, move |row, cx| {
        let shown = cx.global_mut::<SessionView>().shown.as_mut();
        let content = shown.and_then(|s| s.content.as_mut());
        let Some(content) = content.filter(|c| c.read == seq) else {
            log::debug!(target: "sessionView", "content read {seq} of {id} dropped");
            return;
        };
        if let Ok(Some(session)) = row {
            content.found = Some(session);
            leave_deleted(&id, cx);
            items::render(cx);
        }
    });
}

/// `local-db:sessions:get` on the database worker; `done` runs on the main thread.
fn get(
    id: String,
    cx: &mut App,
    done: impl FnOnce(Result<Option<Session>, String>, &mut App) + 'static,
) {
    let State::Ready(db) = &cx.global::<LocalDb>().state else {
        log::error!(target: "sessionView", "read {id} failed: DbClient not ready");
        return done(Err("DbClient not ready".into()), cx);
    };
    let ids = vec![id.clone()];
    let answer = db.call(move |conn| query::by_ids(conn, &ids).map_err(|e| e.to_string()));
    cx.spawn(async move |cx| {
        let row = match answer.await {
            Ok(row) => row.map(|rows| rows.into_iter().next()),
            Err(_) => Err("the database closed during the read".into()),
        };
        match &row {
            Ok(Some(s)) => log::info!(target: "sessionView", "read {id}: {}", s.status),
            Ok(None) => log::warn!(target: "sessionView", "read {id}: not found"),
            Err(error) => log::error!(target: "sessionView", "read {id} failed: {error}"),
        }
        cx.update(|cx| {
            done(row, cx);
            cx.refresh_windows();
        });
    })
    .detach();
}

#[cfg(test)]
mod tests;
