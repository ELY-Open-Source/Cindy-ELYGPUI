//! Cindy's `shared/appShortcuts.ts`: the app shortcut registry, the user's bindings and matching.

#![cfg_attr(
    not(test),
    expect(dead_code, reason = "toggle-sidebar in 6.5a is the first caller")
)]

mod keys;
mod store;
#[cfg(test)]
mod tests;

use serde::Serialize;

use serde_json::value::RawValue;

use crate::cindy_json::{Fields, object, string, truthy};
pub use keys::format;
pub use store::AppShortcuts;

/// `AppShortcutCombo`: `code` is a W3C `KeyboardEvent.code`; `key` only labels it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Combo {
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub meta: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl Combo {
    /// `appShortcutCombosEqual`: the label does not count.
    fn same(&self, other: &Combo) -> bool {
        (&self.code, self.meta, self.ctrl, self.alt, self.shift)
            == (&other.code, other.meta, other.ctrl, other.alt, other.shift)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    App,
    WorkdirDoc,
    Browser,
    Composer,
    Global,
}

#[derive(Default)]
struct Mods {
    meta: bool,
    ctrl: bool,
    alt: bool,
    shift: bool,
}

const NONE: Mods = Mods {
    meta: false,
    ctrl: false,
    alt: false,
    shift: false,
};
const SHIFT: Mods = Mods {
    shift: true,
    ..NONE
};
const CTRL: Mods = Mods { ctrl: true, ..NONE };
const ALT: Mods = Mods { alt: true, ..NONE };

fn combo(code: &str, m: Mods) -> Combo {
    Combo {
        code: code.into(),
        key: None,
        meta: m.meta,
        ctrl: m.ctrl,
        alt: m.alt,
        shift: m.shift,
    }
}

/// `modCombo`: ⌘ on darwin, Ctrl elsewhere.
fn mod_combo(code: &str, platform: &str, shift: bool) -> Combo {
    let darwin = platform == "darwin";
    combo(
        code,
        Mods {
            meta: darwin,
            ctrl: !darwin,
            alt: false,
            shift,
        },
    )
}

pub struct Definition {
    pub id: &'static str,
    pub scope: Scope,
    pub rebindable: bool,
    pub hidden_in_settings: bool,
    yields_to_user_bindings: bool,
    platforms: Option<&'static [&'static str]>,
}

const fn def(id: &'static str, scope: Scope) -> Definition {
    Definition {
        id,
        scope,
        rebindable: true,
        hidden_in_settings: false,
        yields_to_user_bindings: false,
        platforms: None,
    }
}

const HIDDEN_YIELDING: Definition = Definition {
    hidden_in_settings: true,
    yields_to_user_bindings: true,
    ..def("", Scope::App)
};
const ZOOM: Option<&[&str]> = Some(&["win32", "linux"]);

/// `APP_SHORTCUT_DEFINITIONS`, in Cindy's order.
pub const DEFINITIONS: [Definition; 30] = [
    def("new-maker", Scope::App),
    def("toggle-sidebar", Scope::App),
    Definition {
        yields_to_user_bindings: true,
        ..def("session-history-back", Scope::App)
    },
    Definition {
        yields_to_user_bindings: true,
        ..def("session-history-forward", Scope::App)
    },
    Definition {
        platforms: Some(&["darwin", "win32"]),
        ..def("region-screenshot", Scope::Global)
    },
    def("cycle-permission-mode", Scope::Composer),
    Definition {
        rebindable: false,
        hidden_in_settings: true,
        ..def("close-tab-or-window", Scope::App)
    },
    def("right-tab-prev", Scope::App),
    def("right-tab-next", Scope::App),
    def("open-terminal", Scope::App),
    Definition {
        hidden_in_settings: true,
        platforms: Some(&["darwin"]),
        ..def("open-settings", Scope::App)
    },
    def("find-in-page", Scope::App),
    def("search-in-project", Scope::WorkdirDoc),
    Definition {
        hidden_in_settings: true,
        ..def("save-file", Scope::WorkdirDoc)
    },
    Definition {
        platforms: ZOOM,
        ..def("zoom-in", Scope::App)
    },
    Definition {
        platforms: ZOOM,
        ..def("zoom-out", Scope::App)
    },
    Definition {
        platforms: ZOOM,
        ..def("zoom-reset", Scope::App)
    },
    def("browser-focus-url", Scope::Browser),
    def("browser-back", Scope::Browser),
    def("browser-forward", Scope::Browser),
    def("browser-reload", Scope::Browser),
    Definition {
        id: "switch-session-1",
        ..HIDDEN_YIELDING
    },
    Definition {
        id: "switch-session-2",
        ..HIDDEN_YIELDING
    },
    Definition {
        id: "switch-session-3",
        ..HIDDEN_YIELDING
    },
    Definition {
        id: "switch-session-4",
        ..HIDDEN_YIELDING
    },
    Definition {
        id: "switch-session-5",
        ..HIDDEN_YIELDING
    },
    Definition {
        id: "switch-session-6",
        ..HIDDEN_YIELDING
    },
    Definition {
        id: "switch-session-7",
        ..HIDDEN_YIELDING
    },
    Definition {
        id: "switch-session-8",
        ..HIDDEN_YIELDING
    },
    Definition {
        id: "switch-session-9",
        ..HIDDEN_YIELDING
    },
];

