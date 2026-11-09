//! `ConfirmDialogProvider`'s `confirm`: one queue per window, one dialog at a time.

use std::{
    collections::{HashMap, VecDeque},
    time::Duration,
};

use futures::channel::oneshot;
use gpui::{AnyElement, App, Global, IntoElement as _, SharedString, Window, WindowId};

use crate::{confirm_dialog::ConfirmDialog, i18n::Translator};

/// The next dialog waits for the last one's exit.
const GAP: Duration = Duration::from_millis(200);

/// `ConfirmOptions` as far as its callers use it.
pub struct Ask {
    pub title: String,
    pub description: Option<String>,
    pub confirm_text: Option<String>,
    pub auto_focus_confirm: bool,
}

struct Item {
    ask: Ask,
    reply: Option<oneshot::Sender<bool>>,
}

#[derive(Default)]
struct Board {
    queue: VecDeque<Item>,
    current: Option<Item>,
    open: bool,
}

#[derive(Default)]
pub struct Confirms(HashMap<WindowId, Board>);

impl Global for Confirms {}

fn board(window: WindowId, cx: &mut App) -> &mut Board {
    cx.default_global::<Confirms>().0.entry(window).or_default()
}

/// Settles once: confirm, or cancel on Escape, Cancel or a closed window.
fn settle(window: WindowId, confirmed: bool, cx: &mut App) {
    let reply = board(window, cx)
        .current
        .as_mut()
        .and_then(|i| i.reply.take());
    if let Some(reply) = reply {
        log::info!(target: "confirm-dialog", "settled: {}", if confirmed { "confirm" } else { "cancel" });
        if reply.send(confirmed).is_err() {
            log::warn!(target: "confirm-dialog", "its asker is gone");
        }
    }
}

fn next(window: WindowId, cx: &mut App) {
    let b = board(window, cx);
    if b.current.is_some() {
        return;
    }
    b.current = b.queue.pop_front();
    b.open = b.current.is_some();
}

/// `confirm`: true once the user confirms.
pub fn ask(window: &Window, ask: Ask, cx: &mut App) -> oneshot::Receiver<bool> {
    let (reply, answer) = oneshot::channel();
    let id = window.window_handle().window_id();
    board(id, cx).queue.push_back(Item {
        ask,
        reply: Some(reply),
    });
    next(id, cx);
    cx.refresh_windows();
    answer
}

/// The dialog for the window's current ask, kept through its exit.
pub fn view(window: &Window, cx: &mut App) -> Option<AnyElement> {
    let id = window.window_handle().window_id();
    let b = board(id, cx);
    let open = b.open;
    let item = b.current.as_ref()?;
    let (title, confirm) = (item.ask.title.clone(), item.ask.confirm_text.clone());
    let (description, focus_confirm) = (item.ask.description.clone(), item.ask.auto_focus_confirm);
    let t = cx.global::<Translator>();
    let strings = (
        SharedString::from(title),
        SharedString::from(confirm.unwrap_or_else(|| t.t("commonUi.confirmDialog.confirm", &[]))),
        SharedString::from(t.t("commonUi.confirmDialog.cancel", &[])),
    );
    let dialog = ConfirmDialog::new(
        "confirm-dialog-provider",
        open,
        strings,
        move |window, cx| {
            let id = window.window_handle().window_id();
            settle(id, false, cx);
            board(id, cx).open = false;
            let handle = window.window_handle();
            cx.spawn(async move |cx| {
                cx.background_executor().timer(GAP).await;
                cx.update(|cx| {
                    board(id, cx).current = None;
                    next(id, cx);
                    if handle.update(cx, |_, window, _| window.refresh()).is_err() {
                        log::warn!(target: "confirm-dialog", "its window closed");
                    }
                });
            })
            .detach();
        },
        move |window, cx| settle(window.window_handle().window_id(), true, cx),
    );
    let dialog = match description {
        Some(text) => dialog.description(text),
        None => dialog,
    };
    let dialog = if focus_confirm {
        dialog.auto_focus_confirm()
    } else {
        dialog
    };
    Some(dialog.into_any_element())
}
