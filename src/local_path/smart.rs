//! `localPathResolver.ts`'s smart lookups: the resolver asked off the main thread, answers kept.

use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};

use gpui::{App, AppContext as _, Global, Task};
use indexmap::IndexMap;

use super::{resolve, windows_like};
use crate::{
    file_path_policy::allowed,
    open_path::strip_drive_uri_slash,
    path_resolver::{
        Kind, Options, Resolved,
        cache::{all_cached, cached},
    },
};

#[cfg(test)]
mod tests;

/// `SMART_RESOLVE_CACHE_CAP`.
const CAP: usize = 500;
/// `NEGATIVE_RECHECK_MIN_MS`.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "workspace media paths recheck in 9.6i")
)]
const RECHECK_MIN: u64 = 2_000;

/// `SmartResolveResult`; `None` holds the legacy join.
#[derive(Clone, Debug, PartialEq)]
pub enum Smart {
    Unique(String, Kind),
    Multiple(Vec<String>),
    None(String),
}

/// What an asker does once the lookup it joined has landed.
type Then = Box<dyn FnOnce(&mut App)>;

/// Hrefs by folder, in the order they were asked.
type Batches = Vec<(String, Vec<String>)>;

/// The renderer's cache and its batch scheduler.
#[derive(Default)]
pub struct SmartPaths {
    /// Each answer with when it landed, oldest first.
    cache: IndexMap<String, (Smart, u64)>,
    asked: HashSet<String>,
    rechecking: HashSet<String>,
    /// When a lookup in flight left for the resolver; none while it is queued.
    sent: HashMap<String, u64>,
    /// Lookups sent before a boundary: a negative answer is checked once more.
    after: HashSet<String>,
    /// The askers of each lookup queued or in flight.
    waiters: HashMap<String, Vec<Then>>,
    pending: Batches,
    rechecks: Batches,
    scheduled: bool,
    /// Where this clock's milliseconds count from.
    epoch: Option<Instant>,
}

impl Global for SmartPaths {}

/// `Date.now()` on the executor clock: whole milliseconds, as Cindy's stamps are.
pub fn now(cx: &mut App) -> u64 {
    let now = cx.background_executor().now();
    let epoch = *cx.default_global::<SmartPaths>().epoch.get_or_insert(now);
    now.duration_since(epoch).as_millis() as u64
}

/// `smartResolveCacheKey`.
fn key(dir: &str, href: &str) -> String {
    format!("{dir}::{href}")
}

/// `toMainLookupHref`: a file URL's path, its `%` kept from the resolver's decode.
fn lookup(href: &str, dir: &str) -> String {
    match (href.starts_with("file://"), windows_like(dir)) {
        (true, _) => resolve(href, dir).replace('%', "%25"),
        (false, true) => strip_drive_uri_slash(href),
        (false, false) => href.to_owned(),
    }
}

fn smart(found: Resolved, fallback: String) -> Smart {
    match found {
        Resolved::Unique(path, kind) => Smart::Unique(path, kind),
        Resolved::Multiple(candidates) => Smart::Multiple(candidates),
        Resolved::None => Smart::None(fallback),
    }
}

/// The main process's options: its allow list and the user's home.
fn main<R>(call: impl FnOnce(&Options) -> R) -> R {
    let home = dirs::home_dir().expect("startup found the home folder");
    call(&Options {
        allowed: &allowed,
        max_candidates: 10,
        max_depth: 8,
        home: &home.to_string_lossy(),
    })
}

/// `resolveLocalPathSmart`: one lookup past this cache; `refresh` passes the resolver's too.
pub fn resolve_smart(href: &str, dir: &str, refresh: bool, cx: &App) -> Task<Smart> {
    let fallback = resolve(href, dir);
    let (lookup, dir) = (lookup(href, dir), dir.to_owned());
    cx.background_spawn(async move {
        let found = main(|options| cached(&lookup, &dir, refresh, options));
        log::debug!(target: "fs:resolve-path", "{lookup} in {dir}: {found:?}");
        smart(found, fallback)
    })
}

/// `peekResolveLocalPathSmart`.
pub fn peek<'a>(href: &str, dir: &str, cx: &'a App) -> Option<&'a Smart> {
    let held = cx.try_global::<SmartPaths>()?.cache.get(&key(dir, href));
    held.map(|(result, _)| result)
}

fn enqueue(batches: &mut Batches, dir: &str, href: &str) {
    let at = batches.iter().position(|(held, _)| held == dir);
    let at = at.unwrap_or_else(|| {
        batches.push((dir.to_owned(), Vec::new()));
        batches.len() - 1
    });
    batches[at].1.push(href.to_owned());
}

/// `scheduleSmartResolveFlush`: one flush once the code that asked is done.
fn schedule(cx: &mut App) {
    let paths = cx.default_global::<SmartPaths>();
    if !paths.scheduled {
        paths.scheduled = true;
        cx.defer(flush);
    }
}

