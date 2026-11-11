//! The resolver's shared cache: 30 seconds an answer, 200 answers, the oldest out first.

use std::{
    collections::{HashMap, VecDeque},
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

use super::{Options, Resolved, resolve, resolve_all};

const TTL: Duration = Duration::from_secs(30);
const CAP: usize = 200;

/// Answers in the order they were first kept, as a JavaScript `Map` holds them.
#[derive(Default)]
pub struct Cache(VecDeque<(String, Resolved, Instant)>);

static CACHE: LazyLock<Mutex<Cache>> = LazyLock::new(Mutex::default);

fn key(dir: &str, href: &str) -> String {
    format!("{dir}::{href}")
}

impl Cache {
    /// The live answer `keep` accepts; any other entry under the key is dropped.
    pub fn take(
        &mut self,
        key: &str,
        now: Instant,
        keep: impl Fn(&Resolved) -> bool,
    ) -> Option<Resolved> {
        let at = self.0.iter().position(|(held, ..)| held == key)?;
        let (_, result, expires) = &self.0[at];
        if *expires > now && keep(result) {
            return Some(result.clone());
        }
        self.0.remove(at);
        None
    }

    pub fn put(&mut self, key: String, result: Resolved, now: Instant) {
        if self.0.len() >= CAP {
            self.0.pop_front();
        }
        let entry = (key, result, now + TTL);
        match self.0.iter().position(|(held, ..)| *held == entry.0) {
            Some(at) => self.0[at] = entry,
            None => self.0.push_back(entry),
        }
    }
}

fn shared() -> std::sync::MutexGuard<'static, Cache> {
    CACHE
        .lock()
        .expect("no resolver panics with the cache held")
}

/// `resolveWorkspacePathCached`; `refresh` reads the folder again.
pub fn cached(href: &str, dir: &str, refresh: bool, options: &Options) -> Resolved {
    let (key, now) = (key(dir, href), Instant::now());
    if let Some(hit) = shared().take(&key, now, |_| !refresh) {
        return hit;
    }
    let result = resolve(href, dir, options);
    shared().put(key, result.clone(), now);
    result
}

/// `resolveWorkspacePathBatchCached`; `revalidate` reads again what was not one hit.
pub fn all_cached(
    hrefs: &[String],
    dir: &str,
    revalidate: bool,
    options: &Options,
) -> HashMap<String, Resolved> {
    let now = Instant::now();
    let (mut out, mut missed) = (HashMap::new(), Vec::new());
    for href in hrefs {
        if out.contains_key(href) || missed.contains(href) {
            continue;
        }
        let keep = |held: &Resolved| !revalidate || matches!(held, Resolved::Unique(..));
        match shared().take(&key(dir, href), now, keep) {
            Some(hit) => _ = out.insert(href.clone(), hit),
            None => missed.push(href.clone()),
        }
    }
    if missed.is_empty() {
        return out;
    }
    let mut resolved = resolve_all(&missed, dir, options);
    for href in missed {
        let result = resolved.remove(&href).expect("every href is resolved");
        shared().put(key(dir, &href), result.clone(), now);
        out.insert(href, result);
    }
    out
}
