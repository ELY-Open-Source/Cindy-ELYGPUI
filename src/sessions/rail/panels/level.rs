//! The project level: `openProject`, `scheduleProjectClose` and the panel of a project's tasks.

use gpui::{
    AnyElement, App, Bounds, FocusHandle, HitboxBehavior, IntoElement, ParentElement as _, Pixels,
    Point, Styled as _, Window, div,
};

use super::{Shell, entry_list, head, on_level, scroll};
use crate::{
    preferences::Preferences,
    sessions::{
        fold::{self, ShowAll},
        main_list::{SESSION_LIMIT_KEY, lists, text},
        rail::sections::{self, CLOSE_GRACE, OpenProject, Panels, Section, panels},
    },
};

/// `openProject`: opens or switches the level at the row's bounds; Projects must be open.
pub(super) fn open(key: &str, opener: Option<FocusHandle>, cx: &mut App) {
    let anchor = cx
        .try_global::<Shell>()
        .and_then(|s| s.rows.iter().find(|(k, ..)| k == key))
        .map(|(_, bounds, _)| *bounds);
    sections::cancel_close(cx);
    let p = panels(cx);
    p.project_close = None;
    if p.open != Some(Section::Projects) {
        return;
    }
    let Some(anchor) = anchor else {
        log::warn!(target: "sidebar", "rail project {key}: row not laid out");
        return;
    };
    let via_keyboard = opener.is_some();
    let same = p.project.as_ref().filter(|o| o.key == key);
    let show_all = same.is_some_and(|o| o.show_all);
    let focused_in = same.is_some_and(|o| o.via_keyboard == via_keyboard && o.focused_in);
    if same.is_none() || same.is_some_and(|o| o.via_keyboard != via_keyboard) {
        log::info!(target: "sidebar", "rail project {key} opened, keyboard {via_keyboard}");
    }
    p.project = Some(OpenProject {
        key: key.to_owned(),
        anchor,
        via_keyboard,
        focused_in,
        show_all,
    });
    if via_keyboard {
        p.project_opener = opener;
    }
    cx.refresh_windows();
}

/// `scheduleProjectClose`: no grace closes a level either level's keyboard open made.
fn schedule_close(window: &mut Window, cx: &mut App) {
    let p = panels(cx);
    let keyboard = p.via_keyboard || p.project.as_ref().is_some_and(|o| o.via_keyboard);
    if keyboard || super::suppressed(window, cx) {
        return;
    }
    let task = window.spawn(cx, async move |cx| {
        cx.background_executor().timer(CLOSE_GRACE).await;
        let done = cx.update(|window, cx| {
            panels(cx).project_close = None;
            if !super::suppressed(window, cx) {
                close(window, cx);
            }
        });
        if let Err(error) = done {
            log::error!(target: "sidebar", "rail project close: window gone: {error:#}");
        }
    });
    panels(cx).project_close = Some(task);
}

/// The level closes alone; focus goes back to the row a keyboard open came from.
fn close(window: &mut Window, cx: &mut App) {
    let p = panels(cx);
    let Some(open) = p.project.take() else {
        return;
    };
    let back = p.project_opener.take();
    log::info!(target: "sidebar", "rail project {} closed, focus back {}", open.key, back.is_some());
    if let Some(back) = back {
        window.focus(&back, cx);
    }
    cx.refresh_windows();
}

/// `RAIL_PROJECT_KEEPALIVE_SELECTOR` or a project row holds the level; elsewhere its grace runs.
pub(super) fn pointer(at: Point<Pixels>, window: &mut Window, cx: &mut App) {
    if panels(cx).project.is_none() {
        return;
    }
    let on_row = cx
        .try_global::<Shell>()
        .is_some_and(|s| s.rows.iter().any(|(.., hit)| hit.is_hovered_at(at, window)));
    if on_row || on_level(1, at, window, cx) {
        panels(cx).project_close = None;
    } else if panels(cx).project_close.is_none() {
        schedule_close(window, cx);
    }
}

/// A project row's prepaint: its bounds anchor the level, its hitbox keeps it.
pub(super) fn record(key: String, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
    let hit = window.insert_hitbox(bounds, HitboxBehavior::Normal);
    cx.default_global::<Shell>().rows.push((key, bounds, hit));
}

/// The open project's tasks under its head; the head's New comes with 8.10.
pub(super) fn body(cx: &mut App) -> Option<(Bounds<Pixels>, AnyElement)> {
    let p = cx.global::<Panels>();
    let open = p
        .project
        .as_ref()
        .filter(|_| p.open == Some(Section::Projects))?;
    let (key, anchor, expanded) = (open.key.clone(), open.anchor, open.show_all);
    let project = lists(cx)?
        .rail_projects
        .into_iter()
        .find(|p| p.key == key)?;
    let limit = fold::limit(text(cx.global::<Preferences>(), SESSION_LIMIT_KEY), 5);
    let now = jiff::Timestamp::now().as_millisecond();
    let since = Some(fold::recent_since(now));
    let sessions = &project.sessions;
    let (visible, hidden) = fold::view(sessions, limit, expanded, since, |s| s.activity_at);
    let more = ShowAll::new("rail-project-show-all".into(), sessions.len(), |cx| {
        if let Some(open) = panels(cx).project.as_mut() {
            open.show_all = true;
        }
    });
    let rows = entry_list(&visible, hidden, more, now, cx);
    let body = div()
        .flex()
        .flex_col()
        .child(head(project.name.clone(), sessions.len(), cx))
        .child(scroll(rows))
        .into_any_element();
    Some((anchor, body))
}

#[cfg(test)]
mod tests;
