//! `sidebarSettingsStore.ts`'s read of `owners/<key>/sidebar-settings.json`.

use std::{io::ErrorKind, path::Path, time::Duration};

use anyhow::{Context as _, Result, bail};
use serde_json::{Map, Value, value::RawValue};

use super::{
    lists::entry_identity,
    paths::{compare, normalize_key},
};

pub const FILE: &str = "sidebar-settings.json";
const MAX_BYTES: u64 = 4 * 1024 * 1024;
const MAX_ENTRIES: usize = 10_000;
const MAX_LENGTH: usize = 4_096;
/// `updateAtomic`'s wait for another process.
const LOCK_WAIT: Duration = Duration::from_secs(12);

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Settings {
    pub pinned_order: Vec<String>,
    pub hidden: Vec<String>,
}

fn strings(raw: Option<&RawValue>) -> Vec<String> {
    let Some(Value::Array(items)) = raw.and_then(|r| crate::cindy_json::roundtrip(r.get()).ok())
    else {
        return Vec::new();
    };
    items
        .into_iter()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect()
}

/// JavaScript's `length`: UTF-16 units.
fn js_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// `normalizeSidebarPinnedOrder`.
fn pinned_order(raw: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for entry in raw {
        if entry.is_empty() || js_len(&entry) > MAX_LENGTH || out.contains(&entry) {
            continue;
        }
        out.push(entry);
        if out.len() >= MAX_ENTRIES {
            break;
        }
    }
    out
}

/// The items of a JSON array, each as `String(item)`; `null` items are skipped.
fn js_strings(raw: Option<&RawValue>) -> Vec<String> {
    let items = raw.and_then(|r| serde_json::from_str::<Vec<Box<RawValue>>>(r.get()).ok());
    items
        .unwrap_or_default()
        .iter()
        .filter(|v| v.get() != "null")
        .filter_map(|v| crate::cindy_json::js_string_raw(v).ok())
        .collect()
}

/// `normalizeHiddenProjectKeys`: `normalizeProjectKey` of every item, one per comparison identity.
fn hidden(raw: Vec<String>, platform: &str) -> Vec<String> {
    let (mut seen, mut out) = (Vec::new(), Vec::new());
    for entry in raw {
        let (Some(key), Some(identity)) = (normalize_key(&entry), compare(&entry, platform)) else {
            continue;
        };
        if js_len(&key) > MAX_LENGTH || seen.contains(&identity) {
            continue;
        }
        seen.push(identity);
        out.push(key);
        if out.len() >= MAX_ENTRIES {
            break;
        }
    }
    out
}

/// The owner's sidebar settings; a missing or unreadable file reads as the defaults.
pub fn read(owner_dir: &Path, platform: &str) -> Settings {
    let path = owner_dir.join(FILE);
    match crate::cindy_json::read(&path, Some(MAX_BYTES)) {
        Ok(None) => Settings::default(),
        Ok(Some(fields)) => Settings {
            pinned_order: pinned_order(strings(fields.get("pinnedOrder").map(|v| &**v))),
            hidden: hidden(
                js_strings(fields.get("hiddenProjectKeys").map(|v| &**v)),
                platform,
            ),
        },
        Err(_) => {
            log::warn!(target: "sidebar-settings", "sidebar settings read failed; falling back to defaults: {}", path.display());
            Settings::default()
        }
    }
}

/// `applyPinnedMutation`'s `promote`: the entry first, its other spellings dropped.
pub fn promoted(current: &[String], entry: &str, platform: &str) -> Vec<String> {
    let identity = entry_identity(entry, platform);
    let first = current
        .first()
        .filter(|c| entry_identity(c, platform) == identity);
    let rest: Vec<String> = current
        .iter()
        .filter(|c| entry_identity(c, platform) != identity)
        .cloned()
        .collect();
    if first.is_some() && rest.len() + 1 == current.len() {
        return current.to_vec();
    }
    let head = first.cloned().unwrap_or_else(|| entry.to_owned());
    std::iter::once(head).chain(rest).collect()
}

/// `applyPinnedMutation`'s `remove`: every spelling of the entry dropped.
pub fn removed(current: &[String], entry: &str, platform: &str) -> Vec<String> {
    let identity = entry_identity(entry, platform);
    let kept = current
        .iter()
        .filter(|c| entry_identity(c, platform) != identity);
    kept.cloned().collect()
}

/// `sidebarPathState` for a local owner: a regular file or none; a left `.bak` blocks.
fn writable(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() => return Ok(()),
        Ok(_) => bail!("sidebar settings migration is pending"),
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => bail!("sidebar settings migration is pending: {error}"),
    }
    match std::fs::symlink_metadata(crate::cross_process_lock::append_suffix(path, ".bak")) {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        _ => bail!("sidebar settings migration is pending: a backup is left"),
    }
}

/// `SidebarPinnedOrderMutation`: the kinds this port sends.
#[derive(Clone, Debug, PartialEq)]
pub enum Mutation {
    Promote(String),
    Remove(String),
    /// A drag that started from `base`.
    Reorder {
        base: Vec<String>,
        order: Vec<String>,
    },
}

/// `requirePinnedEntry`.
fn entry_ok(entry: &str) -> bool {
    !entry.is_empty() && js_len(entry) <= MAX_LENGTH
}

/// `requirePinnedOrder`: bounded, valid entries, no repeats.
fn order_ok(order: &[String]) -> bool {
    let unique: std::collections::HashSet<&String> = order.iter().collect();
    order.len() <= MAX_ENTRIES && order.iter().all(|e| entry_ok(e)) && unique.len() == order.len()
}

