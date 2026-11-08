//! Cindy's `vibrancyConfig.test.ts` cases, and its glass CSS.

use gpui::{Rgba, TestAppContext, WindowAppearance, rgb, rgba};

use super::{Backdrop, Config, Knobs, Material, Platform, resolve};
use crate::theme::{Appearance, Mode};

const WIN11: Platform = Platform::Windows(Some(22631));
const MATERIALS: [&str; 7] = [
    "sidebar",
    "hud",
    "under-window",
    "fullscreen-ui",
    "popover",
    "menu",
    "none",
];

fn opaque(dark: bool) -> Rgba {
    if dark { rgb(0x1f1f1e) } else { rgb(0xf8f8f6) }
}

fn knobs(vibrancy: Option<&str>, backdrop: Option<&str>) -> Knobs {
    Knobs {
        vibrancy: vibrancy.map(str::to_owned),
        backdrop: backdrop.map(str::to_owned),
    }
}

fn config(vibrancy: Option<&str>, background: Rgba, material: Option<Backdrop>) -> Config {
    Config {
        vibrancy: vibrancy.map(str::to_owned),
        background,
        material,
    }
}

#[test]
fn wallpaper_coverage_disables_native_material() {
    for platform in [Platform::Mac, WIN11, Platform::Linux] {
        for dark in [false, true] {
            let covered = resolve("cindy", dark, platform, true, &Knobs::default());
            assert_eq!(covered.vibrancy, None);
            assert_eq!(covered.background, opaque(dark));
            if platform == WIN11 {
                assert_eq!(covered.material, Some(Backdrop::None));
            }
            let other = resolve("default", dark, platform, false, &Knobs::default());
            assert_eq!(covered, other, "{platform:?}");
        }
    }
}

#[test]
fn macos_shows_hud_or_the_knob_only_for_cindy() {
    let clear = rgba(0);
    let cindy = resolve("cindy", false, Platform::Mac, false, &Knobs::default());
    assert_eq!(cindy, config(Some("hud"), clear, None));
    let empty = resolve("cindy", true, Platform::Mac, false, &knobs(Some(""), None));
    assert_eq!(
        empty.vibrancy.as_deref(),
        Some("hud"),
        "`||` takes an empty knob as unset"
    );
    for material in MATERIALS {
        let set = knobs(Some(material), None);
        for dark in [false, true] {
            let shown = (material != "none").then_some(material);
            let cindy = resolve("cindy", dark, Platform::Mac, false, &set);
            assert_eq!(cindy, config(shown, clear, None));
        }
        let default = resolve("default", false, Platform::Mac, false, &set);
        assert_eq!(default, config(None, opaque(false), None));
        let atom = resolve("atom-one", true, Platform::Mac, false, &set);
        assert_eq!(atom, config(None, opaque(true), None));
    }
}

#[test]
fn windows_11_shows_acrylic_with_the_sidebar_backing() {
    let light = resolve("cindy", false, WIN11, false, &Knobs::default());
    let backing = rgba(0xeeeee9d9);
    assert_eq!(light, config(None, backing, Some(Backdrop::Acrylic)));
    let dark = resolve("cindy", true, WIN11, false, &Knobs::default());
    assert_eq!(
        dark,
        config(None, rgba(0x050505d9), Some(Backdrop::Acrylic))
    );
    for (knob, material) in [("mica", Backdrop::Mica), ("tabbed", Backdrop::Tabbed)] {
        let set = knobs(None, Some(knob));
        let resolved = resolve("cindy", true, WIN11, false, &set);
        assert_eq!(resolved, config(None, rgba(0), Some(material)));
    }
    let bad = resolve("cindy", true, WIN11, false, &knobs(None, Some("glass")));
    assert_eq!(bad.material, Some(Backdrop::Acrylic));
}

#[test]
fn windows_10_and_other_families_stay_opaque() {
    for platform in [
        Platform::Windows(Some(19045)),
        Platform::Windows(None),
        WIN11,
    ] {
        let family = if platform == WIN11 {
            "default"
        } else {
            "cindy"
        };
        let resolved = resolve(family, true, platform, false, &Knobs::default());
        assert_eq!(resolved, config(None, opaque(true), Some(Backdrop::None)));
    }
    let edge = resolve(
        "cindy",
        false,
        Platform::Windows(Some(22000)),
        false,
        &Knobs::default(),
    );
    assert_eq!(edge.material, Some(Backdrop::Acrylic));
    let linux = resolve("cindy", false, Platform::Linux, false, &Knobs::default());
    assert_eq!(linux, config(None, opaque(false), None));
}

fn glass(cx: &mut TestAppContext, family: &str, platform: Platform) {
    cx.update(|cx| {
        let initial = crate::os_appearance::Initial {
            os: WindowAppearance::Dark,
            gpui_agrees: true,
        };
        let appearance = Appearance::new(Mode::System, Some(family), initial, None).unwrap();
        cx.set_global(appearance);
        cx.set_global(Material {
            platform,
            knobs: Knobs::default(),
        });
        crate::wallpaper::none_for_tests(cx);
    });
}

#[gpui::test]
fn cindy_glass_follows_the_platform(cx: &mut TestAppContext) {
    glass(cx, "cindy", Platform::Linux);
    cx.update(|cx| {
        let appearance = cx.global::<Appearance>();
        assert_eq!(super::root(cx), opaque(true).into(), "the backing shows");
        let translucent = appearance.token("surface-translucent-sidebar");
        assert_eq!(super::sidebar(false, cx), translucent);
        let overlay = appearance.token("surface-translucent-overlay");
        assert_eq!(super::overlay(cx), overlay);
    });
    glass(cx, "cindy", WIN11);
    cx.update(|cx| {
        let appearance = cx.global::<Appearance>();
        assert_eq!(super::sidebar(false, cx), gpui::transparent_black());
        let translucent = appearance.token("surface-translucent-sidebar");
        assert_eq!(
            super::sidebar(true, cx),
            translucent,
            "the drawer covers the app"
        );
        assert_eq!(super::overlay(cx), appearance.token("cmd-palette-bg"));
        assert_eq!(super::root(cx), rgba(0x050505d9).into());
    });
}

#[gpui::test]
fn other_families_keep_their_surfaces(cx: &mut TestAppContext) {
    glass(cx, "default", Platform::Mac);
    cx.update(|cx| {
        let appearance = cx.global::<Appearance>();
        assert_eq!(super::root(cx), appearance.hsl_token("content-area"));
        assert_eq!(super::sidebar(true, cx), appearance.hsl_token("sidebar"));
        assert_eq!(super::overlay(cx), appearance.token("cmd-palette-bg"));
    });
}
