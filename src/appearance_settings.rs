use std::{collections::BTreeMap, fs, path::Path};

use anyhow::{Context as _, Result, bail};
use serde_json::value::RawValue;

use crate::theme_import::color::js_trim;

const FILE: &str = "appearance-settings.json";
const MAX_BYTES: u64 = 16 * 1024;
pub const DEFAULT_UI_SIZE: f64 = 14.0;

/// The font settings Cindy's `applyFontSettings` uses.
#[derive(Debug, PartialEq)]
pub struct Fonts {
    pub ui_family: String,
    pub code_family: String,
    pub ui_size: f64,
}

/// Top-level fields kept as JSON text, so one odd value cannot spoil the rest.
type Overrides = BTreeMap<String, Box<RawValue>>;

fn read_overrides(path: &Path) -> Result<Option<Overrides>> {
    let meta = match fs::metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        other => other?,
    };
    if meta.len() > MAX_BYTES {
        bail!("file exceeds {MAX_BYTES} byte limit");
    }
    let text = fs::read_to_string(path)?;
    if !js_trim(&text).starts_with('{') {
        bail!("settings file root must be an object");
    }
    Ok(Some(serde_json::from_str(&text)?))
}

/// Cindy's `normalizeFamily`: a string, trimmed, at most 256 characters.
fn family(raw: Option<&RawValue>) -> String {
    let text: Option<String> = raw.and_then(|r| serde_json::from_str(r.get()).ok());
    js_trim(text.as_deref().unwrap_or(""))
        .chars()
        .take(256)
        .collect()
}

/// A JSON number as `JSON.parse` reads it: one too large becomes infinite.
fn number(raw: Option<&RawValue>) -> Option<f64> {
    raw?.get().parse().ok()
}

/// Cindy's appearance store read, fonts only; a bad file means defaults and stays as it is.
pub fn fonts(user_data: &Path) -> Fonts {
    let path = user_data.join(FILE);
    let overrides = match read_overrides(&path).with_context(|| path.display().to_string()) {
        Ok(overrides) => overrides.unwrap_or_default(),
        Err(error) => {
            log::warn!(target: "appearance-settings-store", "appearance settings read failed; falling back to defaults: {error:#}");
            Overrides::new()
        }
    };
    let field = |key: &str| overrides.get(key).map(Box::as_ref);
    let ui_size = match number(field("uiSize")) {
        Some(size) if size.is_finite() => (size + 0.5).floor().clamp(12.0, 24.0),
        _ => DEFAULT_UI_SIZE,
    };
    let fonts = Fonts {
        ui_family: family(field("uiFamily")),
        code_family: family(field("codeFamily")),
        ui_size,
    };
    log::info!(target: "appearance-settings-store", "appearance settings loaded: {fonts:?}");
    fonts
}

/// The first family of a CSS font list; gpui takes one family.
pub fn first_family(list: &str) -> Option<String> {
    let first = js_trim(list.split(',').next()?);
    let name = first.trim_matches(['"', '\'']);
    (!name.is_empty()).then(|| name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{Fonts, first_family, fonts};

    #[test]
    fn reads_cindy_overrides_and_survives_bad_files() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("appearance-settings.json");
        let defaults = Fonts {
            ui_family: String::new(),
            code_family: String::new(),
            ui_size: 14.0,
        };
        assert_eq!(fonts(dir.path()), defaults);
        std::fs::write(&file, r#"{"uiSize": 30.4, "uiFamily": "  \"IBM Plex Sans\", sans-serif ", "wallpaperVisibility": 0.2}"#).unwrap();
        let read = fonts(dir.path());
        assert_eq!(
            (read.ui_size, read.ui_family.as_str()),
            (24.0, "\"IBM Plex Sans\", sans-serif")
        );
        std::fs::write(&file, r#"{"uiSize": 12.5}"#).unwrap();
        assert_eq!(fonts(dir.path()).ui_size, 13.0);
        std::fs::write(
            &file,
            r#"{"uiSize": 1e400, "uiFamily": "IBM Plex Sans", "x": {"y": -1e999}}"#,
        )
        .unwrap();
        let read = fonts(dir.path());
        assert_eq!(
            (read.ui_size, read.ui_family.as_str()),
            (14.0, "IBM Plex Sans")
        );
        std::fs::write(&file, r#"{"uiSize": "18", "uiFamily": 7}"#).unwrap();
        assert_eq!(fonts(dir.path()), defaults);
        std::fs::write(&file, "[1]").unwrap();
        assert_eq!(fonts(dir.path()), defaults);
        assert!(file.exists(), "a bad file stays for the user to fix");
        assert_eq!(
            first_family("\"IBM Plex Sans\", sans-serif").as_deref(),
            Some("IBM Plex Sans")
        );
        assert_eq!(first_family("  "), None);
    }
}
