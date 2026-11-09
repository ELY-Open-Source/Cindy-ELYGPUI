//! `router.tsx`'s routes this chapter owns, with the redirects at `/` and `/cc-agent` and
//! `useTrackLastView`'s chat slot.

use gpui::{App, Global};

use crate::{
    preferences::Preferences,
    rich_answer::js::{self, Js},
    sessions,
};

/// `lastViewStore`'s chat slot.
const CHAT_KEY: &str = "cc-agent.lastChatView.v1";
const NEW: &str = "/cc-agent/new";

/// The hash route's pathname.
pub struct Router {
    path: String,
}

impl Global for Router {}

#[derive(Debug, PartialEq)]
pub enum Route<'a> {
    /// `MainEntryRedirect`.
    Root,
    /// `CCAgentIndexRedirect`.
    Index,
    /// `NewMakerDraftRoute`.
    New,
    /// `CCAgentSessionView`.
    Session(&'a str),
    /// A route another chapter owns.
    Other,
}

/// `LastChatView`.
#[derive(Debug, PartialEq)]
enum LastChat {
    New,
    Session(String),
}

/// React Router's match, a trailing slash ignored.
pub fn route(path: &str) -> Route<'_> {
    let path = match path.strip_suffix('/') {
        Some(rest) if !rest.is_empty() => rest,
        _ => path,
    };
    match path {
        "/" => return Route::Root,
        "/cc-agent" => return Route::Index,
        "/cc-agent/new" => return Route::New,
        _ => {}
    }
    match path.strip_prefix("/cc-agent/") {
        Some("boot" | "new-dialogue" | "scheduled" | "files" | "orca") => Route::Other,
        Some(id) if !id.contains('/') => Route::Session(id),
        _ => Route::Other,
    }
}

pub fn install(cx: &mut App) {
    cx.set_global(Router { path: "/".into() });
    cx.observe_global::<sessions::Sessions>(settle).detach();
    settle(cx);
}

pub fn path(cx: &App) -> &str {
    &cx.global::<Router>().path
}

pub fn navigate(to: &str, cx: &mut App) {
    if path(cx) == to {
        return;
    }
    log::info!(target: "router", "navigate to {to}");
    cx.global_mut::<Router>().path = to.to_owned();
    track_last_view(to, cx);
    settle(cx);
}

/// The redirects; the index waits for the task list, as Cindy's waits out `isLoading`.
fn settle(cx: &mut App) {
    let target = match route(path(cx)) {
        Route::Root => Some("/cc-agent".to_owned()),
        Route::Index => index_target(cx),
        _ if matches!(path(cx), "/cc-agent/new-dialogue" | "/cc-agent/orca/new") => {
            Some(NEW.to_owned())
        }
        _ => None,
    };
    if let Some(target) = target {
        navigate(&target, cx);
    }
}

/// `CCAgentIndexRedirect`: the last chat view while its task is still listed, else the first
/// task, else a draft.
fn index_target(cx: &App) -> Option<String> {
    let tasks = sessions::active_task_ids(cx)?;
    let route = match last_chat(cx.global::<Preferences>()) {
        Some(LastChat::New) => NEW.to_owned(),
        Some(LastChat::Session(id)) if tasks.contains(&id.as_str()) => format!("/cc-agent/{id}"),
        _ => tasks
            .first()
            .map_or(NEW.to_owned(), |id| format!("/cc-agent/{id}")),
    };
    log::info!(target: "router", "cc-agent index resolved to {route} among {}", tasks.len());
    Some(route)
}

/// `loadLastChatView`: anything that does not parse or validate reads as none.
fn last_chat(prefs: &Preferences) -> Option<LastChat> {
    let raw = prefs.text(CHAT_KEY).ok().flatten()?;
    let parsed = js::parse(raw).ok()?;
    let record = parsed.obj()?;
    match record.get("kind").and_then(Js::str)? {
        "new" => Some(LastChat::New),
        "session" => {
            let id = record.get("sessionId").and_then(Js::str)?;
            (!id.is_empty()).then(|| LastChat::Session(id.to_owned()))
        }
        _ => None,
    }
}

/// `useTrackLastView` for the chat slot.
fn track_last_view(to: &str, cx: &mut App) {
    let view = match route(to) {
        Route::New => serde_json::json!({ "kind": "new" }),
        Route::Session(id) => serde_json::json!({ "kind": "session", "sessionId": id }),
        _ => return,
    };
    let prefs = cx.global_mut::<Preferences>();
    if let Err(error) = prefs.set(CHAT_KEY, &view.to_string()) {
        log::error!(target: "router", "last chat view not saved: {error:#}");
    }
}

#[cfg(test)]
mod tests;
