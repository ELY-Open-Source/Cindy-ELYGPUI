//! `useSidebarCardMode.ts` and `sidebarInstallVintage.ts`: the Pinned section's display mode
//! and the main list's, kept in memory once read or chosen, as each module keeps its own.

use std::cell::Cell;

use crate::{
    preferences::Preferences,
    rich_answer::js::{self, Js},
};
use gpui::App;

const PINNED_KEY: &str = "sidebar.cardMode";
const MAIN_KEY: &str = "sidebar.mainListMode";
const VINTAGE_KEY: &str = "cc-agent.sidebar.installVintage.v1";
const OWNER_CLAIM_KEY: &str = "cc-agent.sidebar.identityOwnerClaim.v1";
/// `LEGACY_USAGE_KEYS`: keys only an install from before the sidebar redesign leaves.
const LEGACY_USAGE_KEYS: [&str; 8] = [
    "cc-agent.lastChatView.v1",
    "cc-agent.lastDocView.v1",
    "cc-agent.sidebar.collapsedProjects",
    "cc-agent.sidebar.collapsedAutomationGroups",
    "cc-agent.sidebar.pinnedSessionOrder",
    "cc-agent.sidebar.filter.projects",
    "cc-agent.sidebar.filter.manualProjectOrder",
    PINNED_KEY,
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pinned {
    Text,
    Card,
    List,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Main {
    Text,
    List,
}

impl Pinned {
    pub const ALL: [Pinned; 3] = [Pinned::Text, Pinned::Card, Pinned::List];

    /// `parseMode`, with the old boolean strings.
    fn parse(raw: Option<&str>) -> Option<Self> {
        match raw? {
            "text" | "false" => Some(Pinned::Text),
            "card" | "true" => Some(Pinned::Card),
            "list" => Some(Pinned::List),
            _ => None,
        }
    }

    pub fn value(self) -> &'static str {
        match self {
            Pinned::Text => "text",
            Pinned::Card => "card",
            Pinned::List => "list",
        }
    }
}

impl Main {
    pub const ALL: [Main; 2] = [Main::Text, Main::List];

    fn parse(raw: Option<&str>) -> Option<Self> {
        match raw? {
            "text" => Some(Main::Text),
            "list" => Some(Main::List),
            _ => None,
        }
    }

    pub fn value(self) -> &'static str {
        match self {
            Main::Text => "text",
            Main::List => "list",
        }
    }
}

/// `memoryValue`, `mainMemoryValue` and `getSidebarInstallVintage`'s fixed answer.
pub struct ViewModes {
    pinned: Option<Pinned>,
    main: Cell<Option<Main>>,
    legacy: bool,
}

impl gpui::Global for ViewModes {}

fn text<'a>(prefs: &'a Preferences, key: &str) -> Option<&'a str> {
    prefs
        .text(key)
        .inspect_err(|error| log::warn!(target: "sessions", "{error:#}"))
        .ok()
        .flatten()
}

/// The vintage is fixed on the first read that needs it, as the sidebar's first frame does.
pub fn install(cx: &mut App) {
    let prefs = cx.global_mut::<Preferences>();
    let needs_vintage = Main::parse(text(prefs, MAIN_KEY)).is_none()
        && Pinned::parse(text(prefs, PINNED_KEY)).is_none();
    let legacy = needs_vintage && vintage(prefs) == "legacy";
    cx.set_global(ViewModes {
        pinned: None,
        main: Cell::new(None),
        legacy,
    });
}

/// `getSidebarInstallVintage`: the stored answer, else inferred from usage traces and stored.
fn vintage(prefs: &mut Preferences) -> &'static str {
    match text(prefs, VINTAGE_KEY) {
        Some("legacy") => return "legacy",
        Some("fresh") => return "fresh",
        _ => {}
    }
    let vintage = if has_legacy_usage_trace(prefs) {
        "legacy"
    } else {
        "fresh"
    };
    log::info!(target: "sessions", "sidebar install vintage: {vintage}");
    if let Err(error) = prefs.set(VINTAGE_KEY, vintage) {
        log::error!(target: "sessions", "install vintage not saved: {error:#}");
    }
    vintage
}

/// `hasLegacyUsageTrace`: a legacy key, bare or owner-scoped, or a claim that captured data.
fn has_legacy_usage_trace(prefs: &Preferences) -> bool {
    prefs.keys().any(|key| {
        LEGACY_USAGE_KEYS.iter().any(|base| {
            key == *base
                || key
                    .strip_prefix(base)
                    .is_some_and(|rest| rest.starts_with(".owner."))
        })
    }) || text(prefs, OWNER_CLAIM_KEY).is_some_and(claim_captured_legacy_data)
}

/// `claimEnvelopeCapturedLegacyData`, read through `JSON.parse`'s port.
fn claim_captured_legacy_data(raw: &str) -> bool {
    let Ok(parsed) = js::parse(raw) else {
        return false;
    };
    let Some(record) = parsed.obj() else {
        return false;
    };
    let owner = record.get("ownerId").and_then(Js::str);
    if record.get("version") != Some(&Js::Num(1.0)) || owner.is_none_or(str::is_empty) {
        return false;
    }
    let Some(legacy) = record.get("legacy") else {
        return true;
    };
    match legacy.obj().and_then(|legacy| legacy.get("values")) {
        Some(Js::Obj(values)) => values.values().any(|value| *value != Js::Null),
        Some(Js::Arr(values)) => values.iter().any(|value| *value != Js::Null),
        _ => false,
    }
}

/// `getSidebarViewMode`.
pub fn pinned(cx: &App) -> Pinned {
    let modes = cx.global::<ViewModes>();
    modes
        .pinned
        .or_else(|| Pinned::parse(text(cx.global::<Preferences>(), PINNED_KEY)))
        .unwrap_or(Pinned::Text)
}

/// `getSidebarMainViewMode`: set, else migrated from the pinned value, else by vintage.
pub fn main(cx: &App) -> Main {
    let modes = cx.global::<ViewModes>();
    if let Some(main) = modes.main.get() {
        return main;
    }
    let prefs = cx.global::<Preferences>();
    let found = Main::parse(text(prefs, MAIN_KEY)).or_else(|| {
        Pinned::parse(text(prefs, PINNED_KEY)).map(|pinned| match pinned {
            Pinned::Text => Main::Text,
            Pinned::Card | Pinned::List => Main::List,
        })
    });
    let found = found.or(modes.legacy.then_some(Main::Text));
    modes.main.set(found);
    found.unwrap_or(Main::List)
}

/// `useSidebarCardMode().setMode`: memory first, so a failed write still switches.
pub fn set_pinned(mode: Pinned, cx: &mut App) {
    log::info!(target: "sessions", "pinned view mode {}", mode.value());
    cx.global_mut::<ViewModes>().pinned = Some(mode);
    if let Err(error) = cx.global_mut::<Preferences>().set(PINNED_KEY, mode.value()) {
        log::error!(target: "sessions", "pinned view mode not saved: {error:#}");
    }
    cx.refresh_windows();
}

/// `useSidebarMainViewMode().setMode`.
pub fn set_main(mode: Main, cx: &mut App) {
    log::info!(target: "sessions", "main view mode {}", mode.value());
    cx.global::<ViewModes>().main.set(Some(mode));
    if let Err(error) = cx.global_mut::<Preferences>().set(MAIN_KEY, mode.value()) {
        log::error!(target: "sessions", "main view mode not saved: {error:#}");
    }
    cx.refresh_windows();
}

#[cfg(test)]
mod tests;
