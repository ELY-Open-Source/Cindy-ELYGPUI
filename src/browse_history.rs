//! `sessionBrowseHistory.ts` with `useSessionBrowseHistory`: back and forward through tasks.

use std::path::PathBuf;

use gpui::{App, Global};

use crate::{router, sessions::Sessions};

/// `MAX_ENTRIES`.
const MAX: usize = 50;
/// `NON_SESSION_SEGMENTS`.
const NOT_TASKS: [&str; 4] = ["new", "new-dialogue", "scheduled", "boot"];

/// `sessionIdFromPathname`: `/cc-agent/<id>` with no further segment.
pub fn session_id(path: &str) -> Option<&str> {
    let id = path.strip_prefix("/cc-agent/")?;
    (!id.is_empty() && !id.contains('/') && !NOT_TASKS.contains(&id)).then_some(id)
}

/// A hop waiting for its route: `-1`, `1`, or `0` to return to the cursor's task.
struct Pending {
    direction: isize,
    target: String,
}

/// The window's stack, the `sessionBrowseHistory` singleton with the hook's refs.
#[derive(Default)]
pub struct History {
    ids: Vec<String>,
    /// `-1` while empty.
    index: isize,
    owner: Option<PathBuf>,
    pending: Option<Pending>,
    last_path: String,
}

impl Global for History {}

impl History {
    fn new() -> Self {
        Self {
            index: -1,
            ..Self::default()
        }
    }

    fn at(&self, index: isize) -> Option<&str> {
        usize::try_from(index)
            .ok()
            .and_then(|i| self.ids.get(i))
            .map(String::as_str)
    }

    /// `record`: the task already under the cursor takes no slot; the forward side drops.
    fn record(&mut self, id: &str) {
        if self.at(self.index) == Some(id) {
            return;
        }
        self.ids.truncate((self.index + 1) as usize);
        self.ids.push(id.to_owned());
        if self.ids.len() > MAX {
            self.ids.drain(..self.ids.len() - MAX);
        }
        self.index = self.ids.len() as isize - 1;
    }

    /// `step`: one hop, none past either end.
    fn step(&mut self, direction: isize) -> Option<String> {
        let next = self.index + direction;
        let id = self.at(next)?.to_owned();
        self.index = next;
        Some(id)
    }

    /// `bindOwner`: another data owner empties the stack.
    fn bind_owner(&mut self, owner: Option<PathBuf>) {
        if owner == self.owner {
            return;
        }
        log::info!(target: "session-history", "data owner changed; the stack empties");
        self.owner = owner;
        (self.ids, self.index) = (Vec::new(), -1);
    }
}

/// Follows the route and the data owner from now on.
pub fn install(cx: &mut App) {
    cx.set_global(History::new());
    cx.observe_global::<router::Router>(landed).detach();
    cx.observe_global::<Sessions>(|cx| {
        let owner = cx.global::<Sessions>().owner_dir.clone();
        if cx.global::<History>().owner != owner {
            cx.global_mut::<History>().bind_owner(owner);
        }
    })
    .detach();
    landed(cx);
}

/// The hook's route effect: a hop moves the cursor once it lands, else the task is recorded.
fn landed(cx: &mut App) {
    let path = router::path(cx).to_owned();
    let id = session_id(&path).map(str::to_owned);
    let h = cx.global_mut::<History>();
    if let Some(pending) = h.pending.take() {
        if id.as_deref() == Some(pending.target.as_str()) {
            if pending.direction != 0 {
                h.step(pending.direction);
            }
            return;
        }
        if path == h.last_path {
            h.pending = Some(pending);
        }
    }
    h.last_path = path;
    if let Some(id) = id {
        h.record(&id);
    }
}

/// `go`: off the stack, back returns to the cursor's task and forward does nothing.
pub fn go(direction: isize, cx: &mut App) -> bool {
    let h = cx.global::<History>();
    let current = session_id(router::path(cx));
    let on_stack = current.is_some() && h.at(h.index) == current;
    let (hop, target) = match on_stack {
        true => (direction, h.at(h.index + direction)),
        false if direction == 1 => return false,
        false => (0, h.at(h.index)),
    };
    let Some(target) = target.map(str::to_owned) else {
        return false;
    };
    log::info!(target: "session-history", "hop {hop} to {target}");
    cx.global_mut::<History>().pending = Some(Pending {
        direction: hop,
        target: target.clone(),
    });
    router::navigate(&format!("/cc-agent/{target}"), cx);
    true
}

/// `canBack` and `canForward`.
pub fn can(cx: &App) -> (bool, bool) {
    let h = cx.global::<History>();
    let current = session_id(router::path(cx));
    let on_stack = current.is_some() && h.at(h.index) == current;
    let back = if on_stack { h.index > 0 } else { h.index >= 0 };
    let forward = on_stack && h.index >= 0 && (h.index as usize) < h.ids.len() - 1;
    (back, forward)
}

#[cfg(test)]
mod tests;
