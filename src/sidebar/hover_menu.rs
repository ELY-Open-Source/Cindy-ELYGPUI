//! `useHoverOpenMenu`: open on hover, close after leaving; a click only opens.

use std::time::Duration;

use gpui::{App, FocusHandle, Task, Window};

const OPEN_DELAY: Duration = Duration::from_millis(90);
const CLOSE_DELAY: Duration = Duration::from_millis(160);

#[derive(Default)]
pub struct HoverMenu {
    open: bool,
    /// `openedViaHover`: such a menu takes no focus and returns none.
    via_hover: bool,
    /// The trigger a close hands focus back to; none after a hover open or a press outside.
    back: Option<FocusHandle>,
    pending: Option<(Pending, Task<()>)>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Pending {
    Open,
    Close,
}

/// Finds the menu in the app's state.
pub type Locate = fn(&mut App) -> &mut HoverMenu;

impl HoverMenu {
    pub fn is_open(&self) -> bool {
        self.open
    }

    fn cancel(&mut self, kind: Pending) {
        if self.pending.as_ref().is_some_and(|(k, _)| *k == kind) {
            self.pending = None;
        }
    }
}

/// `setOpen`; a close runs `onCloseAutoFocus`, which returns focus unless the open was a hover.
fn set(locate: Locate, open: bool, window: &mut Window, cx: &mut App) {
    let menu = locate(cx);
    if menu.open == open {
        return;
    }
    menu.open = open;
    let back = match open {
        true => None,
        false => menu.back.take(),
    };
    log::info!(target: "sidebar", "hover menu {}, focus back {}", if open { "opened" } else { "closed" }, back.is_some());
    if let Some(trigger) = back {
        window.focus(&trigger, cx);
    }
    cx.refresh_windows();
}

fn schedule(locate: Locate, kind: Pending, window: &mut Window, cx: &mut App) {
    let delay = match kind {
        Pending::Open => OPEN_DELAY,
        Pending::Close => CLOSE_DELAY,
    };
    let task = window.spawn(cx, async move |cx| {
        cx.background_executor().timer(delay).await;
        let done = cx.update(|window, cx| {
            let menu = locate(cx);
            menu.pending = None;
            if kind == Pending::Open {
                (menu.via_hover, menu.back) = (true, None);
            }
            set(locate, kind == Pending::Open, window, cx);
        });
        if let Err(error) = done {
            log::error!(target: "sidebar", "hover menu {kind:?}: window gone: {error:#}");
        }
    });
    locate(cx).pending = Some((kind, task));
}

/// `scheduleOpen`: the pointer entered the trigger.
pub fn enter_trigger(locate: Locate, window: &mut Window, cx: &mut App) {
    let menu = locate(cx);
    menu.cancel(Pending::Close);
    if menu.open || menu.pending.is_some() {
        return;
    }
    schedule(locate, Pending::Open, window, cx);
}

/// `scheduleClose`: the pointer left the trigger or the content.
pub fn leave(locate: Locate, window: &mut Window, cx: &mut App) {
    let menu = locate(cx);
    menu.cancel(Pending::Open);
    if !menu.open || menu.pending.is_some() {
        return;
    }
    schedule(locate, Pending::Close, window, cx);
}

/// `cancelClose`: the pointer reached the content.
pub fn enter_content(locate: Locate, cx: &mut App) {
    locate(cx).cancel(Pending::Close);
}

/// `onOpenChange(true)` from a click or a key; a later close focuses `trigger`.
pub fn open(locate: Locate, trigger: FocusHandle, window: &mut Window, cx: &mut App) {
    let menu = locate(cx);
    (menu.pending, menu.via_hover, menu.back) = (None, false, Some(trigger));
    set(locate, true, window, cx);
}

/// `onOpenChange(false)` from Escape or a choice; a press outside returns no focus.
pub fn close(locate: Locate, outside: bool, window: &mut Window, cx: &mut App) {
    let menu = locate(cx);
    menu.pending = None;
    if outside {
        menu.back = None;
    }
    set(locate, false, window, cx);
}

#[cfg(test)]
mod tests;
