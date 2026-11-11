//! `useWorkspaceMediaPath`: a relative media path of a local task, searched for in its folder.
#![cfg_attr(
    not(test),
    expect(dead_code, reason = "the image's box draws with 9.6i3")
)]

use std::collections::{HashMap, HashSet};

use gpui::{App, Global};

use crate::{
    local_path::{
        self,
        smart::{self, Smart},
    },
    path_resolver::Kind,
};

#[cfg(test)]
mod tests;

/// `isRelativeLocalPath`: no drive, no scheme, no root.
fn relative(src: &str) -> bool {
    let b = src.as_bytes();
    let drive = b.len() > 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && is_slash(b[2]);
    let word = |c: &u8| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'+' | b'.' | b'-');
    let named = b.iter().skip(1).take_while(|c| word(c)).count() + 1;
    let scheme = b.first().is_some_and(u8::is_ascii_alphabetic) && b.get(named) == Some(&b':');
    !drive && !scheme && !b.first().is_some_and(|c| is_slash(*c))
}

fn is_slash(c: u8) -> bool {
    c == b'/' || c == b'\\'
}

/// `lookupPath`: the segments the resolver looks up; none when the path leaves its folder.
fn lookup(src: &str) -> Option<String> {
    let mut segments: Vec<&str> = Vec::new();
    for segment in src.split(['\\', '/']) {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop()?;
            }
            _ => segments.push(segment),
        }
    }
    (!segments.is_empty()).then(|| segments.join("/"))
}

/// `Shown`.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Shown {
    Pending,
    Missing,
    Found,
}

/// `WorkspaceMediaPath`.
#[derive(Clone, Debug, PartialEq)]
pub struct Media {
    pub src: Option<String>,
    pub pending: bool,
    pub revision: u64,
}

/// A boundary's stamp, and the count that tells it from another at the same stamp.
type Boundary = (u64, u64);

/// One mounted hook: its effects' inputs and its state.
#[derive(Default)]
struct Hook {
    /// The href and folder the lookup effect last ran on.
    looked: Option<(Option<String>, String)>,
    /// The key and streaming flag the streaming effect last ran on.
    streamed: Option<(Option<String>, bool)>,
    run: u64,
    settled: Option<(String, Smart)>,
    boundary: Option<Boundary>,
    handled: Option<Boundary>,
    display: Option<(Option<String>, Shown)>,
    revision: u64,
}

/// The hooks of the media drawn, by each one's key in its row.
#[derive(Default)]
pub struct Mounted {
    hooks: HashMap<String, Hook>,
    drawn: HashSet<String>,
    /// `streamingMediaKeys`.
    streaming: HashSet<String>,
    count: u64,
}

impl Global for Mounted {}

/// Before the rows draw: no media is mounted yet.
pub fn begin(cx: &mut App) {
    cx.default_global::<Mounted>().drawn.clear();
}

/// After the rows: media no longer drawn has unmounted, and its state goes.
pub fn end(cx: &mut App) {
    let Mounted { hooks, drawn, .. } = cx.default_global::<Mounted>();
    hooks.retain(|id, _| drawn.contains(id));
}

/// The window's `focus`: a boundary for each hook that looks a path up.
pub fn focused(cx: &mut App) {
    let at = smart::now(cx);
    let Mounted { hooks, count, .. } = cx.default_global::<Mounted>();
    let listening = |hook: &&mut Hook| matches!(&hook.looked, Some((Some(_), _)));
    for hook in hooks.values_mut().filter(listening) {
        *count += 1;
        hook.boundary = Some((at, *count));
    }
    log::debug!(target: "markdown", "workspace media saw the window's focus");
}

/// `apply`, for the lookup the hook `id` joined in `run`: it keeps what the cache holds now.
fn landed(id: &str, run: u64, (href, dir): (&str, &str), cx: &mut App) {
    let found = smart::peek(href, dir, cx).cloned();
    let hook = cx.default_global::<Mounted>().hooks.get_mut(id);
    if let (Some(hook), Some(found)) = (hook.filter(|hook| hook.run == run), found) {
        log::debug!(target: "markdown", "workspace media {id} settled");
        hook.settled = Some((format!("{dir}::{href}"), found));
    }
}