impl Mutation {
    fn check(&self) -> Result<()> {
        match self {
            Mutation::Promote(entry) | Mutation::Remove(entry) if !entry_ok(entry) => {
                bail!("invalid sidebar pinned entry")
            }
            Mutation::Reorder { base, order } if !order_ok(base) || !order_ok(order) => {
                bail!("invalid sidebar pinned order")
            }
            _ => Ok(()),
        }
    }

    /// `applyPinnedMutation`.
    fn apply(&self, current: &[String], platform: &str) -> Vec<String> {
        match self {
            Mutation::Promote(entry) => promoted(current, entry, platform),
            Mutation::Remove(entry) => removed(current, entry, platform),
            Mutation::Reorder { base, order } => rebased(current, base, order, platform),
        }
    }
}

/// `rebasePinnedReorder`: a drag laid on the latest order. Another window's new entries keep
/// their slot; entries it removed stay removed.
fn rebased(current: &[String], base: &[String], desired: &[String], platform: &str) -> Vec<String> {
    let identity = |e: &str| entry_identity(e, platform);
    let base: Vec<String> = base.iter().map(|e| identity(e)).collect();
    let mut durable: Vec<(String, &String)> = Vec::new();
    for entry in current {
        let id = identity(entry);
        if !durable.iter().any(|(d, _)| *d == id) {
            durable.push((id, entry));
        }
    }
    let (mut out, mut placed): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    for entry in desired {
        let id = identity(entry);
        if placed.contains(&id) {
            continue;
        }
        let kept = durable.iter().find(|(d, _)| *d == id).map(|(_, e)| *e);
        if base.contains(&id) && kept.is_none() {
            continue;
        }
        out.push(kept.unwrap_or(entry).clone());
        placed.push(id);
    }
    for (index, (id, entry)) in durable.iter().enumerate() {
        if base.contains(id) || placed.contains(id) {
            continue;
        }
        out.insert(index.min(out.len()), (*entry).clone());
        placed.push(id.clone());
    }
    pinned_order(out)
}

/// `updateAtomic`: under the lock, the file's fields with one field set from `next`.
fn update<T>(
    owner_dir: &Path,
    field: &str,
    next: impl FnOnce(&Map<String, Value>) -> Result<(Value, T)>,
) -> Result<T> {
    let path = owner_dir.join(FILE);
    writable(&path)?;
    std::fs::create_dir_all(owner_dir)?;
    let lock = crate::cross_process_lock::append_suffix(&path, ".lock");
    crate::cross_process_lock::with_lock(&lock, "sidebar-settings", LOCK_WAIT, |status| {
        if status != crate::cross_process_lock::Status::Held {
            bail!("sidebar settings are busy in another process");
        }
        writable(&path)?;
        let fields = crate::cindy_json::read(&path, Some(MAX_BYTES))
            .context("sidebar settings file is unreadable; refusing to overwrite it")?
            .unwrap_or_default();
        let mut overrides = Map::new();
        for (key, raw) in &fields {
            overrides.insert(key.clone(), crate::cindy_json::roundtrip(raw.get())?);
        }
        let (value, out) = next(&overrides)?;
        overrides.insert(field.into(), value);
        let text = serde_json::to_string_pretty(&Value::Object(overrides))?;
        if text.len() as u64 > MAX_BYTES {
            bail!("file exceeds {MAX_BYTES} byte limit");
        }
        let tmp = crate::cross_process_lock::append_suffix(&path, ".tmp");
        std::fs::write(&tmp, text).context("sidebar settings temp write")?;
        std::fs::rename(&tmp, &path).context("sidebar settings rename")?;
        Ok(out)
    })
}

/// `savePinnedOrder` through `updateAtomic`: the owner's order with `mutation` applied.
pub fn save(owner_dir: &Path, mutation: &Mutation, platform: &str) -> Result<Vec<String>> {
    mutation.check()?;
    update(owner_dir, "pinnedOrder", |fields| {
        let current = pinned_order(json_strings(fields.get("pinnedOrder")));
        let next = pinned_order(mutation.apply(&current, platform));
        log::info!(target: "sidebar-settings", "pinned order saved: {} entries, changed {}", next.len(), next != current);
        Ok((Value::from(next.clone()), next))
    })
}

fn json_strings(value: Option<&Value>) -> Vec<String> {
    let Some(Value::Array(items)) = value else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect()
}

/// `setProjectHidden` hiding a project: its key appended unless one spelling is there.
pub fn hide(owner_dir: &Path, key: &str, platform: &str) -> Result<Vec<String>> {
    let Some(key) = normalize_key(key).filter(|k| {
        js_len(k) <= MAX_LENGTH
            && ["local:", "remote:", "device:"]
                .iter()
                .any(|p| k.starts_with(p))
    }) else {
        bail!("invalid sidebar project key");
    };
    update(owner_dir, "hiddenProjectKeys", |fields| {
        let raw = fields.get("hiddenProjectKeys").map(|v| v.to_string());
        let raw = raw
            .as_deref()
            .and_then(|r| RawValue::from_string(r.to_owned()).ok());
        let mut keys = hidden(js_strings(raw.as_deref()), platform);
        let identity = compare(&key, platform);
        if !keys.iter().any(|k| compare(k, platform) == identity) {
            if keys.len() >= MAX_ENTRIES {
                bail!("too many hidden sidebar projects");
            }
            keys.push(key.clone());
            log::info!(target: "sidebar-settings", "project hidden: {} hidden", keys.len());
        }
        Ok((Value::from(keys.clone()), keys))
    })
}
