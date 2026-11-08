//! Cindy's `vibrancyConfig.ts` and its glass CSS: only the Cindy family shows the desktop.

mod native;

use gpui::{App, Hsla, Rgba, Window, rgb, rgba};

use crate::theme::{Appearance, Kind};

/// `WindowsBackdropMaterial`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backdrop {
    Acrylic,
    Mica,
    Tabbed,
    None,
}

impl Backdrop {
    fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "acrylic" => Self::Acrylic,
            "mica" => Self::Mica,
            "tabbed" => Self::Tabbed,
            "none" => Self::None,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Mac,
    /// The build from `process.getSystemVersion()`, when it reads.
    Windows(Option<u32>),
    Linux,
}

/// The developer knobs `XDT_VIBRANCY_MATERIAL` and `XDT_BACKDROP_MATERIAL`.
#[derive(Clone, Debug, Default)]
pub struct Knobs {
    pub vibrancy: Option<String>,
    pub backdrop: Option<String>,
}

/// `VibrancyConfig`: the macOS material, the window backing and the Windows material.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub vibrancy: Option<String>,
    pub background: Rgba,
    pub material: Option<Backdrop>,
}

fn backdrop(knobs: &Knobs) -> Backdrop {
    let Some(raw) = knobs.backdrop.as_deref().filter(|raw| !raw.is_empty()) else {
        return Backdrop::Acrylic;
    };
    Backdrop::parse(raw).unwrap_or_else(|| {
        log::warn!(target: "vibrancyConfig", "Invalid XDT_BACKDROP_MATERIAL '{raw}', falling back to acrylic.");
        Backdrop::Acrylic
    })
}

/// `resolveVibrancyConfig`.
pub fn resolve(
    family: &str,
    dark: bool,
    platform: Platform,
    covered: bool,
    knobs: &Knobs,
) -> Config {
    let cindy = family == "cindy" && !covered;
    let opaque = if dark { rgb(0x1f1f1e) } else { rgb(0xf8f8f6) };
    let clear = rgba(0x00000000);
    match platform {
        Platform::Mac => {
            let material = match knobs.vibrancy.as_deref() {
                Some(raw) if !raw.is_empty() => raw,
                _ => "hud",
            };
            Config {
                vibrancy: (cindy && material != "none").then(|| material.to_owned()),
                background: if cindy { clear } else { opaque },
                material: None,
            }
        }
        Platform::Windows(build) if cindy && build.is_some_and(|b| b >= 22000) => {
            let material = backdrop(knobs);
            let background = match (material, dark) {
                (Backdrop::Acrylic, true) => rgba(0x050505d9),
                (Backdrop::Acrylic, false) => rgba(0xeeeee9d9),
                _ => clear,
            };
            Config {
                vibrancy: None,
                background,
                material: Some(material),
            }
        }
        Platform::Windows(_) => Config {
            vibrancy: None,
            background: opaque,
            material: Some(Backdrop::None),
        },
        Platform::Linux => Config {
            vibrancy: None,
            background: opaque,
            material: None,
        },
    }
}

/// The platform, the knobs and the material each window shows.
pub struct Material {
    platform: Platform,
    knobs: Knobs,
}

impl gpui::Global for Material {}

impl Material {
    pub fn load() -> Self {
        let knob = |name| std::env::var(name).ok();
        let knobs = Knobs {
            vibrancy: knob("XDT_VIBRANCY_MATERIAL"),
            backdrop: knob("XDT_BACKDROP_MATERIAL"),
        };
        let platform = native::platform();
        log::info!(target: "vibrancyConfig", "platform {platform:?}, knobs {knobs:?}");
        Self { platform, knobs }
    }

    pub fn config(&self, appearance: &Appearance) -> Config {
        let dark = appearance.kind() == Kind::Dark;
        resolve(&appearance.family, dark, self.platform, false, &self.knobs)
    }
}

/// `applyWindowVibrancy` for one window.
pub fn apply(window: &mut Window, cx: &App) {
    let config = cx.global::<Material>().config(cx.global::<Appearance>());
    native::apply(window, &config);
}

/// The CSS that sets `[data-theme='cindy-light']` or `cindy-dark` apart.
pub fn cindy_theme(appearance: &Appearance) -> bool {
    matches!(
        appearance.active.theme.id.as_str(),
        "cindy-light" | "cindy-dark"
    )
}

/// The window root: the backing under Cindy's own themes, `bg-content-area` elsewhere.
pub fn root(cx: &App) -> Hsla {
    let appearance = cx.global::<Appearance>();
    if cindy_theme(appearance) {
        cx.global::<Material>().config(appearance).background.into()
    } else {
        appearance.hsl_token("content-area")
    }
}

/// `aside.bg-sidebar`: glass under Cindy's own themes; acrylic already tints the docked one.
pub fn sidebar(peek: bool, cx: &App) -> Hsla {
    let appearance = cx.global::<Appearance>();
    if !cindy_theme(appearance) {
        return appearance.hsl_token("sidebar");
    }
    let material = cx.global::<Material>().config(appearance).material;
    if !peek && material == Some(Backdrop::Acrylic) {
        return gpui::transparent_black();
    }
    appearance.token("surface-translucent-sidebar")
}

/// `.bg-[var(--cmd-palette-bg)]`: the overlay glass, except on Windows.
pub fn overlay(cx: &App) -> Hsla {
    let appearance = cx.global::<Appearance>();
    let windows = matches!(cx.global::<Material>().platform, Platform::Windows(_));
    if cindy_theme(appearance) && !windows {
        appearance.token("surface-translucent-overlay")
    } else {
        appearance.token("cmd-palette-bg")
    }
}

#[cfg(test)]
mod tests;