impl Definition {
    /// `getDefaultCombos`.
    pub fn defaults(&self, platform: &str) -> Vec<Combo> {
        let darwin = platform == "darwin";
        let m = |code: &str| vec![mod_combo(code, platform, false)];
        match self.id {
            "new-maker" => m("KeyN"),
            "toggle-sidebar" => m("KeyB"),
            "session-history-back" => m("BracketLeft"),
            "session-history-forward" => m("BracketRight"),
            "region-screenshot" if darwin => vec![combo(
                "KeyA",
                Mods {
                    ctrl: true,
                    shift: true,
                    ..NONE
                },
            )],
            "region-screenshot" => vec![combo(
                "KeyA",
                Mods {
                    alt: true,
                    shift: true,
                    ..NONE
                },
            )],
            "cycle-permission-mode" => vec![combo("Tab", SHIFT)],
            "close-tab-or-window" => m("KeyW"),
            "right-tab-prev" | "right-tab-next" => {
                let next = self.id == "right-tab-next";
                let tab = combo(
                    "Tab",
                    Mods {
                        ctrl: true,
                        shift: !next,
                        ..NONE
                    },
                );
                let first = match (darwin, next) {
                    (true, false) => combo(
                        "BracketLeft",
                        Mods {
                            meta: true,
                            shift: true,
                            ..NONE
                        },
                    ),
                    (true, true) => combo(
                        "BracketRight",
                        Mods {
                            meta: true,
                            shift: true,
                            ..NONE
                        },
                    ),
                    (false, false) => combo("PageUp", CTRL),
                    (false, true) => combo("PageDown", CTRL),
                };
                vec![first, tab]
            }
            "open-terminal" => vec![combo("Backquote", CTRL)],
            "open-settings" => vec![combo("Comma", Mods { meta: true, ..NONE })],
            "find-in-page" => m("KeyF"),
            "search-in-project" => vec![mod_combo("KeyF", platform, true)],
            "save-file" => m("KeyS"),
            "zoom-in" => vec![
                combo("Equal", CTRL),
                combo(
                    "Equal",
                    Mods {
                        ctrl: true,
                        shift: true,
                        ..NONE
                    },
                ),
                combo("NumpadAdd", CTRL),
            ],
            "zoom-out" => vec![combo("Minus", CTRL), combo("NumpadSubtract", CTRL)],
            "zoom-reset" => vec![combo("Digit0", CTRL), combo("Numpad0", CTRL)],
            "browser-focus-url" => m("KeyL"),
            "browser-back" => vec![combo("ArrowLeft", ALT)],
            "browser-forward" => vec![combo("ArrowRight", ALT)],
            "browser-reload" => vec![mod_combo("KeyR", platform, false), combo("F5", NONE)],
            slot => {
                let n = slot
                    .strip_prefix("switch-session-")
                    .expect("a defined shortcut");
                m(&format!("Digit{n}"))
            }
        }
    }

    /// `isAppShortcutAvailableOnPlatform`.
    pub fn available(&self, platform: &str) -> bool {
        self.platforms.is_none_or(|p| p.contains(&platform))
    }
}

