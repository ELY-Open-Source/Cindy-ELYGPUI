//! `router.tsx`'s routes this chapter owns, with the redirects at `/` and `/cc-agent` and
//! `useTrackLastView`'s chat slot.

use std::borrow::Cow;

use gpui::{App, Global};

use crate::{
    cindy_json::decode_uri_component,
    preferences::Preferences,
    rich_answer::js::{self, Js},
    sessions,
};

/// `lastViewStore`'s chat slot.
const CHAT_KEY: &str = "cc-agent.lastChatView.v1";
const NEW: &str = "/cc-agent/new";

/// The hash route's location: its pathname and its search.
pub struct Router {
    location: String,
    /// `location.state.from` of a handoff card's navigation.
    from: Option<Handoff>,
}

/// `HandoffFromState`: the task whose card opened this one, and that task's title.
#[derive(Clone, Debug, PartialEq)]
pub struct Handoff {
    pub dispatcher: String,
    pub title: Option<String>,
}

impl Global for Router {}

#[derive(Debug, PartialEq)]
pub enum Route {
    /// `MainEntryRedirect`.
    Root,
    /// `CCAgentIndexRedirect`.
    Index,
    /// `NewMakerDraftRoute`.
    New,
    /// `CCAgentSessionView` with its `sessionId` param.
    Session(String),
    /// `ScheduledTasksView`.
    Scheduled,
    /// A route another chapter owns.
    Other,
}

/// `LastChatView`.
#[derive(Debug, PartialEq)]
enum LastChat {
    New,
    Session(String),
}

/// `matchRoutes`: the pathname decoded first, as the rendered route and `useParams` see it.
pub fn route(path: &str) -> Route {
    matched(&decode_path(path))
}

/// `decodePath`: each segment decoded, its own slashes kept apart as `%2F`.
fn decode_path(path: &str) -> Cow<'_, str> {
    if !path.contains('%') {
        return Cow::Borrowed(path);
    }
    let segment = |raw: &str| Some(decode_uri_component(raw)?.replace('/', "%2F"));
    let decoded: Option<Vec<String>> = path.split('/').map(segment).collect();
    match decoded {
        Some(segments) => Cow::Owned(segments.join("/")),
        None => {
            log::warn!(target: "router", "pathname {path} does not decode");
            Cow::Borrowed(path)
        }
    }
}

/// `matchPath` on a pathname as it stands, a trailing slash ignored.
fn matched(path: &str) -> Route {
    let path = match path.strip_suffix('/') {
        Some(rest) if !rest.is_empty() => rest,
        _ => path,
    };
    match path {
        "/" => return Route::Root,
        "/cc-agent" => return Route::Index,
        "/cc-agent/new" => return Route::New,
        "/cc-agent/scheduled" => return Route::Scheduled,
        _ => {}
    }
    match path.strip_prefix("/cc-agent/") {
        Some("boot" | "new-dialogue" | "files" | "orca") => Route::Other,
        // `matchPathImpl` gives a param its slashes back.
        Some(id) if !id.contains('/') => Route::Session(id.replace("%2F", "/")),
        _ => Route::Other,
    }
}

pub fn install(cx: &mut App) {
    cx.set_global(Router {
        location: "/".into(),
        from: None,
    });
    cx.observe_global::<sessions::Sessions>(settle).detach();
    settle(cx);
}

pub fn location(cx: &App) -> &str {
    &cx.global::<Router>().location
}

/// The location's pathname, which the routes match.
pub fn path(cx: &App) -> &str {
    pathname(location(cx))
}

fn pathname(location: &str) -> &str {
    location.split_once('?').map_or(location, |(path, _)| path)
}

/// The task `/cc-agent/:sessionId` shows, as the sidebar's `useMatch` reads it.
pub fn active_session(cx: &App) -> Option<String> {
    match route(path(cx)) {
        Route::Session(id) => Some(id),
        _ => None,
    }
}

/// `parseHandoffState` of the location's state.
pub fn handoff(cx: &App) -> Option<&Handoff> {
    cx.global::<Router>().from.as_ref()
}

/// `navigate(to)`: the new location carries no state.
pub fn navigate(to: &str, cx: &mut App) {
    go(to, None, cx);
}

/// `navigate(to, { state: { from } })`.
pub fn navigate_from(to: &str, from: Handoff, cx: &mut App) {
    go(to, Some(from), cx);
}

fn go(to: &str, from: Option<Handoff>, cx: &mut App) {
    let router = cx.global::<Router>();
    if router.location == to && router.from == from {
        return;
    }
    log::info!(target: "router", "navigate to {to}, handoff: {}", from.is_some());
    *cx.global_mut::<Router>() = Router {
        location: to.to_owned(),
        from,
    };
    track_last_view(pathname(to), cx);
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

/// `useTrackLastView` for the chat slot: `matchPath` on the pathname, which decodes nothing.
fn track_last_view(to: &str, cx: &mut App) {
    let view = match matched(to) {
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
