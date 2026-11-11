//! What a link chip does: `useSessionLinkTarget`, `ProjectLinkChip`'s focus, keys and menu.

use std::{
    cell::Cell,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

use ely_gpui_component::{
    menus::{Menu, MenuItem},
    primitives::IconName,
};
use gpui::{
    App, Bounds, ClickEvent, Div, ElementId, InteractiveElement as _, Modifiers, MouseButton,
    ParentElement as _, Pixels, Role, SharedString, Stateful, StatefulInteractiveElement as _,
    Styled as _, Window, canvas, point,
};
use rusqlite::{Connection, OptionalExtension as _};

use super::{Act, held_mut};
use crate::{
    deep_link,
    i18n::Translator,
    link_open,
    localdb::{LocalDb, State},
    router,
    session_view::{
        cindy_make,
        links::{self, Shared, What},
    },
    sessions::{self, group::Session, query},
};

/// The opens asked so far, over every mounted task.
static ASKED: AtomicU64 = AtomicU64::new(0);

/// `getTeamByWorkerSession`'s lead.
fn lead(db: &Connection, worker: &str) -> rusqlite::Result<Option<String>> {
    let sql = "SELECT orca_teams.lead_session_id FROM orca_workers
               INNER JOIN orca_teams ON orca_teams.id = orca_workers.team_id
               WHERE orca_workers.session_id = ? LIMIT 1";
    db.query_row(sql, [worker], |row| row.get(0)).optional()
}

/// `resolveSessionRoute`; `?worker=` comes with Orca's chapter, a companion's chat with 14.0.
fn resolve(id: &str, task: Option<&Session>, lead: impl FnOnce() -> Option<String>) -> String {
    match task {
        Some(task) if cindy_make(&task.source) => "/cc-agent".to_owned(),
        Some(task) if task.orca_worker => match lead() {
            Some(lead) => format!("/cc-agent/{lead}"),
            None => "/cc-agent".to_owned(),
        },
        _ => format!("/cc-agent/{id}"),
    }
}

/// The route read on the database worker; a failed read is no task, as Cindy's `catch` has it.
fn route(db: &Connection, id: &str) -> String {
    let task = match query::by_ids(db, &[id.to_owned()]) {
        Ok(rows) => rows.into_iter().next(),
        Err(error) => {
            log::warn!(target: "sessionView", "link target {id} not read: {error}");
            None
        }
    };
    resolve(id, task.as_ref(), || match lead(db, id) {
        Ok(lead) => lead,
        Err(error) => {
            log::warn!(target: "sessionView", "worker {id}'s lead not read: {error}");
            None
        }
    })
}

/// `openTarget` for the chip `chip`: its latest ask alone navigates, while it stays mounted.
pub fn open(href: &str, chip: &str, separate: bool, window: &Window, cx: &mut App) {
    if let Some(target) = deep_link::parse_session(href) {
        task(target.session_id, chip, separate, window, cx);
    }
}

/// `open` for the task `id`, which a handoff card names itself.
pub fn task(id: String, chip: &str, separate: bool, window: &Window, cx: &mut App) {
    let version = ASKED.fetch_add(1, Ordering::Relaxed) + 1;
    let Some(found) = held_mut(cx) else {
        return;
    };
    found.opening.insert(chip.to_owned(), version);
    log::info!(target: "sessionView", "session link to {id} activated, apart: {separate}");
    if separate {
        // `openSessionInNewWindow` comes with the task windows' chapter: it fails as a throw.
        log::error!(target: "sessionView", "no new window opens task {id} before its chapter");
        return links::failed("chat.markdownRenderer.openLinkFailed", window, cx);
    }
    let chip = chip.to_owned();
    let latest = move |cx: &mut App| {
        held_mut(cx).is_some_and(|found| found.opening.get(&chip) == Some(&version))
    };
    let State::Ready(db) = &cx.global::<LocalDb>().state else {
        log::warn!(target: "sessionView", "link target {id} not read: DbClient not ready");
        return router::navigate(&format!("/cc-agent/{id}"), cx);
    };
    let task = id.clone();
    let answer = db.call(move |conn| route(conn, &task));
    cx.spawn(async move |cx| {
        let Ok(route) = answer.await else {
            return log::error!(target: "sessionView", "link target {id}: database closed");
        };
        cx.update(|cx| {
            if !latest(cx) {
                return log::info!(target: "sessionView", "link target {id}: a later ask won");
            }
            router::navigate(&route, cx);
            cx.refresh_windows();
        });
    })
    .detach();
}

/// `ProjectLinkChip`'s `handleClick`; a message is drawn only where the sidebar's list is.
pub fn locate(href: &str, window: &Window, cx: &mut App) {
    let Some(dir) = deep_link::parse_project(href) else {
        return;
    };
    log::info!(target: "sessionView", "project link activated");
    sessions::focus::request(&dir, window, cx);
}

/// A click or a key on the chip `id`, with the modifiers and button it held.
pub fn activate(
    (act, id): (&Act, &str),
    (modifiers, button): (Modifiers, MouseButton),
    window: &Window,
    cx: &mut App,
) {
    match act {
        Act::Session(href) => {
            let separate = link_open::separate(modifiers, button);
            open(href, id, separate, window, cx)
        }
        Act::Project(href) => locate(href, window, cx),
    }
}

/// `useLinkContextMenu`'s actions for the chip `id`; the host adds Copy link.
pub fn menu(act: &Act, id: &SharedString, cx: &App) -> Menu {
    let t = cx.global::<Translator>();
    let label = |key: &str| t.t(key, &[]);
    match act {
        Act::Session(href) => {
            let (here, apart) = (href.clone(), href.clone());
            let (chip, other) = (id.clone(), id.clone());
            let current = MenuItem::new(label("chat.markdownRenderer.openInCurrentWindow"))
                .icon(IconName::CornerDownRight)
                .on_click(move |window, cx| open(&here, &chip, false, window, cx));
            let new = MenuItem::new(label("ccAgent.sidebar.sessionMenu.openInNewWindow"))
                .icon(IconName::AppWindow)
                .keys(label("chat.markdownRenderer.shiftClick"))
                .on_click(move |window, cx| open(&apart, &other, true, window, cx));
            Menu::new().item(current).item(new)
        }
        Act::Project(href) => {
            let href = href.clone();
            let locate = MenuItem::new(label("chat.markdownRenderer.locateProject"))
                .icon(IconName::FolderOpen)
                .on_click(move |window, cx| locate(&href, window, cx));
            Menu::new().item(locate)
        }
    }
}

/// The chip as `role="button" tabIndex={0}`: its clicks, keys and menu.
pub fn listen(
    el: Stateful<Div>,
    id: &ElementId,
    act: &Act,
    shared: &Shared,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let stop = ElementId::NamedChild(std::sync::Arc::new(id.clone()), "focus".into());
    let focus = window
        .use_keyed_state(stop, cx, |_, cx| cx.focus_handle().tab_stop(true))
        .read(cx)
        .clone();
    let name = SharedString::from(id.to_string());
    let spot: Rc<Cell<Bounds<Pixels>>> = Rc::default();
    let drawn = spot.clone();
    let probe = canvas(move |bounds, _, _| drawn.set(bounds), |_, _, _, _| {});
    let what = What::Chip(act.clone(), name.clone());
    let (left, middle, right, keys) = (what.clone(), what.clone(), what.clone(), what);
    let (by_key, by_press) = (shared.asked().clone(), shared.asked().clone());
    let menu = focus.clone();
    // A session link's middle click opens apart; a project link has no `onAuxClick`.
    let aux = matches!(act, Act::Session(_));
    let go = |what: &What, held: (Modifiers, MouseButton), window: &Window, cx: &mut App| {
        if let What::Chip(act, id) = what {
            activate((act, id), held, window, cx);
        }
    };
    el.relative()
        .track_focus(&focus)
        .role(Role::Button)
        .child(probe.absolute().inset_0())
        .on_key_down(move |event, window, cx| {
            let k = &event.keystroke;
            if k.key == "menu" || (k.key == "f10" && k.modifiers.shift) {
                window.prevent_default();
                cx.stop_propagation();
                // `linkMenuPosition` for a key: the chip's left and bottom.
                let bounds = spot.get();
                let under = point(bounds.left(), bounds.bottom());
                links::ask(&by_key, keys.clone(), under, cx);
            } else if matches!(k.key.as_str(), "enter" | "space") {
                window.prevent_default();
                go(&keys, (k.modifiers, MouseButton::Left), window, cx);
            }
        })
        // Keys activate on key down above; gpui's key-up click is not Cindy's.
        .on_click(move |event, window, cx| {
            if matches!(event, ClickEvent::Mouse(_)) {
                cx.stop_propagation();
                go(&left, (event.modifiers(), MouseButton::Left), window, cx);
            }
        })
        .on_aux_click(move |event, window, cx| {
            let ClickEvent::Mouse(mouse) = event else {
                return;
            };
            if aux && mouse.up.button == MouseButton::Middle {
                cx.stop_propagation();
                go(
                    &middle,
                    (event.modifiers(), MouseButton::Middle),
                    window,
                    cx,
                );
            }
        })
        .on_mouse_down(MouseButton::Right, move |event, window, cx| {
            window.prevent_default();
            cx.stop_propagation();
            // The stop keeps the press from gpui's own focus: the menu hands focus back here.
            menu.focus(window, cx);
            links::ask(&by_press, right.clone(), event.position, cx);
        })
}

#[cfg(test)]
mod tests;
