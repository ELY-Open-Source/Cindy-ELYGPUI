use std::{collections::HashMap, sync::LazyLock};

use serde::Deserialize;

use super::Kind;
use crate::theme_import::color::{
    Rgb, js_space, js_trim, parse_css_color, relative_luminance, to_hex,
};
use crate::theme_import::palette::derive_unchecked_switch_track;

type Colors = HashMap<String, String>;
type Defaults = HashMap<String, (Option<String>, Option<String>)>;

const PLACEHOLDER_KEYS: [&str; 4] = [
    "settings-input-placeholder",
    "chat-input-placeholder",
    "ask-input-placeholder",
    "plan-action-fb-placeholder",
];
const SWITCH_SURFACES: [&str; 5] = [
    "surface",
    "surface-elevated",
    "surface-card-ivory",
    "surface-hover",
    "surface-hover-soft",
];
const ACCENT_STATES: [(&str, &str, &str); 2] = [
    (
        "accent-cta-bg-pure",
        "button-cta-hover",
        "button-cta-pressed",
    ),
    (
        "confirm-btn-primary-bg",
        "confirm-btn-primary-hover",
        "confirm-btn-primary-pressed",
    ),
];
const LEGACY_SNAPSHOT_MIN_MATCHES: usize = 10;

#[derive(Deserialize)]
struct Slots {
    light: Vec<String>,
    dark: Vec<String>,
}

static LEGACY_LOGIN: LazyLock<HashMap<String, Slots>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../themes/login-legacy-defaults.json"))
        .expect("themes/login-legacy-defaults.json")
});

fn default_of<'a>(defaults: &'a Defaults, id: &str, kind: Kind) -> Option<&'a str> {
    let (light, dark) = defaults.get(id)?;
    match kind {
        Kind::Light => light.as_deref(),
        Kind::Dark => dark.as_deref(),
    }
}

fn parse(colors: &Colors, key: &str) -> Option<Rgb> {
    parse_css_color(colors.get(key)?)
}

/// Cindy's `normalizeUncheckedSwitchColors`.
fn switch_colors(mut colors: Colors, defaults: &Defaults) -> Colors {
    let explicit = |key: &str| {
        colors.get(key).is_some_and(|value| {
            [Kind::Light, Kind::Dark]
                .iter()
                .all(|k| default_of(defaults, key, *k) != Some(value))
        })
    };
    if explicit("switch-track-off") || explicit("switch-thumb-off") {
        return colors;
    }
    let (Some(preferred), Some(thumb)) = (
        parse(&colors, "text-secondary"),
        parse(&colors, "surface-on-card"),
    ) else {
        return colors;
    };
    let Some(mut against) = SWITCH_SURFACES
        .iter()
        .map(|k| parse(&colors, k))
        .collect::<Option<Vec<_>>>()
    else {
        return colors;
    };
    against.push(thumb);
    let (track, thumb) = match derive_unchecked_switch_track(preferred, &against) {
        Ok(track) => (track, thumb),
        Err(_) => match (parse(&colors, "border-default"), parse(&colors, "surface")) {
            (Some(track), Some(thumb)) => (track, thumb),
            _ => return colors,
        },
    };
    colors.insert("switch-track-off".into(), to_hex(track));
    colors.insert("switch-thumb-off".into(), to_hex(thumb));
    colors
}

/// Cindy's `resolveThemeRgb`: a value or default, following plain `var()` aliases.
fn resolve_rgb(
    colors: &Colors,
    defaults: &Defaults,
    id: &str,
    kind: Kind,
    depth: u8,
) -> Option<Rgb> {
    let value = colors
        .get(id)
        .map(String::as_str)
        .or_else(|| default_of(defaults, id, kind))?;
    let alias = js_trim(value)
        .strip_prefix("var(--")
        .and_then(|rest| rest.strip_suffix(')'))
        .filter(|name| {
            !name.is_empty()
                && name
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        });
    match alias {
        Some(alias) if depth < 8 => resolve_rgb(colors, defaults, alias, kind, depth + 1),
        _ => parse_css_color(value),
    }
}

