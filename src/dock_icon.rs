//! Cindy's `app-icon.ts`: the chosen icon in the macOS Dock for this run.

use anyhow::Result;
use gpui::App;

use crate::appearance_settings::AppIcon;
use crate::theme::{Appearance, Kind};

/// Dock artwork under `resources/`, light then dark, already on the Apple icon grid.
fn artwork(icon: AppIcon, dark: bool) -> &'static str {
    let (light, dark_art) = match icon {
        AppIcon::Default => ("icon-dock.png", "icon-dock.png"),
        AppIcon::Mark => ("app-icons/mark-light.png", "app-icons/mark-dark.png"),
        AppIcon::Wordmark => (
            "app-icons/wordmark-light.png",
            "app-icons/wordmark-dark.png",
        ),
        AppIcon::Cat => ("app-icons/cat.png", "app-icons/cat.png"),
    };
    if dark { dark_art } else { light }
}

/// The chosen icon and the artwork the Dock shows now.
pub struct Dock {
    icon: AppIcon,
    applied: Option<&'static str>,
}

impl gpui::Global for Dock {}

impl Dock {
    /// The artwork to show, or none when the Dock already shows the right one.
    fn next(&self, dark: bool, packaged: bool) -> Option<&'static str> {
        if self.icon == AppIcon::Default && packaged && self.applied.is_none() {
            return None;
        }
        let path = artwork(self.icon, dark);
        (self.applied != Some(path)).then_some(path)
    }
}

/// Restores the chosen icon at startup; unbundled builds show no Cindy icon otherwise.
pub fn install(icon: AppIcon, cx: &mut App) {
    cx.set_global(Dock {
        icon,
        applied: None,
    });
    apply(cx);
}

/// Shows the icon for the active theme. Finder and Launchpad keep the bundle icon.
pub fn apply(cx: &mut App) {
    if !cfg!(target_os = "macos") {
        return;
    }
    let dark = cx.global::<Appearance>().kind() == Kind::Dark;
    let dock = cx.global_mut::<Dock>();
    let Some(path) = dock.next(dark, packaged()) else {
        return;
    };
    match set(path) {
        Ok(()) => {
            log::info!(target: "app-icon", "Dock icon {path}");
            dock.applied = Some(path);
        }
        Err(error) => log::warn!(target: "app-icon", "Dock icon {path} not set: {error:#}"),
    }
}

/// Electron's `app.isPackaged`: running from an app bundle.
fn packaged() -> bool {
    std::env::current_exe().is_ok_and(|exe| exe.to_string_lossy().contains(".app/Contents/MacOS/"))
}

#[cfg(target_os = "macos")]
fn png(path: &str) -> &'static [u8] {
    macro_rules! art {
        ($($file:literal),*) => {
            match path {
                $($file => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/resources/", $file)),)*
                _ => unreachable!("no Dock artwork {path}"),
            }
        };
    }
    art!(
        "icon-dock.png",
        "app-icons/cat.png",
        "app-icons/mark-light.png",
        "app-icons/mark-dark.png",
        "app-icons/wordmark-light.png",
        "app-icons/wordmark-dark.png"
    )
}

/// Electron's `dock.setIcon`: the application icon image.
#[cfg(target_os = "macos")]
fn set(path: &str) -> Result<()> {
    use anyhow::Context as _;
    use objc2::AllocAnyThread as _;
    let mtm = objc2::MainThreadMarker::new().context("not on the main thread")?;
    let data = objc2_foundation::NSData::with_bytes(png(path));
    let image = objc2_app_kit::NSImage::initWithData(objc2_app_kit::NSImage::alloc(), &data)
        .context("not an image")?;
    let app = objc2_app_kit::NSApplication::sharedApplication(mtm);
    // SAFETY: a decoded image, set on the main thread.
    unsafe { app.setApplicationIconImage(Some(&image)) };
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn set(_: &str) -> Result<()> {
    unreachable!("the Dock is macOS's")
}

#[cfg(test)]
mod tests;
