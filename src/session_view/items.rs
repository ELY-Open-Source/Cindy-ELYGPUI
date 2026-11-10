//! The routed task's render items: `buildMessageStreamRenderProjection` on its ready view.

use std::{cell::Cell, rc::Rc};

use gpui::App;
use indexmap::IndexSet;

use super::{SessionView, session};
use crate::{
    chat_message::merge::Message,
    chat_store::Store,
    history_render::{self, Input, Projection},
    history_view::controller::{View, settled},
};

/// The view the content follows, with what it last rendered.
pub(super) struct Watch {
    view: View<Message>,
    stop: Option<Box<dyn FnOnce()>>,
    items: Option<Result<Rc<Projection>, String>>,
}

impl Drop for Watch {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            stop();
        }
    }
}

/// `useSyncExternalStore` on the task's view: each change renders once the listener returns.
pub(super) fn watch(id: &str, cx: &mut App) -> Option<Watch> {
    let Some(view) = Store::global(cx).view(id) else {
        log::error!(target: "sessionView", "{id} has no history view to render");
        return None;
    };
    let scheduled = Rc::new(Cell::new(false));
    let (foreground, app) = (cx.foreground_executor().clone(), cx.to_async());
    let stop = view.subscribe(move || {
        if scheduled.replace(true) {
            return;
        }
        let (scheduled, app) = (scheduled.clone(), app.clone());
        let run = async move {
            // The listener runs inside the change; the render waits for it to return.
            settled(std::future::ready(())).await;
            scheduled.set(false);
            app.update(render);
        };
        foreground.spawn(run).detach();
    });
    Some(Watch {
        view,
        stop: Some(Box::new(stop)),
        items: None,
    })
}

fn watched(cx: &mut App) -> Option<&mut Watch> {
    let shown = cx.global_mut::<SessionView>().shown.as_mut()?;
    shown.content.as_mut()?.stream.as_mut()
}

/// The projection again, from the view's snapshot and the task's working directory.
pub(super) fn render(cx: &mut App) {
    let Some(view) = watched(cx).map(|watch| watch.view.clone()) else {
        return;
    };
    let snapshot = view.snapshot();
    let working_dir = session(cx).and_then(|s| s.working_dir.clone());
    // No row is live before 10.0's sends and streams.
    let none = IndexSet::new();
    let input = Input {
        snapshot: &snapshot,
        live: &[],
        streaming: false,
        pending: &none,
        tail: &none,
        working_dir: working_dir.as_deref().unwrap_or_default(),
    };
    let items = snapshot.ready.then(|| history_render::render(&input));
    match &items {
        Some(Ok(p)) => {
            let (items, rows) = (p.items.len(), p.rows.len());
            log::debug!(target: "sessionView", "rendered {items} items from {rows} rows");
        }
        Some(Err(error)) => log::error!(target: "sessionView", "render failed: {error}"),
        None => log::debug!(target: "sessionView", "the history view is not ready"),
    }
    if let Some(watch) = watched(cx) {
        watch.items = items.map(|items| items.map(Rc::new));
    }
    cx.refresh_windows();
}

/// The routed task's items, once its view is ready; `Err` is the projection's throw.
#[cfg_attr(not(test), expect(dead_code, reason = "the rows of 9.5 draw them"))]
pub fn items(cx: &App) -> Option<&Result<Rc<Projection>, String>> {
    let shown = cx.global::<SessionView>().shown.as_ref()?;
    shown.content.as_ref()?.stream.as_ref()?.items.as_ref()
}
