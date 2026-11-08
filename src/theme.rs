mod css;
mod oklch;

use std::collections::HashMap;

use anyhow::{Context as _, Result, bail};
use gpui::{Rgba, WindowAppearance};
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Light,
    Dark,
}

/// Cindy's `Theme`: an id, a type and overrides of registry tokens.
#[derive(Clone, Debug, Deserialize)]
pub struct ThemeData {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: Kind,
    pub colors: HashMap<String, String>,
}

#[derive(Deserialize)]
struct RegistryEntry {
    id: String,
    light: Option<String>,
    dark: Option<String>,
}

const REGISTRY: &str = include_str!("../themes/registry.json");
const BUILTINS: [&str; 11] = [
    include_str!("../themes/builtin/cindy-light.json"),
    include_str!("../themes/builtin/cindy-dark.json"),
    include_str!("../themes/builtin/default-light.json"),
    include_str!("../themes/builtin/default-dark.json"),
    include_str!("../themes/builtin/atom-one-light.json"),
    include_str!("../themes/builtin/one-dark-pro.json"),
    include_str!("../themes/builtin/solarized-light.json"),
    include_str!("../themes/builtin/eclipse.json"),
    include_str!("../themes/builtin/monokai-pro.json"),
    include_str!("../themes/builtin/github-dark.json"),
    include_str!("../themes/builtin/material-ocean-hc.json"),
];

/// Cindy's `BUILTIN_FAMILIES`: id, name, light variant, dark variant.
const FAMILIES: [(&str, &str, Option<&str>, Option<&str>); 8] = [
    ("cindy", "Cindy", Some("cindy-light"), Some("cindy-dark")),
    (
        "default",
        "Classic",
        Some("default-light"),
        Some("default-dark"),
    ),
    (
        "atom-one",
        "Atom One",
        Some("atom-one-light"),
        Some("one-dark-pro"),
    ),
    (
        "solarized-light",
        "Solarized Light",
        Some("solarized-light"),
        None,
    ),
    ("eclipse", "Eclipse", None, Some("eclipse")),
    ("monokai-pro", "Monokai Pro", None, Some("monokai-pro")),
    ("github", "GitHub", None, Some("github-dark")),
    (
        "material-ocean-hc",
        "Material Ocean High Contrast",
        None,
        Some("material-ocean-hc"),
    ),
];
pub const DEFAULT_FAMILY: &str = "cindy";

/// The `theme` preference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Light,
    Dark,
    System,
}

impl Mode {
    pub fn parse(raw: Option<&str>) -> Self {
        match raw {
            Some("light") => Mode::Light,
            Some("dark") => Mode::Dark,
            Some("system") | None => Mode::System,
            Some(other) => {
                log::warn!(target: "theme", "ignored unknown theme preference {other:?}");
                Mode::System
            }
        }
    }

    fn kind(self, os: WindowAppearance) -> Kind {
        match (self, os) {
            (Mode::Light, _) => Kind::Light,
            (Mode::Dark, _) => Kind::Dark,
            (Mode::System, WindowAppearance::Dark | WindowAppearance::VibrantDark) => Kind::Dark,
            (Mode::System, _) => Kind::Light,
        }
    }
}

/// The registry and the built-in themes.
pub struct Themes {
    defaults: HashMap<String, (Option<String>, Option<String>)>,
    builtins: Vec<ThemeData>,
}

/// The theme on screen: its family, and whether it stood in for the other type.
pub struct Active {
    pub family: &'static str,
    pub theme: ThemeData,
    pub fallback: bool,
}

impl Themes {
    pub fn load() -> Result<Self> {
        let registry: Vec<RegistryEntry> =
            serde_json::from_str(REGISTRY).context("themes/registry.json")?;
        let builtins = BUILTINS
            .iter()
            .map(|raw| serde_json::from_str(raw).context("themes/builtin"))
            .collect::<Result<Vec<ThemeData>>>()?;
        Ok(Self {
            defaults: registry
                .into_iter()
                .map(|d| (d.id, (d.light, d.dark)))
                .collect(),
            builtins,
        })
    }

    fn builtin(&self, id: &str) -> Result<&ThemeData> {
        self.builtins
            .iter()
            .find(|t| t.id == id)
            .with_context(|| format!("no built-in theme {id}"))
    }

