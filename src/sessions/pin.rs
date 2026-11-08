//! `handleTogglePin`: `pinnedAt` on the row and in the database, and a pin promoted to the head
//! of `pinnedOrder` as `useSidebarFilter`'s `promotePin` and `enqueuePinnedPersist` do.

use gpui::{App, AppContext as _, Task, Window};

use super::{Sessions, paths::PLATFORM, settings, write};
use crate::{i18n::Translator, toast};

/// `durablePinnedOrderRef` and the write queue: saves run one after another.
#[derive(Default)]
pub struct Writes {
    durable: Vec<String>,
    pending: usize,
    /// The last save; the next one awaits it. Dropping it would cancel it.
    queue: Option<Task<()>>,
}

impl Writes {
    /// A read while saves are pending moves the baseline and keeps the shown order.
    pub fn read(&mut self, read: &mut Vec<String>, shown: &[String]) {
        if self.pending > 0 {
            self.durable = std::mem::replace(read, shown.to_vec());
        }
    }
}

/// Pins or unpins; an unpin leaves `pinnedOrder` alone, as Cindy's does.
pub fn toggle(id: &str, pinned: bool, window: &Window, cx: &mut App) {
    if !pinned {
        promote(id, window, cx);
    }
    let at = (!pinned).then(|| jiff::Timestamp::now().as_millisecond());
    write::apply(id, write::Meta::Pinned(at), window, cx);
}

fn pinned_order(cx: &mut App) -> Option<&mut Vec<String>> {
    let loaded = cx.global_mut::<Sessions>().loaded.as_mut()?;
    Some(&mut loaded.settings.pinned_order)
}

/// `updatePinnedOrder`: the order changes here first, then the save queues behind the others.
fn promote(id: &str, window: &Window, cx: &mut App) {
    let Some(order) = pinned_order(cx) else {
        log::warn!(target: "sessions", "pin {id}: no list loaded");
        return;
    };
    let current = order.clone();
    let desired = settings::promoted(&current, id, PLATFORM);
    if desired == current {
        return;
    }
    *order = desired.clone();
    let store = cx.global_mut::<Sessions>();
    if store.pins.pending == 0 {
        store.pins.durable = current;
    }
    store.pins.pending += 1;
    let previous = store.pins.queue.take();
    let owner_dir = store.owner_dir.clone();
    cx.refresh_windows();
    let handle = window.window_handle();
    let entry = id.to_owned();
    let task = cx.spawn(async move |cx| {
        if let Some(previous) = previous {
            previous.await;
        }
        let saving = cx.background_spawn({
            let entry = entry.clone();
            async move {
                let Some(owner_dir) = owner_dir else {
                    anyhow::bail!("sidebar settings require an active data owner");
                };
                settings::promote(&owner_dir, &entry, PLATFORM)
            }
        });
        let result = saving.await;
        cx.update(|cx| settled(&entry, &desired, result, handle, cx));
    });
    cx.global_mut::<Sessions>().pins.queue = Some(task);
}

/// A save's end: a failure rolls back to the durable order while its order still shows; the
/// last save of a run shows what was saved.
fn settled(
    id: &str,
    desired: &[String],
    result: anyhow::Result<Vec<String>>,
    handle: gpui::AnyWindowHandle,
    cx: &mut App,
) {
    let store = cx.global_mut::<Sessions>();
    store.pins.pending -= 1;
    let (succeeded, error) = match result {
        Ok(saved) => {
            store.pins.durable = saved;
            (true, None)
        }
        Err(error) => (false, Some(error)),
    };
    let durable = store.pins.durable.clone();
    let drained = store.pins.pending == 0;
    if let Some(order) = pinned_order(cx)
        && ((succeeded && drained) || (!succeeded && *order == desired))
    {
        *order = durable;
    }
    if succeeded {
        super::watch_owner(cx);
    }
    cx.refresh_windows();
    let Some(error) = error else {
        return;
    };
    log::warn!(target: "sessions", "failed to persist pinned session order for {id}: {error:#}");
    let shown = handle.update(cx, |_, window, cx| {
        let message = cx
            .global::<Translator>()
            .t("ccAgent.sidebar.pinFailed", &[]);
        toast::error(window, message, toast::Options::default(), cx);
    });
    if shown.is_err() {
        log::warn!(target: "sessions", "pinFailed: its window closed before the toast");
    }
}
