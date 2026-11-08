use super::{Dock, artwork};
use crate::appearance_settings::AppIcon;

fn dock(icon: AppIcon, applied: Option<&'static str>) -> Dock {
    Dock { icon, applied }
}

#[test]
fn picks_the_artwork_for_the_theme() {
    let light = dock(AppIcon::Mark, None).next(false, false);
    assert_eq!(light, Some("app-icons/mark-light.png"));
    let dark = dock(AppIcon::Wordmark, None).next(true, false);
    assert_eq!(dark, Some("app-icons/wordmark-dark.png"));
    assert_eq!(artwork(AppIcon::Cat, true), artwork(AppIcon::Cat, false));
}

#[test]
fn a_bundle_keeps_its_own_icon_until_another_replaced_it() {
    assert_eq!(dock(AppIcon::Default, None).next(true, true), None);
    assert_eq!(
        dock(AppIcon::Default, None).next(true, false),
        Some("icon-dock.png"),
        "an unbundled run shows no Cindy icon"
    );
    let replaced = dock(AppIcon::Default, Some("app-icons/cat.png"));
    assert_eq!(replaced.next(true, true), Some("icon-dock.png"));
}

#[test]
fn the_same_artwork_is_not_set_twice() {
    let shown = dock(AppIcon::Mark, Some("app-icons/mark-dark.png"));
    assert_eq!(shown.next(true, false), None);
    assert_eq!(shown.next(false, false), Some("app-icons/mark-light.png"));
}

#[cfg(target_os = "macos")]
#[test]
fn every_artwork_is_embedded() {
    let icons = [
        AppIcon::Default,
        AppIcon::Cat,
        AppIcon::Wordmark,
        AppIcon::Mark,
    ];
    for icon in icons {
        for dark in [false, true] {
            let png = super::png(artwork(icon, dark));
            assert!(png.starts_with(b"\x89PNG"), "{icon:?} {dark}");
        }
    }
}
