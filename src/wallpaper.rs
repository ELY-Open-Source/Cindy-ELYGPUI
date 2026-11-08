//! Cindy's wallpaper settings from `appearanceSettings.ts` and `custom-wallpaper-settings.ts`.

mod canvas;
mod media;

use std::path::Path;

use crate::cindy_json::{self, Fields, js_round, number, string};

pub use canvas::{Wallpaper, active, canvas, clear};

const CUSTOM_FILE: &str = "custom-wallpaper.json";
const CUSTOM_MAX_BYTES: u64 = 4096;

/// `WALLPAPER_IDS`: the picker order, then the retired ids saved selections keep.
const IDS: [&str; 18] = [
    "none",
    "cindy-whisper",
    "cindy-night-glance",
    "cindy-scarlet-stroll",
    "cindy-neon",
    "cindy-prism",
    "night-launch",
    "silent-orbits",
    "dune-dawn",
    "planet-dawn",
    "indigo-folds",
    "dune-sphere",
    "ocean-clouds",
    "rose-quartz",
    "custom",
    "cindy-window",
    "cindy-studio",
    "cindy-dream",
];

/// The wallpaper half of `AppearanceSettings`, with `customWallpaperUrl`.
#[derive(Clone, PartialEq)]
pub struct Settings {
    pub id: &'static str,
    pub overlay: f64,
    /// Absent for legacy preferences: the theme's old veil decides.
    pub visibility: Option<f64>,
    pub blur: Option<f64>,
    pub custom_url: String,
}

/// The custom URL stays out of logs, as `logLoadedValue: false` keeps it.
impl std::fmt::Debug for Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Settings")
            .field("id", &self.id)
            .field("overlay", &self.overlay)
            .field("visibility", &self.visibility)
            .field("blur", &self.blur)
            .field("custom", &!self.custom_url.is_empty())
            .finish()
    }
}

fn round2(value: f64) -> f64 {
    js_round(value * 100.0) / 100.0
}

/// `clampAppearanceWallpaperOverlay`: 0 to 1 in steps of 0.05.
fn overlay(value: f64) -> f64 {
    if !value.is_finite() {
        return 0.2;
    }
    round2((js_round(value / 0.05) * 0.05).clamp(0.0, 1.0))
}

/// `clampAppearanceWallpaperVisibility`.
fn visibility(value: f64) -> f64 {
    if value.is_finite() {
        round2(value.clamp(0.0, 1.0))
    } else {
        0.0
    }
}

/// `clampAppearanceWallpaperBlur`: whole CSS pixels, 0 to 20.
fn blur(value: f64) -> f64 {
    if value.is_finite() {
        js_round(value).clamp(0.0, 20.0)
    } else {
        0.0
    }
}

/// `normalizeCustomWallpaperUrl`.
fn custom_url(value: Option<String>) -> String {
    let Some(url) = value else {
        return String::new();
    };
    let Some(name) = url.strip_prefix("cindy-media://client-wallpaper/") else {
        return String::new();
    };
    let valid = name.split_once('.').is_some_and(|(hash, ext)| {
        hash.len() == 64
            && hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            && matches!(ext, "webp" | "mp4")
    });
    if valid { url } else { String::new() }
}

/// `customWallpaperStore.read().url`; unreadable files fall back without their details.
fn read_custom_url(user_data: &Path) -> String {
    match cindy_json::read(&user_data.join(CUSTOM_FILE), Some(CUSTOM_MAX_BYTES)) {
        Ok(fields) => custom_url(fields.and_then(|f| string(f.get("url").map(AsRef::as_ref)))),
        Err(_) => {
            log::warn!(target: "custom-wallpaper-settings", "custom-wallpaper settings unreadable; falling back to defaults");
            String::new()
        }
    }
}

/// `normalizeAppearanceSettings` for the wallpaper keys, then `readAppearanceSettings`.
pub fn read(fields: &Fields, user_data: &Path) -> Settings {
    let raw = |key: &str| fields.get(key).map(AsRef::as_ref);
    let saved = string(raw("wallpaperId"));
    let id = IDS
        .into_iter()
        .find(|id| saved.as_deref() == Some(*id))
        .unwrap_or("none");
    let custom_url = read_custom_url(user_data);
    Settings {
        id: if id == "custom" && custom_url.is_empty() {
            "none"
        } else {
            id
        },
        overlay: number(raw("wallpaperOverlay")).map_or(0.2, overlay),
        visibility: number(raw("wallpaperVisibility"))
            .filter(|v| v.is_finite())
            .map(visibility),
        blur: number(raw("wallpaperBlur"))
            .filter(|v| v.is_finite())
            .map(blur),
        custom_url,
    }
}

/// `getWallpaperVeil`: old 0–60% preferences unchanged, then up to a fully opaque veil.
fn veil(overlay_value: f64, dark: bool) -> f64 {
    let strength = overlay(overlay_value);
    let base = if dark { 65.0 } else { 55.0 };
    if strength <= 0.6 {
        return base + strength * 40.0;
    }
    let previous_max = base + 24.0;
    previous_max + ((strength - 0.6) / 0.4) * (100.0 - previous_max)
}

impl Settings {
    /// `getWallpaperVisibility`.
    pub fn visibility(&self, dark: bool) -> f64 {
        match self.visibility {
            Some(value) => visibility(value),
            None => (100.0 - veil(self.overlay, dark)) / 100.0,
        }
    }
}

/// No wallpaper, as a fresh profile has.
#[cfg(test)]
pub fn none_for_tests(cx: &mut gpui::App) {
    let settings = read(&Fields::new(), Path::new("/nonexistent"));
    Wallpaper::install(settings, Path::new("/nonexistent"), cx);
}

#[cfg(test)]
mod tests;
