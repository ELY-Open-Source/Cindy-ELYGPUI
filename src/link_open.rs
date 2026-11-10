//! `useLinkOpenPreference.ts` and `linkInteraction.ts`: where a message's link opens.

#![cfg_attr(
    not(test),
    expect(dead_code, reason = "9.5g's links read where they open")
)]

use std::cell::Cell;

use gpui::{App, Modifiers, MouseButton};

use crate::preferences::Preferences;

const LEGACY_KEY: &str = "chat.linkOpenPreference";

/// `LinkOpenPreference`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    Sidebar,
    External,
}

/// `LinkOpenKind`: a web page, or a page on this computer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Web,
    Local,
}

impl Target {
    /// `parsePreference`.
    fn parse(raw: Option<&str>) -> Option<Self> {
        match raw? {
            "sidebar" => Some(Target::Sidebar),
            "external" => Some(Target::External),
            _ => None,
        }
    }

    fn value(self) -> &'static str {
        match self {
            Target::Sidebar => "sidebar",
            Target::External => "external",
        }
    }
}

impl Kind {
    /// `STORAGE_KEYS`.
    fn key(self) -> &'static str {
        match self {
            Kind::Web => "chat.webLinkOpenPreference",
            Kind::Local => "chat.localLinkOpenPreference",
        }
    }

    /// `LINK_OPEN_DEFAULTS`.
    fn default(self) -> Target {
        match self {
            Kind::Web => Target::External,
            Kind::Local => Target::Sidebar,
        }
    }
}

/// `isLocalOpenUrl`: a file, or a page this computer serves to itself.
pub fn is_local(url: &str) -> bool {
    let Ok(parsed) = url::Url::parse(url) else {
        return false;
    };
    if parsed.scheme() == "file" {
        return true;
    }
    if !matches!(parsed.scheme(), "http" | "https") {
        return false;
    }
    match parsed.host() {
        Some(url::Host::Domain(name)) => {
            let name = name.strip_suffix('.').unwrap_or(name);
            name == "localhost" || name.ends_with(".localhost")
        }
        Some(url::Host::Ipv4(address)) => address.octets()[0] == 127,
        Some(url::Host::Ipv6(address)) => address == std::net::Ipv6Addr::LOCALHOST,
        None => false,
    }
}

/// `memoryValue` and `legacyMigrated`.
#[derive(Default)]
pub struct LinkOpen {
    web: Cell<Option<Target>>,
    local: Cell<Option<Target>>,
    migrated: Cell<bool>,
}

impl gpui::Global for LinkOpen {}

pub fn install(cx: &mut App) {
    cx.set_global(LinkOpen::default());
}

fn text(prefs: &Preferences, key: &str) -> Option<String> {
    let read = prefs.text(key);
    let read = read.inspect_err(|error| log::warn!(target: "link-open", "{error:#}"));
    read.ok().flatten().map(str::to_owned)
}

fn saved(result: anyhow::Result<()>, key: &str) {
    if let Err(error) = result {
        log::error!(target: "link-open", "{key} not saved: {error:#}");
    }
}

/// `migrateLegacyIfNeeded`: the old single choice, kept where it differs from a default.
fn migrate(prefs: &mut Preferences) -> anyhow::Result<()> {
    let Some(legacy) = Target::parse(prefs.text(LEGACY_KEY)?) else {
        return Ok(());
    };
    for kind in [Kind::Web, Kind::Local] {
        let unset = prefs.text(kind.key())?.is_none_or(str::is_empty);
        if unset && legacy != kind.default() {
            prefs.set(kind.key(), legacy.value())?;
        }
    }
    log::info!(target: "link-open", "the old link preference {} moved", legacy.value());
    prefs.remove(LEGACY_KEY)
}

fn memo(kind: Kind, cx: &App) -> &Cell<Option<Target>> {
    let link_open = cx.global::<LinkOpen>();
    match kind {
        Kind::Web => &link_open.web,
        Kind::Local => &link_open.local,
    }
}

/// `getLinkOpenPreference`.
pub fn preference(kind: Kind, cx: &mut App) -> Target {
    if !cx.global::<LinkOpen>().migrated.replace(true) {
        let moved = migrate(cx.global_mut::<Preferences>());
        // As Cindy's `try`: a failed step leaves the old key for a later run.
        if let Err(error) = moved {
            log::error!(target: "link-open", "the old link preference stays: {error:#}");
        }
    }
    if let Some(known) = memo(kind, cx).get() {
        return known;
    }
    let prefs = cx.global_mut::<Preferences>();
    let Some(parsed) = Target::parse(text(prefs, kind.key()).as_deref()) else {
        return kind.default();
    };
    if parsed == kind.default() {
        // A stored default would hold back a later change of defaults.
        saved(prefs.remove(kind.key()), kind.key());
    }
    memo(kind, cx).set(Some(parsed));
    parsed
}

/// `getLinkOpenPreferenceForUrl`.
pub fn for_url(url: &str, cx: &mut App) -> Target {
    let kind = match is_local(url) {
        true => Kind::Local,
        false => Kind::Web,
    };
    preference(kind, cx)
}

/// `isSeparateLinkActivation`: the click leaves the reading surface as it is.
pub fn separate(modifiers: Modifiers, button: MouseButton) -> bool {
    modifiers.shift || modifiers.platform || modifiers.control || button == MouseButton::Middle
}

#[cfg(test)]
mod tests;
