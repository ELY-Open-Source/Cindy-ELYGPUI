//! `AppShortcutStore.ts` reads and `lib/appShortcutStore.ts`: the effective shortcuts.

use std::{fs, io::ErrorKind, path::Path};

use gpui::{Global, KeyDownEvent};

use super::{Combo, effective, format, keys, normalize_overrides};
use serde_json::value::RawValue;

pub const FILE: &str = "app-shortcuts.v1.json";

/// Cindy's `process.platform` for this build.
pub const PLATFORM: &str = if cfg!(target_os = "macos") {
    "darwin"
} else if cfg!(target_os = "windows") {
    "win32"
} else {
    "linux"
};

/// The effective shortcuts, as `lib/appShortcutStore.ts` keeps them for every window.
pub struct AppShortcuts {
    effective: Vec<(&'static str, Vec<Combo>)>,
}

impl Global for AppShortcuts {}

impl AppShortcuts {
    /// `AppShortcutStore.load`: a bad file is logged and the defaults stand.
    pub fn load(user_data: &Path) -> Self {
        let path = user_data.join(FILE);
        let overrides = match fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<Box<RawValue>>(&text) {
                Ok(parsed) => {
                    let fields = super::fields(&parsed);
                    let overrides = fields.as_ref().and_then(|f| f.get("overrides"));
                    normalize_overrides(overrides.map(AsRef::as_ref), PLATFORM)
                }
                Err(error) => {
                    log::warn!(target: "app-shortcuts:store", "app shortcut overrides read failed, using defaults {{ error: '{error}' }}");
                    Vec::new()
                }
            },
            Err(error) if error.kind() == ErrorKind::NotFound => Vec::new(),
            Err(error) => {
                log::warn!(target: "app-shortcuts:store", "app shortcut overrides read failed, using defaults {{ error: '{error}' }}");
                Vec::new()
            }
        };
        log::info!(target: "app-shortcuts:store", "{} user bindings", overrides.len());
        AppShortcuts {
            effective: effective(&overrides, PLATFORM, &[]),
        }
    }

    pub fn combos(&self, id: &str) -> &[Combo] {
        self.effective
            .iter()
            .find(|(i, _)| *i == id)
            .map_or(&[], |(_, c)| c)
    }

    /// `useAppShortcut`'s match: held repeats never fire.
    pub fn matches(&self, id: &str, event: &KeyDownEvent) -> bool {
        if event.is_held {
            return false;
        }
        let pressed = keys::pressed(&event.keystroke);
        self.combos(id)
            .iter()
            .any(|c| pressed.as_ref().is_some_and(|p| p.same(c)))
    }

    /// `useAppShortcutDisplay`: the first combo, or nothing.
    pub fn display(&self, id: &str) -> String {
        self.combos(id)
            .first()
            .map_or(String::new(), |c| format(c, PLATFORM))
    }
}
