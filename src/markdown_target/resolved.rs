//! `useResolvedMarkdownTarget` for a local task: a candidate lit once its path resolves.

use std::collections::{HashMap, HashSet};

use gpui::{App, Global};

#[cfg(test)]
mod tests;

use super::{Local, Target};
use crate::{
    local_path::{
        shape,
        smart::{self, Smart},
    },
    path_resolver::Kind,
};

/// `localKindFromAbsPath`.
fn kind(path: &str, fallback: Local) -> Local {
    match (shape::image(path), shape::model(path)) {
        (true, _) => Local::Image,
        (_, true) => Local::Model,
        _ => fallback,
    }
}

/// `resolvedLocalFromResult`: a unique answer alone lights the candidate.
pub fn lit(target: &Target, result: &Smart) -> Option<Target> {
    let (Target::Candidate { href, kind, at, .. }, Smart::Unique(path, found)) = (target, result)
    else {
        return None;
    };
    let kind = match found {
        Kind::Directory => Local::Directory,
        Kind::File => self::kind(path, *kind),
    };
    Some(Target::Resolved {
        href: href.clone(),
        path: path.clone(),
        kind,
        at: *at,
    })
}

/// One mounted hook: what it last ran on, which run that was, and its state.
struct Hook {
    deps: (Target, String, bool),
    run: u64,
    lit: Option<Target>,
}

/// The hooks of the targets drawn, by each target's key in its row.
#[derive(Default)]
pub struct Mounted {
    hooks: HashMap<String, Hook>,
    drawn: HashSet<String>,
    runs: u64,
}

impl Global for Mounted {}

/// Before the rows draw: no target is mounted yet.
pub fn begin(cx: &mut App) {
    cx.default_global::<Mounted>().drawn.clear();
}

/// After the rows: a target no longer drawn has unmounted, and its state goes.
pub fn end(cx: &mut App) {
    let Mounted { hooks, drawn, .. } = cx.default_global::<Mounted>();
    hooks.retain(|id, _| drawn.contains(id));
}

/// `resolveFromCache`: the state the cache gives now, or `None` while it holds no answer.
fn read(target: &Target, dir: &str, streaming: bool, cx: &App) -> Option<Option<Target>> {
    let Target::Candidate { href, .. } = target else {
        return Some(None);
    };
    if streaming {
        return Some(None);
    }
    let found = smart::peek(href, dir, cx)?;
    Some(lit(target, found))
}

/// The lookup hook `id` joined in `run` has landed: the hook takes what the cache holds now.
fn landed(id: &str, run: u64, cx: &mut App) {
    let hook = cx.default_global::<Mounted>().hooks.get(id);
    let Some((target, dir, streaming)) = hook
        .filter(|hook| hook.run == run)
        .map(|hook| hook.deps.clone())
    else {
        return;
    };
    let lit = read(&target, &dir, streaming, cx).flatten();
    log::debug!(target: "markdown", "target {id} resolved: {}", lit.is_some());
    if let Some(hook) = cx.default_global::<Mounted>().hooks.get_mut(id) {
        hook.lit = lit;
    }
}

/// The hook's state for the target mounted as `id`: it reads the cache when its inputs change.
pub fn resolved(
    id: &str,
    target: &Target,
    dir: &str,
    streaming: bool,
    cx: &mut App,
) -> Option<Target> {
    let mounted = cx.default_global::<Mounted>();
    mounted.drawn.insert(id.to_owned());
    let same = |hook: &&Hook| {
        let (held, at, was) = &hook.deps;
        held == target && at == dir && *was == streaming
    };
    if let Some(hook) = mounted.hooks.get(id).filter(same) {
        return hook.lit.clone();
    }
    mounted.runs += 1;
    let run = mounted.runs;
    let found = read(target, dir, streaming, cx);
    let lit = found.clone().flatten();
    let hook = Hook {
        deps: (target.clone(), dir.to_owned(), streaming),
        run,
        lit: lit.clone(),
    };
    let mounted = cx.default_global::<Mounted>();
    mounted.hooks.insert(id.to_owned(), hook);
    if let (None, Target::Candidate { href, .. }) = (found, target) {
        let id = id.to_owned();
        smart::ask(href, dir, move |cx| landed(&id, run, cx), cx);
    }
    lit
}
