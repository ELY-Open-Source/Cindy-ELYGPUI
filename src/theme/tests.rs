use gpui::{Rgba, WindowAppearance};

use super::{Kind, Mode, Themes, css};

fn close(a: Rgba, b: Rgba) -> bool {
    [(a.r, b.r), (a.g, b.g), (a.b, b.b), (a.a, b.a)]
        .iter()
        .all(|(x, y)| (x - y).abs() < 0.002)
}

fn rgba(r: f32, g: f32, b: f32, a: f32) -> Rgba {
    Rgba { r, g, b, a }
}

#[test]
fn css_colors_evaluate() {
    assert!(close(
        css::color("#ff000080").unwrap(),
        rgba(1.0, 0.0, 0.0, 128.0 / 255.0)
    ));
    assert!(close(
        css::color("hsl(0 100% 50%)").unwrap(),
        rgba(1.0, 0.0, 0.0, 1.0)
    ));
    assert!(close(
        css::color("hsl(120 100% 25% / 50%)").unwrap(),
        rgba(0.0, 0.5, 0.0, 0.5)
    ));
    assert!(close(
        css::color("rgba(0, 0, 0, 0.5)").unwrap(),
        rgba(0.0, 0.0, 0.0, 0.5)
    ));
    let gray = css::color("color-mix(in srgb, #ffffff 50%, #000000)").unwrap();
    assert!(close(gray, rgba(0.5, 0.5, 0.5, 1.0)));
    let faded = css::color("color-mix(in srgb, #ff0000 40%, transparent)").unwrap();
    assert!(close(faded, rgba(1.0, 0.0, 0.0, 0.4)));
    assert!(css::color("0 84% 60%").is_err());
}

#[test]
fn filled_button_states_follow_oklch() {
    let lift = "calc(sign(max(0, l - 0.25)) * (l - 0.045) + (1 - sign(max(0, l - 0.25))) * 0.27)";
    let base = css::color("#3b82f6").unwrap();
    let hover = css::color(&format!("oklch(from #3b82f6 {lift} c h)")).unwrap();
    let (l, c, h) = super::oklch::from_srgb(base);
    let (hl, hc, hh) = super::oklch::from_srgb(hover);
    assert!((hl - (l - 0.045)).abs() < 0.002, "{l} -> {hl}");
    assert!((hc - c).abs() < 0.01 && (hh - h).abs() < 0.5);
    let dark = css::color(&format!("oklch(from #000000 {lift} c h)")).unwrap();
    assert!((super::oklch::from_srgb(dark).0 - 0.27).abs() < 0.002);
}

#[test]
fn every_builtin_token_resolves() {
    let themes = Themes::load(None).unwrap();
    let colorish = |v: &str| {
        v.starts_with('#')
            || ["rgb", "hsl", "color-mix", "oklch", "transparent"]
                .iter()
                .any(|p| v.starts_with(p))
    };
    let all = themes
        .families
        .iter()
        .flat_map(|f| f.light.iter().chain(f.dark.iter()));
    for theme in all {
        for id in themes.defaults.keys() {
            let Ok(value) = themes.value(theme, id) else {
                assert!(themes.raw(theme, id).is_none(), "{} --{id}", theme.id);
                continue;
            };
            if colorish(&value) {
                themes.color(theme, id).unwrap_or_else(|e| panic!("{e:#}"));
            }
        }
    }
}

#[test]
fn dark_theme_takes_light_default_when_dark_is_missing() {
    let themes = Themes::load(None).unwrap();
    let (id, (light, _)) = themes
        .defaults
        .iter()
        .find(|(_, (light, dark))| light.is_some() && dark.is_none())
        .expect("a root-only token");
    let dark = themes.families[0].dark.as_ref().unwrap();
    assert_eq!(dark.kind, Kind::Dark);
    assert_eq!(themes.raw(dark, id), light.as_deref());
}

#[test]
fn families_pick_variants_with_cindy_fallback() {
    let themes = Themes::load(None).unwrap();
    let dark_os = WindowAppearance::Dark;
    let cindy = themes
        .active(&themes.family(None), Mode::System, dark_os)
        .unwrap();
    assert_eq!(
        (
            cindy.family.as_str(),
            cindy.theme.id.as_str(),
            cindy.fallback
        ),
        ("cindy", "cindy-dark", false)
    );
    let eclipse = themes.active("eclipse", Mode::Light, dark_os).unwrap();
    assert_eq!(
        (eclipse.theme.id.as_str(), eclipse.fallback),
        ("eclipse", true)
    );
    let light = themes
        .active("atom-one", Mode::System, WindowAppearance::Light)
        .unwrap();
    assert_eq!(light.theme.id, "atom-one-light");
    assert_eq!(themes.family(Some("nope")), "cindy");
}