/// Cindy's `bridgeLegacyAccentHover`.
fn bridge_accent_hover(mut colors: Colors, defaults: &Defaults, kind: Kind) -> Colors {
    if !colors.contains_key("accent-hover") {
        return colors;
    }
    let mut bridged = Vec::new();
    for (base, hover, pressed) in ACCENT_STATES {
        let has_hover = colors.contains_key(hover);
        if !has_hover {
            bridged.push((hover, "var(--accent-hover)".to_owned()));
        }
        if colors.contains_key(pressed) {
            continue;
        }
        let base_rgb = resolve_rgb(&colors, defaults, base, kind, 0);
        let hover_rgb = resolve_rgb(
            &colors,
            defaults,
            if has_hover { hover } else { "accent-hover" },
            kind,
            0,
        );
        let lighter = matches!((base_rgb, hover_rgb), (Some(b), Some(h)) if relative_luminance(h) > relative_luminance(b));
        let sign = if lighter { '+' } else { '-' };
        bridged.push((
            pressed,
            format!("oklch(from var(--{hover}) calc(l {sign} 0.04) c h)"),
        ));
    }
    for (key, value) in bridged {
        colors.insert(key.to_owned(), value);
    }
    colors
}

/// Cindy's `canonical` in `loginLegacyDefaults.ts`.
fn canonical(value: &str) -> String {
    let v: String = js_trim(value)
        .to_lowercase()
        .chars()
        .filter(|c| !js_space(*c))
        .collect();
    if !v.starts_with("rgb") {
        return v;
    }
    let chars: Vec<char> = v.chars().collect();
    let (mut out, mut i) = (String::new(), 0);
    while i < chars.len() {
        let start = i;
        let digits = |from: usize| {
            from + chars[from..]
                .iter()
                .take_while(|c| c.is_ascii_digit())
                .count()
        };
        let int_end = digits(i);
        let end = if chars.get(int_end) == Some(&'.') && digits(int_end + 1) > int_end + 1 {
            digits(int_end + 1)
        } else {
            int_end
        };
        if end > start {
            let number: f64 = chars[start..end]
                .iter()
                .collect::<String>()
                .parse()
                .expect("digits");
            out.push_str(&number.to_string());
            i = end;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// Cindy's `dropLegacyLoginDefaults`.
fn drop_legacy_login(mut colors: Colors, kind: Kind) -> Colors {
    let stale: Vec<String> = colors
        .iter()
        .filter(|(id, value)| {
            let value = canonical(value);
            LEGACY_LOGIN.get(id.as_str()).is_some_and(|slots| {
                let olds = match kind {
                    Kind::Light => &slots.light,
                    Kind::Dark => &slots.dark,
                };
                olds.iter().any(|old| canonical(old) == value)
            })
        })
        .map(|(id, _)| id.clone())
        .collect();
    if stale.len() >= LEGACY_SNAPSHOT_MIN_MATCHES {
        for id in stale {
            colors.remove(&id);
        }
    }
    colors
}

/// Cindy's `normalizeLocalThemeColors`.
pub fn normalize(mut colors: Colors, kind: Kind, defaults: &Defaults) -> Colors {
    if colors.get("settings-badge-connected").map(String::as_str) == Some("var(--text-primary)")
        && !colors.contains_key("settings-badge-connected-text")
    {
        colors.insert(
            "settings-badge-connected".into(),
            "var(--card-status-done)".into(),
        );
        colors.insert(
            "settings-badge-connected-text".into(),
            "var(--text-primary)".into(),
        );
    }
    if !colors.contains_key("text-placeholder")
        && let Some(seed) = PLACEHOLDER_KEYS
            .iter()
            .find_map(|k| colors.get(*k).cloned())
    {
        for key in PLACEHOLDER_KEYS {
            colors.remove(key);
        }
        colors.insert("text-placeholder".into(), seed);
    }
    let colors = bridge_accent_hover(switch_colors(colors, defaults), defaults, kind);
    drop_legacy_login(colors, kind)
}
