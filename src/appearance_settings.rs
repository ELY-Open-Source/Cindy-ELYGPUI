use std::path::Path;

use crate::cindy_json::{self, Fields, js_round, number, string};
use crate::theme_import::color::js_trim;

const FILE: &str = "appearance-settings.json";
const MAX_BYTES: u64 = 16 * 1024;
pub const DEFAULT_UI_SIZE: f64 = 14.0;

/// The settings the window shell reads: fonts for `applyFontSettings`, and the window zoom.
#[derive(Debug, PartialEq)]
pub struct Settings {
    pub ui_family: String,
    pub code_family: String,
    pub ui_size: f64,
    pub window_zoom: f64,
}

/// `clampAppearanceWindowZoom`: 0.5 to 3 in steps of 0.1, rounded to two places.
fn window_zoom(value: Option<f64>) -> f64 {
    let Some(value) = value.filter(|v| v.is_finite()) else {
        return 1.0;
    };
    let stepped = js_round(value / 0.1) * 0.1;
    js_round(stepped.clamp(0.5, 3.0) * 100.0) / 100.0
}

/// Cindy's `normalizeFamily`: a string, trimmed, at most 256 characters.
fn family(fields: &Fields, key: &str) -> String {
    let text = string(fields.get(key).map(AsRef::as_ref)).unwrap_or_default();
    js_trim(&text).chars().take(256).collect()
}

/// Cindy's appearance store read; a bad file means defaults and stays as it is.
pub fn read(user_data: &Path) -> Settings {
    let fields = match cindy_json::read(&user_data.join(FILE), Some(MAX_BYTES)) {
        Ok(fields) => fields.unwrap_or_default(),
        Err(error) => {
            log::warn!(target: "appearance-settings-store", "appearance settings read failed; falling back to defaults: {error:#}");
            Fields::new()
        }
    };
    let ui_size = match number(fields.get("uiSize").map(AsRef::as_ref)) {
        Some(size) if size.is_finite() => (size + 0.5).floor().clamp(12.0, 24.0),
        _ => DEFAULT_UI_SIZE,
    };
    let settings = Settings {
        ui_family: family(&fields, "uiFamily"),
        code_family: family(&fields, "codeFamily"),
        ui_size,
        window_zoom: window_zoom(number(fields.get("windowZoom").map(AsRef::as_ref))),
    };
    log::info!(target: "appearance-settings-store", "appearance settings loaded: {settings:?}");
    settings
}

/// `--text-N` as `applyFontSettings` scales it: `round(N * uiSize / 14)` CSS pixels.
pub fn text_size(size: f32, cx: &gpui::App) -> f32 {
    use ely_gpui_component::theme::ActiveTheme as _;
    js_round(f64::from(size * cx.theme().font_scale)) as f32
}

/// The first family of a CSS font list; gpui takes one family.
pub fn first_family(list: &str) -> Option<String> {
    let first = js_trim(list.split(',').next()?);
    let name = first.trim_matches(['"', '\'']);
    (!name.is_empty()).then(|| name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{Settings, first_family, read, window_zoom};

    #[test]
    fn reads_cindy_overrides_and_survives_bad_files() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("appearance-settings.json");
        let defaults = Settings {
            ui_family: String::new(),
            code_family: String::new(),
            ui_size: 14.0,
            window_zoom: 1.0,
        };
        assert_eq!(read(dir.path()), defaults);
        std::fs::write(&file, r#"{"uiSize": 30.4, "uiFamily": "  \"IBM Plex Sans\", sans-serif ", "wallpaperVisibility": 0.2}"#).unwrap();
        let loaded = read(dir.path());
        assert_eq!(
            (loaded.ui_size, loaded.ui_family.as_str()),
            (24.0, "\"IBM Plex Sans\", sans-serif")
        );
        std::fs::write(&file, r#"{"uiSize": 12.5}"#).unwrap();
        assert_eq!(read(dir.path()).ui_size, 13.0);
        std::fs::write(
            &file,
            r#"{"uiSize": 1e400, "uiFamily": "IBM Plex Sans", "x": {"y": -1e999}}"#,
        )
        .unwrap();
        let loaded = read(dir.path());
        assert_eq!(
            (loaded.ui_size, loaded.ui_family.as_str()),
            (14.0, "IBM Plex Sans")
        );
        std::fs::write(&file, r#"{"uiSize": "18", "uiFamily": 7}"#).unwrap();
        assert_eq!(read(dir.path()), defaults);
        std::fs::write(&file, "[1]").unwrap();
        assert_eq!(read(dir.path()), defaults);
        assert!(file.exists(), "a bad file stays for the user to fix");
        assert_eq!(
            first_family("\"IBM Plex Sans\", sans-serif").as_deref(),
            Some("IBM Plex Sans")
        );
        assert_eq!(first_family("  "), None);
        std::fs::write(&file, r#"{"windowZoom": 1.25}"#).unwrap();
        assert_eq!(read(dir.path()).window_zoom, 1.3);
    }

    /// Values from Cindy's `clampAppearanceWindowZoom`, run in node.
    #[test]
    fn clamps_zoom_as_cindy_does() {
        let cases = [
            (1.0, 1.0),
            (1.15, 1.1),
            (1.25, 1.3),
            (0.44, 0.5),
            (0.45, 0.5),
            (3.04, 3.0),
            (3.06, 3.0),
            (2.95, 3.0),
            (0.749, 0.7),
            (1.05, 1.1),
            (f64::INFINITY, 1.0),
            (f64::NAN, 1.0),
            (-1.0, 0.5),
        ];
        for (value, zoom) in cases {
            assert_eq!(window_zoom(Some(value)), zoom, "{value}");
        }
        assert_eq!(window_zoom(None), 1.0);
    }
}
