//! `sidebarSettingsStore.ts`'s read of `owners/<key>/sidebar-settings.json`.

use std::path::Path;

use serde_json::{Value, value::RawValue};

use super::paths::{compare, normalize_key};

pub const FILE: &str = "sidebar-settings.json";
const MAX_BYTES: u64 = 4 * 1024 * 1024;
const MAX_ENTRIES: usize = 10_000;
const MAX_LENGTH: usize = 4_096;

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