    /// The family for a saved id; an unknown one falls back to the default.
    pub fn family(family_id: Option<&str>) -> &'static str {
        let Some(id) = family_id else {
            return DEFAULT_FAMILY;
        };
        match FAMILIES.iter().find(|f| f.0 == id) {
            Some(found) => found.0,
            None => {
                log::warn!(target: "theme", "ignored unknown theme family {id:?}");
                DEFAULT_FAMILY
            }
        }
    }

    /// Cindy's `resolveFamilyVariant`.
    pub fn active(&self, family: &'static str, mode: Mode, os: WindowAppearance) -> Result<Active> {
        let &(id, _, light, dark) = FAMILIES
            .iter()
            .find(|f| f.0 == family)
            .with_context(|| format!("no theme family {family}"))?;
        let wanted = mode.kind(os);
        let (preferred, other) = match wanted {
            Kind::Light => (light, dark),
            Kind::Dark => (dark, light),
        };
        let (theme_id, fallback) = match (preferred, other) {
            (Some(theme), _) => (theme, false),
            (None, Some(theme)) => (theme, true),
            (None, None) => bail!("theme family {id} has no variants"),
        };
        Ok(Active {
            family: id,
            theme: self.builtin(theme_id)?.clone(),
            fallback,
        })
    }

    /// Cindy's `resolveThemeValue`.
    pub fn raw<'a>(&'a self, theme: &'a ThemeData, id: &str) -> Option<&'a str> {
        if let Some(value) = theme.colors.get(id) {
            return Some(value);
        }
        let (light, dark) = self.defaults.get(id)?;
        match theme.kind {
            Kind::Light => light.as_deref(),
            Kind::Dark => dark.as_deref().or(light.as_deref()),
        }
    }

    /// The token's value with every `var(--x)` replaced, as the CSS cascade would.
    pub fn value(&self, theme: &ThemeData, id: &str) -> Result<String> {
        self.expand(theme, id, 0)
    }

    fn expand(&self, theme: &ThemeData, id: &str, depth: usize) -> Result<String> {
        if depth > 16 {
            bail!("var() cycle at --{id}");
        }
        let mut text = self
            .raw(theme, id)
            .with_context(|| format!("no value for --{id}"))?
            .to_owned();
        while let Some(start) = text.find("var(--") {
            let end = start + text[start..].find(')').context("unclosed var()")?;
            let inner = self.expand(theme, &text[start + 6..end], depth + 1)?;
            text.replace_range(start..=end, &inner);
        }
        Ok(text)
    }

    /// The token as a color.
    pub fn color(&self, theme: &ThemeData, id: &str) -> Result<Rgba> {
        let value = self.value(theme, id)?;
        css::color(&value).with_context(|| format!("--{id} in {}", theme.id))
    }
}

/// The theme preferences and what they resolve to now.
pub struct Appearance {
    pub themes: Themes,
    pub mode: Mode,
    pub family: &'static str,
    os: WindowAppearance,
    gpui_agrees: bool,
    pub active: Active,
}

impl gpui::Global for Appearance {}

impl Appearance {
    pub fn new(
        mode: Mode,
        family_id: Option<&str>,
        initial: crate::os_appearance::Initial,
    ) -> Result<Self> {
        let os = initial.os;
        let themes = Themes::load()?;
        let family = Themes::family(family_id);
        let active = themes.active(family, mode, os)?;
        Ok(Self {
            themes,
            mode,
            family,
            os,
            gpui_agrees: initial.gpui_agrees,
            active,
        })
    }

    /// Re-resolves when the OS appearance changes; false when it did not.
    pub fn follow(&mut self, os: WindowAppearance) -> Result<bool> {
        if !self.gpui_agrees {
            self.gpui_agrees = os == self.os;
            log::debug!(target: "theme", "gpui reports {os:?} before its portal read; agrees {}", self.gpui_agrees);
            return Ok(false);
        }
        if os == self.os {
            return Ok(false);
        }
        self.os = os;
        self.active = self.themes.active(self.family, self.mode, os)?;
        Ok(true)
    }

    pub fn kind(&self) -> Kind {
        self.active.theme.kind
    }

    /// A token of the active theme as a color; tokens are checked by tests.
    pub fn token(&self, id: &str) -> gpui::Hsla {
        let theme = &self.active.theme;
        let color = self
            .themes
            .color(theme, id)
            .unwrap_or_else(|error| panic!("{error:#}"));
        color.into()
    }
}

#[cfg(test)]
mod tests;
