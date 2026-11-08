use std::{fs, path::Path};

use anyhow::{Context as _, Result};
use gpui::{Bounds, Pixels, point, px, size};
use serde_json::{Map, Value, json};

use crate::cindy_json::{self, Fields, number, truthy};

pub const FILE: &str = "window-state.json";
const DEFAULT_WIDTH: f64 = 1280.0;
const DEFAULT_HEIGHT: f64 = 800.0;

/// `electron-window-state`'s state: bounds in the normal state, flags, the display it was on.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowState {
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub width: f64,
    pub height: f64,
    pub is_maximized: bool,
    pub is_full_screen: bool,
    pub display_bounds: Option<Bounds<Pixels>>,
}

/// `Number.isInteger`.
fn integer(fields: &Fields, key: &str) -> Option<f64> {
    number(fields.get(key).map(AsRef::as_ref)).filter(|f| f.is_finite() && f.fract() == 0.0)
}

fn bounds_of(fields: &Fields) -> Option<Bounds<Pixels>> {
    let inner = cindy_json::object(fields.get("displayBounds")?.get()).ok()?;
    let n = |k: &str| number(inner.get(k).map(AsRef::as_ref)).map(|f| px(f as f32));
    Some(Bounds {
        origin: point(n("x")?, n("y")?),
        size: size(n("width")?, n("height")?),
    })
}

fn bounds_json(b: Bounds<Pixels>) -> Value {
    let n = |p: Pixels| cindy_json::js_value(f64::from(f32::from(p)));
    json!({ "x": n(b.origin.x), "y": n(b.origin.y), "width": n(b.size.width), "height": n(b.size.height) })
}

/// Cindy's `readPersistedWindowMaximized`: the raw flag, before any validation.
pub fn persisted_maximized(raw: Option<&Fields>) -> bool {
    raw.and_then(|r| cindy_json::boolean(r.get("isMaximized").map(AsRef::as_ref))) == Some(true)
}

/// `electron-window-state` load and `validateState`; `primary` is gpui's, or the first
/// display where the platform names none (Wayland).
pub fn restore(
    raw: Option<&Fields>,
    displays: &[Bounds<Pixels>],
    primary: Option<Bounds<Pixels>>,
) -> WindowState {
    let fallback = WindowState {
        x: None,
        y: None,
        width: DEFAULT_WIDTH,
        height: DEFAULT_HEIGHT,
        is_maximized: false,
        is_full_screen: false,
        display_bounds: None,
    };
    let Some(raw) = raw else { return fallback };
    let flag = |k: &str| truthy(raw.get(k).map(AsRef::as_ref));
    let (x, y) = (integer(raw, "x"), integer(raw, "y"));
    let (w, h) = (
        integer(raw, "width").filter(|v| *v > 0.0),
        integer(raw, "height").filter(|v| *v > 0.0),
    );
    let has_bounds = x.is_some() && y.is_some() && w.is_some() && h.is_some();
    if !(has_bounds || flag("isMaximized") || flag("isFullScreen")) {
        return fallback;
    }
    let width = w
        .or_else(|| number(raw.get("width").map(AsRef::as_ref)))
        .unwrap_or(DEFAULT_WIDTH);
    let height = h
        .or_else(|| number(raw.get("height").map(AsRef::as_ref)))
        .unwrap_or(DEFAULT_HEIGHT);
    let state = WindowState {
        x,
        y,
        width,
        height,
        is_maximized: flag("isMaximized"),
        is_full_screen: flag("isFullScreen"),
        display_bounds: bounds_of(raw),
    };
    if !(has_bounds && flag("displayBounds")) {
        return state;
    }
    if displays.is_empty() {
        log::warn!(target: "window-state", "no displays known; saved bounds kept unchecked");
        return state;
    }
    let (sx, sy, sw, sh) = (x.unwrap_or_default(), y.unwrap_or_default(), width, height);
    let visible = displays.iter().any(|d| {
        let (dx, dy) = (
            f64::from(f32::from(d.origin.x)),
            f64::from(f32::from(d.origin.y)),
        );
        let (dw, dh) = (
            f64::from(f32::from(d.size.width)),
            f64::from(f32::from(d.size.height)),
        );
        sx >= dx && sy >= dy && sx + sw <= dx + dw && sy + sh <= dy + dh
    });
    if visible {
        return state;
    }
    log::info!(target: "window-state", "saved bounds are off every display; using defaults");
    WindowState {
        x: Some(0.0),
        y: Some(0.0),
        display_bounds: primary,
        ..fallback
    }
}

/// The file's fields; a bad file means defaults, as `electron-window-state` reads it.
pub fn read(user_data: &Path) -> Option<Fields> {
    match cindy_json::read(&user_data.join(FILE), None) {
        Ok(fields) => fields,
        Err(error) => {
            log::warn!(target: "window-state", "{error:#}; using defaults");
            None
        }
    }
}

impl WindowState {
    /// `updateState`: `normal` holds the bounds only in the normal state, flags always.
    pub fn update(
        &mut self,
        normal: Option<Bounds<Pixels>>,
        maximized: bool,
        full_screen: bool,
        display: Option<Bounds<Pixels>>,
    ) {
        if let Some(bounds) = normal {
            self.x = Some(f64::from(f32::from(bounds.origin.x)).round());
            self.y = Some(f64::from(f32::from(bounds.origin.y)).round());
            self.width = f64::from(f32::from(bounds.size.width)).round();
            self.height = f64::from(f32::from(bounds.size.height)).round();
        }
        self.is_maximized = maximized;
        self.is_full_screen = full_screen;
        if display.is_some() {
            self.display_bounds = display;
        }
    }