pub fn definition(id: &str) -> Option<&'static Definition> {
    DEFINITIONS.iter().find(|d| d.id == id)
}

/// `normalizeAppShortcutCombo`: a modifier alone or a blank code is no combo.
pub fn normalize_combo(raw: &RawValue) -> Option<Combo> {
    let raw = fields(raw)?;
    let f = |name: &str| raw.get(name).map(AsRef::as_ref);
    let code = string(f("code")).filter(|c| !crate::theme_import::color::js_trim(c).is_empty())?;
    if keys::MODIFIER_CODES.contains(&code.as_str()) {
        return None;
    }
    Some(Combo {
        code,
        key: string(f("key")),
        meta: truthy(f("meta")),
        ctrl: truthy(f("ctrl")),
        alt: truthy(f("alt")),
        shift: truthy(f("shift")),
    })
}

/// An object's fields as `JSON.parse` gives them; none for anything else.
pub fn fields(raw: &RawValue) -> Option<Fields> {
    raw.get()
        .starts_with('{')
        .then(|| object(raw.get()).ok())
        .flatten()
}

/// The user's bindings by id; none removes a binding.
pub type Overrides = Vec<(&'static str, Option<Combo>)>;

/// `normalizeAppShortcutOverrides`: unknown, fixed, unavailable or reserved-key entries go.
pub fn normalize_overrides(raw: Option<&RawValue>, platform: &str) -> Overrides {
    let Some(raw) = raw.and_then(fields) else {
        return Vec::new();
    };
    let reserved = |c: &Combo| {
        DEFINITIONS
            .iter()
            .filter(|d| !d.rebindable && d.available(platform))
            .any(|d| d.defaults(platform).iter().any(|r| r.same(c)))
    };
    let mut out = Vec::new();
    for (key, value) in &raw {
        let Some(def) = definition(key).filter(|d| d.rebindable && d.available(platform)) else {
            continue;
        };
        if value.get() == "null" {
            out.push((def.id, None));
        } else if let Some(c) = normalize_combo(value).filter(|c| !reserved(c)) {
            out.push((def.id, Some(c)));
        }
    }
    out
}

/// `appShortcutScopesOverlap`.
pub fn scopes_overlap(a: Scope, b: Scope) -> bool {
    use Scope::*;
    matches!((a, b), (App | Global, _) | (_, App | Global))
        || a == b
        || matches!(
            (a, b),
            (Browser | Composer, WorkdirDoc) | (WorkdirDoc, Browser | Composer)
        )
}

/// `getEffectiveAppShortcuts`: defaults merged with the user's bindings, in registry order.
pub fn effective(
    overrides: &Overrides,
    platform: &str,
    yield_to: &[Combo],
) -> Vec<(&'static str, Vec<Combo>)> {
    let get = |id: &str| overrides.iter().find(|(i, _)| *i == id).map(|(_, c)| c);
    let bound: Vec<(Scope, &Combo)> = DEFINITIONS
        .iter()
        .filter(|d| d.available(platform))
        .filter_map(|d| Some((d.scope, get(d.id)?.as_ref()?)))
        .collect();
    DEFINITIONS
        .iter()
        .filter(|d| d.available(platform))
        .map(|d| {
            let combos = match get(d.id) {
                Some(None) => Vec::new(),
                Some(Some(c)) => vec![c.clone()],
                None => {
                    let mut combos = d.defaults(platform);
                    if d.yields_to_user_bindings {
                        combos.retain(|c| {
                            !bound
                                .iter()
                                .any(|(s, o)| scopes_overlap(d.scope, *s) && o.same(c))
                                && !yield_to.iter().any(|y| y.same(c))
                        });
                    }
                    combos
                }
            };
            (d.id, combos)
        })
        .collect()
}

/// The overrides as JSON, for the golden comparison.
#[cfg(test)]
fn overrides_value(overrides: &Overrides) -> serde_json::Value {
    let map: serde_json::Map<String, serde_json::Value> = overrides
        .iter()
        .map(|(id, c)| {
            (
                id.to_string(),
                serde_json::to_value(c).expect("a combo serializes"),
            )
        })
        .collect();
    serde_json::Value::Object(map)
}
