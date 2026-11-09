//! `handleSessionClick`: a click on a task opens its page, and the active task is the route's.

use gpui::{App, ClickEvent, Window};

use super::row::rename_on_double;
use crate::router;

/// `isActive`: the task `/cc-agent/:sessionId` shows.
pub(super) fn is_active(id: &str, cx: &App) -> bool {
    router::active_session(cx) == Some(id)
}

/// `resolveSessionRoute` for a listed task: its own page; a companion's chat comes with 14.0.
fn route(id: &str) -> String {
    format!("/cc-agent/{id}")
}

/// A click opens the task, as `handleClick` passes it on; a double click then renames. A
/// rename has ended by then: the press outside its field commits it.
pub(super) fn clicked(id: &str, event: &ClickEvent, window: &mut Window, cx: &mut App) {
    let m = event.modifiers();
    if m.shift || m.platform || m.control {
        log::debug!(target: "sessions", "selection click on {id}: bulk selection comes with 8.6d");
        return;
    }
    open(id, cx);
    rename_on_double(id, event, window, cx);
}

/// `handleSessionClick` without modifiers: no navigation to the task already shown.
pub(super) fn open(id: &str, cx: &mut App) {
    if is_active(id, cx) {
        return;
    }
    router::navigate(&route(id), cx);
}

#[cfg(test)]
mod tests;
