use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result, anyhow, bail};
use icu_collator::CollatorBorrowed;
use serde_json::Value;

use super::{Kind, ThemeData, normalize::normalize};

pub const SUFFIX: &str = "-local";

/// Themes read from the folder, and one line for each file that failed.
pub struct Loaded {
    pub themes: Vec<ThemeData>,
    pub diagnostics: Vec<(String, String)>,
}

/// `~/.cindy/themes`, after Cindy's one-time move from `~/.xdmaker/themes`.
pub fn dir(home: &Path) -> PathBuf {
    let dir = home.join(".cindy").join("themes");
    let old = home.join(".xdmaker").join("themes");
    if old.is_dir() && !dir.exists() {
        let moved = fs::create_dir_all(home.join(".cindy")).and_then(|()| fs::rename(&old, &dir));
        match moved {
            Ok(()) => {
                log::info!(target: "local-themes", "Migrated local themes dir from '{}' to '{}'.", old.display(), dir.display());
                if let Err(error) = fs::remove_dir(home.join(".xdmaker")) {
                    log::debug!(target: "local-themes", "kept ~/.xdmaker: {error}");
                }
            }
            Err(error) => {
                log::warn!(target: "local-themes", "Failed to migrate legacy local themes dir: {error}");
            }
        }
    }
    dir
}

fn non_empty(raw: &serde_json::Map<String, Value>, field: &str) -> Result<String> {
    match raw.get(field).and_then(Value::as_str) {
        Some(text) if !crate::theme_import::color::js_trim(text).is_empty() => Ok(text.to_owned()),
        _ => bail!("Invalid '{field}': expected non-empty string."),
    }
}

/// Cindy's `parseLocalThemeJson`.
fn parse(content: &str) -> Result<ThemeData> {
    let Value::Object(raw) = serde_json::from_str(content)? else {
        bail!("Invalid theme JSON: expected object.");
    };
    let id = non_empty(&raw, "id")?;
    non_empty(&raw, "name")?;
    let kind = match raw.get("type").and_then(Value::as_str) {
        Some("light") => Kind::Light,
        Some("dark") => Kind::Dark,
        _ => bail!("Invalid 'type': expected 'light' or 'dark'."),
    };
    let Some(Value::Object(entries)) = raw.get("colors") else {
        bail!("Invalid 'colors': expected object.");
    };
    let colors = entries
        .iter()
        .map(|(key, value)| match value {
            Value::String(text) => Ok((key.clone(), text.clone())),
            _ => Err(anyhow!("Invalid color '{key}': expected string value.")),
        })
        .collect::<Result<HashMap<_, _>>>()?;
    let family = raw
        .get("family")
        .and_then(Value::as_str)
        .map(crate::theme_import::color::js_trim)
        .filter(|f| !f.is_empty())
        .map(str::to_owned);
    Ok(ThemeData {
        id,
        kind,
        family,
        colors,
    })
}

/// Cindy's loader and `mapWireTheme`: sorted `.json` files, `-local` ids, normalized colors.
pub fn load(
    dir: &Path,
    defaults: &HashMap<String, (Option<String>, Option<String>)>,
) -> Result<Loaded> {
    fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    let mut files: Vec<String> = fs::read_dir(dir)
        .with_context(|| format!("read {}", dir.display()))?
        .map(|entry| Ok(entry?.file_name().to_string_lossy().into_owned()))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter(|name| name.to_lowercase().ends_with(".json"))
        .collect();
    let collator = CollatorBorrowed::try_new(Default::default(), Default::default())?;
    files.sort_by(|a, b| collator.compare(a, b));
    let mut loaded = Loaded {
        themes: Vec::new(),
        diagnostics: Vec::new(),
    };
    let mut seen = HashSet::new();
    for file in files {
        let parsed = fs::read_to_string(dir.join(&file))
            .map_err(anyhow::Error::from)
            .and_then(|c| parse(&c));
        let mut theme = match parsed {
            Ok(theme) => theme,
            Err(error) => {
                log::warn!(target: "local-themes", "Failed to load local theme '{file}': {error}");
                loaded.diagnostics.push((file, error.to_string()));
                continue;
            }
        };
        theme.id.push_str(SUFFIX);
        if !seen.insert(theme.id.clone()) {
            let error = format!(
                "Skipped duplicate local theme '{}' from '{file}'.",
                theme.id
            );
            log::warn!(target: "local-themes", "{error}");
            loaded.diagnostics.push((file, error));
            continue;
        }
        theme.colors = normalize(theme.colors, theme.kind, defaults);
        loaded.themes.push(theme);
    }
    log::info!(target: "local-themes", "{} local themes, {} skipped", loaded.themes.len(), loaded.diagnostics.len());
    Ok(loaded)
}