#[test]
fn every_family_maps_to_an_ely_palette() {
    use crate::os_appearance::Initial;
    for (family, ..) in super::FAMILIES {
        for os in [WindowAppearance::Light, WindowAppearance::Dark] {
            let initial = Initial {
                os,
                gpui_agrees: true,
            };
            let appearance =
                super::Appearance::new(Mode::System, Some(family), initial, None).unwrap();
            let palette = super::ely::palette(&appearance);
            assert_eq!(palette.bg, appearance.token("surface"), "{family} {os:?}");
            assert_eq!(
                palette.accent,
                appearance.token("accent-cta-bg"),
                "{family} {os:?}"
            );
        }
    }
}

/// Cindy's own `normalizeLocalThemeColors` wrote the expected side.
#[test]
fn normalize_matches_cindy_output() {
    use std::collections::HashMap;
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/local-themes");
    let read = |name: &str| -> Vec<serde_json::Value> {
        serde_json::from_str(&std::fs::read_to_string(dir.join(name)).unwrap()).unwrap()
    };
    let themes = Themes::load(None).unwrap();
    let (inputs, expected) = (
        read("normalize-input.json"),
        read("normalize-expected.json"),
    );
    assert_eq!(inputs.len(), 28);
    for (input, expected) in inputs.iter().zip(&expected) {
        let colors: HashMap<String, String> =
            serde_json::from_value(input["colors"].clone()).unwrap();
        let kind = if input["type"] == "dark" {
            Kind::Dark
        } else {
            Kind::Light
        };
        let want: HashMap<String, String> =
            serde_json::from_value(expected["colors"].clone()).unwrap();
        assert_eq!(
            super::normalize::normalize(colors, kind, &themes.defaults),
            want,
            "{}",
            input["name"]
        );
    }
}

#[test]
fn local_themes_load_group_and_report() {
    let home = tempfile::tempdir().unwrap();
    let dir = super::local::dir(home.path());
    std::fs::create_dir_all(&dir).unwrap();
    let theme = |id: &str, kind: &str, family: &str| {
        format!(
            r##"{{"id":"{id}","name":"N","type":"{kind}","family":"{family}","colors":{{"surface":"#123456","chat-input-placeholder":"#999999"}}}}"##
        )
    };
    for (file, body) in [
        ("b-dark.json", theme("pair-dark", "dark", " Pair ")),
        ("A-light.JSON", theme("pair-light", "light", "Pair")),
        ("c-solo.json", theme("solo", "light", "")),
        ("d-dup.json", theme("solo", "dark", "")),
        (
            "e-bad.json",
            r#"{"id":"x","name":"N","type":"dim","colors":{}}"#.to_owned(),
        ),
        (
            "f-color.json",
            r#"{"id":"y","name":"N","type":"dark","colors":{"surface":1}}"#.to_owned(),
        ),
        ("notes.txt", "ignored".to_owned()),
    ] {
        std::fs::write(dir.join(file), body).unwrap();
    }
    let themes = Themes::load(Some(&dir)).unwrap();
    let local: Vec<_> = themes.families[8..].iter().map(|f| f.id.as_str()).collect();
    assert_eq!(local, ["Pair-local", "solo-local"]);
    let pair = &themes.families[8];
    assert_eq!(pair.light.as_ref().unwrap().id, "pair-light-local");
    assert_eq!(pair.dark.as_ref().unwrap().id, "pair-dark-local");
    let solo = pair.light.as_ref().unwrap();
    assert_eq!(
        solo.colors.get("text-placeholder").map(String::as_str),
        Some("#999999")
    );
    assert!(!solo.colors.contains_key("chat-input-placeholder"));
    let loaded = super::local::load(&dir, &themes.defaults).unwrap();
    let errors: Vec<_> = loaded
        .diagnostics
        .iter()
        .map(|(f, e)| format!("{f}: {e}"))
        .collect();
    assert_eq!(
        errors,
        [
            "d-dup.json: Skipped duplicate local theme 'solo-local' from 'd-dup.json'.",
            "e-bad.json: Invalid 'type': expected 'light' or 'dark'.",
            "f-color.json: Invalid color 'surface': expected string value.",
        ]
    );
    assert_eq!(themes.family(Some("solo-local")), "solo-local");
}

#[test]
fn legacy_themes_folder_moves_once() {
    let home = tempfile::tempdir().unwrap();
    let old = home.path().join(".xdmaker/themes");
    std::fs::create_dir_all(&old).unwrap();
    std::fs::write(old.join("t.json"), "{}").unwrap();
    let dir = super::local::dir(home.path());
    assert!(dir.join("t.json").exists());
    assert!(!home.path().join(".xdmaker").exists());
}