    /// `saveState`: `jsonfile` writes compact JSON and a newline.
    pub fn save(&self, user_data: &Path) -> Result<()> {
        let mut map = Map::new();
        map.insert("width".into(), cindy_json::js_value(self.width));
        map.insert("height".into(), cindy_json::js_value(self.height));
        if let (Some(x), Some(y)) = (self.x, self.y) {
            map.insert("x".into(), cindy_json::js_value(x));
            map.insert("y".into(), cindy_json::js_value(y));
        }
        map.insert("isMaximized".into(), json!(self.is_maximized));
        map.insert("isFullScreen".into(), json!(self.is_full_screen));
        if let Some(display) = self.display_bounds {
            map.insert("displayBounds".into(), bounds_json(display));
        }
        let path = user_data.join(FILE);
        fs::write(&path, format!("{}\n", Value::Object(map)))
            .with_context(|| format!("write {}", path.display()))?;
        log::info!(target: "window-state", "saved {self:?}");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use gpui::{Bounds, point, px, size};

    use super::{WindowState, persisted_maximized, read, restore};

    fn bounds(x: f32, y: f32, w: f32, h: f32) -> Bounds<gpui::Pixels> {
        Bounds {
            origin: point(px(x), px(y)),
            size: size(px(w), px(h)),
        }
    }

    fn raw(text: &str) -> crate::cindy_json::Fields {
        crate::cindy_json::object(text).unwrap()
    }

    #[test]
    fn restores_as_electron_window_state() {
        let screen = bounds(0.0, 0.0, 3200.0, 1350.0);
        let fallback = restore(None, &[screen], Some(screen));
        assert_eq!(
            (
                fallback.width,
                fallback.height,
                fallback.x,
                fallback.is_maximized
            ),
            (1280.0, 800.0, None, false)
        );
        let real = raw(
            r#"{"width":1280,"height":800,"isMaximized":true,"isFullScreen":false,"displayBounds":{"x":0,"y":0,"width":3200,"height":1350}}"#,
        );
        let state = restore(Some(&real), &[screen], Some(screen));
        assert!(state.is_maximized && state.x.is_none());
        let inside = raw(
            r#"{"x":100,"y":50,"width":900,"height":700,"displayBounds":{"x":0,"y":0,"width":3200,"height":1350}}"#,
        );
        assert_eq!(
            restore(Some(&inside), &[screen], Some(screen)).x,
            Some(100.0)
        );
        let off = raw(
            r#"{"x":4000,"y":0,"width":900,"height":700,"isMaximized":true,"displayBounds":{"x":3200,"y":0,"width":1920,"height":1080}}"#,
        );
        let reset = restore(Some(&off), &[screen], Some(screen));
        assert_eq!(
            (reset.x, reset.width, reset.is_maximized),
            (Some(0.0), 1280.0, false)
        );
        assert!(
            persisted_maximized(Some(&off)),
            "the raw flag still restores maximized"
        );
        let junk = raw(r#"{"x":"a","width":-5}"#);
        assert_eq!(restore(Some(&junk), &[screen], Some(screen)), fallback);
    }

    #[test]
    fn saves_bounds_only_in_the_normal_state() {
        let dir = tempfile::tempdir().unwrap();
        let screen = bounds(0.0, 0.0, 3200.0, 1350.0);
        let mut state: WindowState = restore(None, &[screen], Some(screen));
        state.update(
            Some(bounds(10.0, 20.0, 1000.0, 700.0)),
            false,
            false,
            Some(screen),
        );
        state.update(None, true, false, Some(screen));
        state.save(dir.path()).unwrap();
        let text = std::fs::read_to_string(dir.path().join("window-state.json")).unwrap();
        assert_eq!(
            text,
            "{\"width\":1000,\"height\":700,\"x\":10,\"y\":20,\"isMaximized\":true,\"isFullScreen\":false,\"displayBounds\":{\"x\":0,\"y\":0,\"width\":3200,\"height\":1350}}\n"
        );
        assert_eq!(read(dir.path()).unwrap()["width"].get(), "1000");
    }

    #[test]
    fn minimized_then_closed_keeps_the_normal_bounds() {
        let screen = bounds(0.0, 0.0, 3200.0, 1350.0);
        let mut state = restore(None, &[screen], Some(screen));
        state.update(
            Some(bounds(10.0, 20.0, 1000.0, 700.0)),
            false,
            false,
            Some(screen),
        );
        state.update(None, false, false, Some(screen));
        assert_eq!(
            (state.x, state.y, state.width),
            (Some(10.0), Some(20.0), 1000.0)
        );
    }

    #[test]
    fn an_overflowing_number_does_not_spoil_the_file() {
        let screen = bounds(0.0, 0.0, 3200.0, 1350.0);
        let fields = raw(
            r#"{"x":10,"y":20,"width":900,"height":700,"isMaximized":true,"displayBounds":{"x":0,"y":0,"width":1e400,"height":1350}}"#,
        );
        assert!(persisted_maximized(Some(&fields)));
        assert!(restore(Some(&fields), &[screen], Some(screen)).is_maximized);
    }
}