/// `resolveLocalPathSmartCached`; `then` is its promise's `.then`, run once the answer is kept.
pub fn ask(href: &str, dir: &str, then: impl FnOnce(&mut App) + 'static, cx: &mut App) {
    let key = key(dir, href);
    let paths = cx.default_global::<SmartPaths>();
    if paths.cache.contains_key(&key) {
        return cx.defer(then);
    }
    let flying = paths.asked.contains(&key) || paths.rechecking.contains(&key);
    let waiters = paths.waiters.entry(key.clone()).or_default();
    waiters.push(Box::new(then));
    if !flying {
        paths.asked.insert(key);
        enqueue(&mut paths.pending, dir, href);
        schedule(cx);
    }
}

/// `.then` on the lookup `recheck` said is on its way.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "workspace media paths recheck in 9.6i")
)]
pub fn join(href: &str, dir: &str, then: impl FnOnce(&mut App) + 'static, cx: &mut App) {
    let paths = cx.default_global::<SmartPaths>();
    let waiters = paths.waiters.entry(key(dir, href)).or_default();
    waiters.push(Box::new(then));
}

/// `revalidateNegativeLocalPathSmart`: whether an answer is on its way.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "workspace media paths recheck in 9.6i")
)]
pub fn recheck(href: &str, dir: &str, boundary: Option<u64>, cx: &mut App) -> bool {
    let (key, now) = (key(dir, href), now(cx));
    let paths = cx.default_global::<SmartPaths>();
    let flying = paths.rechecking.contains(&key) || paths.asked.contains(&key);
    if let Some(boundary) = boundary
        && flying
    {
        // One still queued, or sent in a later millisecond, covers it.
        if paths.sent.get(&key).is_some_and(|sent| *sent <= boundary) {
            paths.after.insert(key);
        }
        return true;
    }
    let Some((cached, checked)) = paths.cache.get(&key) else {
        return false;
    };
    if matches!(cached, Smart::Unique(..)) {
        return false;
    }
    if paths.rechecking.contains(&key) {
        return true;
    }
    if boundary.is_none() && now - *checked < RECHECK_MIN {
        return false;
    }
    queue_recheck(key, href, dir, cx);
    true
}

/// `queueNegativeRevalidation`.
fn queue_recheck(key: String, href: &str, dir: &str, cx: &mut App) {
    let paths = cx.default_global::<SmartPaths>();
    if paths.rechecking.insert(key) {
        enqueue(&mut paths.rechecks, dir, href);
        schedule(cx);
    }
}

/// `flushSmartResolveBatch`: one call to the resolver per folder, rechecks apart.
fn flush(cx: &mut App) {
    let paths = cx.default_global::<SmartPaths>();
    paths.scheduled = false;
    let (plain, rechecks) = (
        std::mem::take(&mut paths.pending),
        std::mem::take(&mut paths.rechecks),
    );
    for (dir, hrefs) in plain {
        run(dir, hrefs, false, cx);
    }
    for (dir, hrefs) in rechecks {
        run(dir, hrefs, true, cx);
    }
}

/// `runBatchForDir`.
fn run(dir: String, hrefs: Vec<String>, revalidate: bool, cx: &mut App) {
    let now = now(cx);
    let paths = cx.default_global::<SmartPaths>();
    for href in &hrefs {
        paths.sent.insert(key(&dir, href), now);
    }
    let lookups: Vec<String> = hrefs.iter().map(|href| lookup(href, &dir)).collect();
    let mut seen = HashSet::new();
    let wanted = lookups.iter().filter(|lookup| seen.insert(*lookup));
    let wanted: Vec<String> = wanted.cloned().collect();
    log::debug!(
        target: "fs:resolve-path-batch",
        "{} paths in {dir}, revalidate {revalidate}", wanted.len()
    );
    let walk = cx.background_spawn({
        let dir = dir.clone();
        async move { main(|options| all_cached(&wanted, &dir, revalidate, options)) }
    });
    cx.spawn(async move |cx| {
        let found = walk.await;
        cx.update(|cx| {
            for (href, lookup) in hrefs.iter().zip(&lookups) {
                settle(
                    &dir,
                    href,
                    smart(found[lookup].clone(), resolve(href, &dir)),
                    cx,
                );
            }
            // The promises' callbacks run after the whole batch has settled.
            for href in &hrefs {
                let paths = cx.default_global::<SmartPaths>();
                let waiters = paths.waiters.remove(&key(&dir, href));
                waiters.into_iter().flatten().for_each(|then| then(cx));
            }
            cx.refresh_windows();
        });
    })
    .detach();
}

/// `settleSmartResult`, and the pass a boundary asked of a negative answer.
fn settle(dir: &str, href: &str, result: Smart, cx: &mut App) {
    let (key, now) = (key(dir, href), now(cx));
    let negative = !matches!(result, Smart::Unique(..));
    let paths = cx.default_global::<SmartPaths>();
    if !paths.cache.contains_key(&key) && paths.cache.len() >= CAP {
        paths.cache.shift_remove_index(0);
    }
    paths.cache.insert(key.clone(), (result, now));
    paths.asked.remove(&key);
    paths.rechecking.remove(&key);
    paths.sent.remove(&key);
    if paths.after.remove(&key) && negative {
        queue_recheck(key, href, dir, cx);
    }
}