/// The lookup effect of the hook `id` on `href` in `dir`, in a render that held `boundary`.
fn look(id: &str, href: &str, dir: &str, boundary: Option<Boundary>, cx: &mut App) {
    let key = format!("{dir}::{href}");
    let mounted = cx.default_global::<Mounted>();
    mounted.count += 1;
    let run = mounted.count;
    let hook = mounted.hooks.get_mut(id).expect("the hook is mounted");
    // Only a boundary this effect has not run on asks for its own recheck.
    let at = boundary.filter(|_| boundary != hook.handled);
    let at = at.map(|(at, _)| at);
    (hook.handled, hook.run) = (boundary, run);
    let own = matches!(&hook.settled, Some((held, Smart::Unique(..))) if *held == key);
    let then = {
        let (id, href, dir) = (id.to_owned(), href.to_owned(), dir.to_owned());
        move |cx: &mut App| landed(&id, run, (&href, &dir), cx)
    };
    if let Some(cached) = smart::peek(href, dir, cx).cloned() {
        let hook = cx.default_global::<Mounted>().hooks.get_mut(id);
        hook.expect("the hook is mounted").settled = Some((key, cached));
        if smart::recheck(href, dir, at, cx) {
            smart::join(href, dir, then, cx);
        }
    } else if !own {
        match at.is_some() && smart::recheck(href, dir, at, cx) {
            true => smart::join(href, dir, then, cx),
            false => smart::ask(href, dir, then, cx),
        }
    }
}

/// The hook's effects, in React's order: the lookup, then once more on a boundary just set.
fn effects(id: &str, href: Option<&str>, dir: &str, streaming: bool, cx: &mut App) {
    let key = href.map(|href| format!("{dir}::{href}"));
    let now = smart::now(cx);
    let mounted = cx.default_global::<Mounted>();
    mounted.drawn.insert(id.to_owned());
    let hook = mounted.hooks.entry(id.to_owned()).or_default();
    let before = hook.boundary;
    // A path that leaves a streaming answer is a boundary.
    let streamed = Some((key.clone(), streaming));
    if hook.streamed != streamed {
        hook.streamed = streamed;
        let left = match (key, streaming) {
            (None, _) => false,
            (Some(key), true) => {
                mounted.streaming.insert(key);
                false
            }
            (Some(key), false) => mounted.streaming.remove(&key),
        };
        if left {
            mounted.count += 1;
            hook.boundary = Some((now, mounted.count));
        }
    }
    let looked = Some((href.map(str::to_owned), dir.to_owned()));
    let changed = hook.looked != looked;
    hook.looked = looked;
    if changed {
        // The effect's cleanup: a lookup the last run joined applies no more.
        mounted.count += 1;
        hook.run = mounted.count;
    }
    let Some(href) = href else {
        return;
    };
    if changed {
        look(id, href, dir, before, cx);
    }
    let hook = &cx.default_global::<Mounted>().hooks[id];
    let boundary = hook.boundary;
    if boundary != hook.handled {
        look(id, href, dir, boundary, cx);
    }
}

/// The hook mounted as `id` for `src` in the task's folder `dir`.
pub fn path(id: &str, src: Option<&str>, dir: &str, flags: (bool, bool), cx: &mut App) -> Media {
    let (enabled, streaming) = flags;
    let wanted = src.filter(|src| {
        enabled && !src.is_empty() && !dir.is_empty() && relative(src) && !src.contains('%')
    });
    let href = wanted.and_then(lookup);
    let key = href.as_ref().map(|href| format!("{dir}::{href}"));
    effects(id, href.as_deref(), dir, streaming, cx);
    let cached = href.as_ref().and_then(|href| smart::peek(href, dir, cx));
    let cached = cached.cloned();
    let mounted = cx.default_global::<Mounted>();
    let hook = mounted.hooks.get_mut(id).expect("the hook is mounted");
    let own = hook
        .settled
        .as_ref()
        .filter(|(held, _)| Some(held) == key.as_ref());
    let result = cached.or_else(|| own.map(|(_, result)| result.clone()));
    let result = result.filter(|_| href.is_some());
    let shown = match &result {
        None => Shown::Pending,
        Some(Smart::Unique(_, Kind::File)) => Shown::Found,
        Some(_) => Shown::Missing,
    };
    // A path found after it was missing draws anew.
    let was = hook.display.replace((key.clone(), shown));
    if was == Some((key, Shown::Missing)) && shown == Shown::Found {
        hook.revision += 1;
    }
    let revision = hook.revision;
    let as_written = || src.map(str::to_owned);
    let (src, pending) = match (&href, result) {
        (None, _) => (as_written(), false),
        (Some(_), None) => (None, true),
        (Some(href), Some(Smart::Unique(found, Kind::File))) => {
            let slashed = |path: &str| path.replace('\\', "/");
            let joined = slashed(&local_path::resolve(href, dir));
            let kept = src == Some(href.as_str()) && slashed(&found) == joined;
            (if kept { as_written() } else { Some(found) }, false)
        }
        (Some(_), Some(_)) => (as_written(), false),
    };
    Media {
        src,
        pending,
        revision,
    }
}
