//! `handleSessionClick`: a click on a task opens its page, and the active task is the route's.

use gpui::{App, ClickEvent, Window};

use super::{row::rename_on_double, select};
use crate::router;

/// `isActive`: the task `/cc-agent/:sessionId` shows.
pub(super) fn is_active(id: &str, cx: &App) -> bool {
    router::active_session(cx) == Some(id)
}

/// `resolveSessionRoute` for a listed task: its own page; a companion's chat comes with 14.0.
pub(super) fn route(id: &str) -> String {
    format!("/cc-agent/{id}")
}

/// A click opens the task, as `handleClick` passes it on, or selects it with a modifier; a
/// double click then renames. The press outside a rename field has committed it by then.
pub(super) fn clicked(id: &str, event: &ClickEvent, window: &mut Window, cx: &mut App) {
    let m = event.modifiers();
    if select::modified(&m) {
        return select::click(id, &m, cx);
    }
    open(id, cx);
    rename_on_double(id, event, window, cx);
}

/// `handleSessionClick` without modifiers: the selection clears; no navigation to the task
/// already shown.
pub(super) fn open(id: &str, cx: &mut App) {
    select::plain(id, cx);
    if is_active(id, cx) {
        return;
    }
    router::navigate(&route(id), cx);
}

#[cfg(test)]
mod tests;
