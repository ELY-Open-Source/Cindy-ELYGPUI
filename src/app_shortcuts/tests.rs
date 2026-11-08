use std::fs;

use gpui::{KeyDownEvent, Keystroke};
use serde_json::{
    Value, json,
    value::{RawValue, to_raw_value},
};

use super::store::{FILE, PLATFORM};
use super::{
    AppShortcuts, DEFINITIONS, Scope, effective, format, keys::pressed, normalize_combo,
    normalize_overrides, overrides_value,
};
use crate::cindy_json::roundtrip;

fn fixture(name: &str) -> Value {
    let path = format!(
        "{}/tests/fixtures/app-shortcuts/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    roundtrip(&fs::read_to_string(path).unwrap()).unwrap()
}

fn scope(s: Scope) -> &'static str {
    match s {
        Scope::App => "app",
        Scope::WorkdirDoc => "workdir-doc",
        Scope::Browser => "browser",
        Scope::Composer => "composer",
        Scope::Global => "global",
    }
}

fn entries(list: Vec<(&'static str, Vec<super::Combo>)>) -> Value {
    list.into_iter()
        .map(|(id, combos)| json!([id, combos]))
        .collect()
}

#[test]
fn matches_cindy_on_every_platform() {
    let (input, expected) = (fixture("input.json"), fixture("expected.json"));
    let yield_to: Vec<_> = input["yieldTo"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| normalize_combo(&to_raw_value(c).unwrap()).unwrap())
        .collect();
    for want in expected.as_array().unwrap() {
        let platform = want["platform"].as_str().unwrap();
        let defs: Value = DEFINITIONS
            .iter()
            .map(|d| {
                json!({ "id": d.id, "scope": scope(d.scope), "rebindable": d.rebindable,
                    "hidden": d.hidden_in_settings, "available": d.available(platform),
                    "defaults": d.defaults(platform) })
            })
            .collect();
        assert_eq!(defs, want["definitions"], "{platform}");
        let cases = input["overrides"]
            .as_array()
            .unwrap()
            .iter()
            .zip(want["overrides"].as_array().unwrap());
        for (case, want) in cases {
            let text = match case.get("text") {
                Some(text) => text.as_str().unwrap().to_owned(),
                None => case["raw"].to_string(),
            };
            let raw: Box<RawValue> = serde_json::from_str(&text).unwrap();
            let normalized = normalize_overrides(Some(&raw), platform);
            let name = format!("{platform} {}", case["name"]);
            assert_eq!(overrides_value(&normalized), want["normalized"], "{name}");
            assert_eq!(
                entries(effective(&normalized, platform, &[])),
                want["effective"],
                "{name}"
            );
            assert_eq!(
                entries(effective(&normalized, platform, &yield_to)),
                want["yielded"],
                "{name}"
            );
        }
        let formats: Value = input["formats"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| {
                json!(format(
                    &normalize_combo(&to_raw_value(f).unwrap()).unwrap(),
                    platform
                ))
            })
            .collect();
        assert_eq!(formats, want["formats"], "{platform}");
        let all = [
            Scope::App,
            Scope::WorkdirDoc,
            Scope::Browser,
            Scope::Composer,
            Scope::Global,
        ];
        let overlaps: Value = all
            .iter()
            .flat_map(|x| {
                all.iter()
                    .map(move |y| json!([scope(*x), scope(*y), super::scopes_overlap(*x, *y)]))
            })
            .collect();
        assert_eq!(overlaps, want["overlaps"]);
    }
}

fn key(text: &'static str) -> Keystroke {
    Keystroke::parse(text).unwrap()
}

fn code(text: &'static str) -> Option<(String, bool, bool, bool, bool)> {
    pressed(&key(text)).map(|c| (c.code, c.meta, c.ctrl, c.alt, c.shift))
}

#[test]
fn reads_gpui_keys_as_cindy_codes() {
    let s = |c: &str| Some((c.to_owned(), false, false, false, false));
    assert_eq!(code("b"), s("KeyB"));
    assert_eq!(code("7"), s("Digit7"));
    assert_eq!(code("["), s("BracketLeft"));
    assert_eq!(code("pageup"), s("PageUp"));
    assert_eq!(code("left"), s("ArrowLeft"));
    assert_eq!(code("f5"), s("F5"));
    assert_eq!(code("add"), s("NumpadAdd"));
    assert_eq!(code("subtract"), s("NumpadSubtract"));
    assert_eq!(
        code("cmd-shift-{"),
        Some(("BracketLeft".into(), true, false, false, true))
    );
    assert_eq!(
        code("ctrl-+"),
        Some(("Equal".into(), false, true, false, true))
    );
    assert_eq!(
        code("ctrl-&"),
        Some(("Digit7".into(), false, true, false, true))
    );
    assert_eq!(
        code("ctrl-shift-tab"),
        Some(("Tab".into(), false, true, false, true))
    );
    assert_eq!(
        code("alt-shift-a"),
        Some(("KeyA".into(), false, false, true, true))
    );
    assert_eq!(
        code("ctrl-insert"),
        Some(("Insert".into(), false, true, false, false))
    );
    assert_eq!(code("f25"), None);
    assert_eq!(code("é"), None);
}

#[test]
fn loads_bindings_and_skips_held_keys() {
    let dir = tempfile::tempdir().unwrap();
    let defaults = AppShortcuts::load(dir.path());
    let ctrl_b = if PLATFORM == "darwin" {
        "cmd-b"
    } else {
        "ctrl-b"
    };
    let down = |text: &'static str, is_held| KeyDownEvent {
        keystroke: key(text),
        is_held,
        prefer_character_input: false,
    };
    assert!(defaults.matches("toggle-sidebar", &down(ctrl_b, false)));
    assert!(!defaults.matches("toggle-sidebar", &down(ctrl_b, true)));
    assert!(!defaults.matches("toggle-sidebar", &down("ctrl-shift-b", false)));

    let file = dir.path().join(FILE);
    let bound = json!({ "version": 1, "overrides": { "toggle-sidebar": { "code": "KeyJ", "ctrl": true, "huge": 1 } } });
    fs::write(
        &file,
        bound.to_string().replace("\"huge\":1", "\"huge\":1e400"),
    )
    .unwrap();
    let rebound = AppShortcuts::load(dir.path());
    assert!(rebound.matches("toggle-sidebar", &down("ctrl-j", false)));
    assert!(!rebound.matches("toggle-sidebar", &down(ctrl_b, false)));
    assert_eq!(
        rebound.display("toggle-sidebar"),
        format(&rebound.combos("toggle-sidebar")[0], PLATFORM)
    );

    fs::write(&file, "{").unwrap();
    assert!(AppShortcuts::load(dir.path()).matches("toggle-sidebar", &down(ctrl_b, false)));
    fs::write(&file, r#"{"overrides":{"toggle-sidebar":null}}"#).unwrap();
    let removed = AppShortcuts::load(dir.path());
    assert_eq!(removed.display("toggle-sidebar"), "");
    assert!(!removed.matches("toggle-sidebar", &down(ctrl_b, false)));
}
