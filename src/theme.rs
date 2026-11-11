mod css;
pub mod ely;
pub mod local;
mod normalize;
pub mod oklch;
pub mod omarchy;
pub mod shell;

use std::{collections::HashMap, path::PathBuf};

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
    #[serde(default)]
    pub family: Option<String>,
    pub colors: HashMap<String, String>,
}

/// Cindy's `ThemeFamily`: at most one light and one dark variant.
pub struct Family {
    pub id: String,
    pub light: Option<ThemeData>,
    pub dark: Option<ThemeData>,
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

/// Where themes beyond the built-ins come from.
pub struct Sources {
    pub local_dir: PathBuf,
    pub omarchy_dir: PathBuf,
}

/// The registry and every family: built-in first, then local, as Cindy lists them.
pub struct Themes {
    defaults: HashMap<String, (Option<String>, Option<String>)>,
    families: Vec<Family>,
}

/// The theme on screen: its family, and whether it stood in for the other type.
pub struct Active {
    pub family: String,
    pub theme: ThemeData,
    pub fallback: bool,
}

impl Themes {
    /// The built-in themes, plus local and Omarchy ones from `sources`.
    pub fn load(sources: Option<&Sources>, last_omarchy: &mut Option<ThemeData>) -> Result<Self> {
        let registry: Vec<RegistryEntry> =
            serde_json::from_str(REGISTRY).context("themes/registry.json")?;
        let defaults = registry
            .into_iter()
            .map(|d| (d.id, (d.light, d.dark)))
            .collect();
        let builtins = BUILTINS
            .iter()
            .map(|raw| serde_json::from_str(raw).context("themes/builtin"))
            .collect::<Result<Vec<ThemeData>>>()?;
        let builtin = |id: Option<&str>| -> Result<Option<ThemeData>> {
            id.map(|id| {
                builtins
                    .iter()
                    .find(|t| t.id == id)
                    .cloned()
                    .with_context(|| format!("no built-in theme {id}"))
            })
            .transpose()
        };
        let mut families = FAMILIES
            .iter()
            .map(|&(id, _, light, dark)| {
                Ok(Family {
                    id: id.to_owned(),
                    light: builtin(light)?,
                    dark: builtin(dark)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        if let Some(sources) = sources {
            let mut extra = match local::load(&sources.local_dir, &defaults) {
                Ok(loaded) => loaded.themes,
                Err(error) => {
                    log::error!(target: "local-themes", "Failed to scan local themes: {error:#}");
                    Vec::new()
                }
            };
            match omarchy::read(&sources.omarchy_dir, &defaults) {
                Ok(Some(theme)) => {
                    *last_omarchy = Some(theme.clone());
                    extra.push(theme);
                }
                Ok(None) => {}
                Err(error) => {
                    log::warn!(target: "local-themes", "Failed to load Omarchy theme: {error:#}");
                    extra.extend(last_omarchy.clone());
                }
            }
            families.extend(local_families(extra));
        }
        Ok(Self { defaults, families })
    }

    /// The family for a saved id; an unknown one falls back to the default.
    pub fn family(&self, family_id: Option<&str>) -> String {
        let Some(id) = family_id else {
            return DEFAULT_FAMILY.to_owned();
        };
        if self.families.iter().any(|f| f.id == id) {
            return id.to_owned();
        }
        log::warn!(target: "theme", "ignored unknown theme family {id:?}");
        DEFAULT_FAMILY.to_owned()
    }

    /// Cindy's `resolveFamilyVariant`.
    pub fn active(&self, family: &str, mode: Mode, os: WindowAppearance) -> Result<Active> {
        let found = self
            .families
            .iter()
            .find(|f| f.id == family)
            .with_context(|| format!("no theme family {family}"))?;
        let (preferred, other) = match mode.kind(os) {
            Kind::Light => (&found.light, &found.dark),
            Kind::Dark => (&found.dark, &found.light),
        };
        let (theme, fallback) = match (preferred, other) {
            (Some(theme), _) => (theme, false),
            (None, Some(theme)) => (theme, true),
            (None, None) => bail!("theme family {family} has no variants"),
        };
        Ok(Active {
            family: found.id.clone(),
            theme: theme.clone(),
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

    /// A channel token as Tailwind's `hslVar` reads it: `hsl(var(--id))`.
    pub fn hsl(&self, theme: &ThemeData, id: &str) -> Result<Rgba> {
        let value = self.value(theme, id)?;
        css::color(&format!("hsl({value})")).with_context(|| format!("--{id} in {}", theme.id))
    }
}

/// Cindy's `buildLocalFamilies`: grouped by `family` key, first of each type kept.
fn local_families(themes: Vec<ThemeData>) -> Vec<Family> {
    let mut families: Vec<Family> = Vec::new();
    for theme in themes {
        let key = match &theme.family {
            Some(family) => format!("{family}{}", local::SUFFIX),
            None => theme.id.clone(),
        };
        let index = match families.iter().position(|f| f.id == key) {
            Some(index) => index,
            None => {
                families.push(Family {
                    id: key,
                    light: None,
                    dark: None,
                });
                families.len() - 1
            }
        };
        let slot = match theme.kind {
            Kind::Light => &mut families[index].light,
            Kind::Dark => &mut families[index].dark,
        };
        if slot.is_none() {
            *slot = Some(theme);
        }
    }
    families
}

/// The theme preferences and what they resolve to now.
pub struct Appearance {
    pub themes: Themes,
    pub mode: Mode,
    pub family: String,
    os: WindowAppearance,
    gpui_agrees: bool,
    sources: Option<Sources>,
    last_omarchy: Option<ThemeData>,
    pub active: Active,
}

impl gpui::Global for Appearance {}

impl Appearance {
    pub fn new(
        mode: Mode,
        family_id: Option<&str>,
        initial: crate::os_appearance::Initial,
        sources: Option<Sources>,
    ) -> Result<Self> {
        let os = initial.os;
        let mut last_omarchy = None;
        let themes = Themes::load(sources.as_ref(), &mut last_omarchy)?;
        let family = themes.family(family_id);
        let active = themes.active(&family, mode, os)?;
        Ok(Self {
            themes,
            mode,
            family,
            os,
            gpui_agrees: initial.gpui_agrees,
            sources,
            last_omarchy,
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
        self.active = self.themes.active(&self.family, self.mode, os)?;
        Ok(true)
    }

    pub fn kind(&self) -> Kind {
        self.active.theme.kind
    }

    /// Re-reads local and Omarchy themes, as Cindy does on `local-themes:changed`.
    pub fn reload(&mut self) -> Result<()> {
        self.themes = Themes::load(self.sources.as_ref(), &mut self.last_omarchy)?;
        self.family = self.themes.family(Some(&self.family));
        self.active = self.themes.active(&self.family, self.mode, self.os)?;
        Ok(())
    }

    /// A token of the active theme as a color; tokens are checked by tests.
    pub fn token(&self, id: &str) -> gpui::Hsla {
        let color = self.themes.color(&self.active.theme, id);
        color.unwrap_or_else(|error| panic!("{error:#}")).into()
    }

    /// A color token that may read `inherit`, which is no color of its own.
    pub fn inherited(&self, id: &str) -> Option<gpui::Hsla> {
        let value = self.themes.value(&self.active.theme, id);
        let value = value.unwrap_or_else(|error| panic!("{error:#}"));
        (value.trim() != "inherit").then(|| self.token(id))
    }

    /// A `box-shadow` token of the active theme, at the window zoom.
    pub fn shadow_token(&self, id: &str, cx: &gpui::App) -> gpui::BoxShadow {
        let zoom = cx.global::<crate::units::Zoom>().0;
        let value = self.themes.value(&self.active.theme, id);
        let shadow = value.and_then(|v| css::shadow(&v, zoom).with_context(|| format!("--{id}")));
        shadow.unwrap_or_else(|error| panic!("{error:#}"))
    }

    /// A channel token of the active theme, as `hsl(var(--id))`.
    pub fn hsl_token(&self, id: &str) -> gpui::Hsla {
        let color = self.themes.hsl(&self.active.theme, id);
        color.unwrap_or_else(|error| panic!("{error:#}")).into()
    }
}

#[cfg(test)]
mod tests;
