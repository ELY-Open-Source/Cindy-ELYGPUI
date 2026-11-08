//! Electron's `setVibrancy`, `setBackgroundMaterial` and `setBackgroundColor` per platform.

use gpui::{Window, WindowBackgroundAppearance};
use raw_window_handle::HasWindowHandle;

use super::{Config, Platform};

pub fn platform() -> Platform {
    if cfg!(target_os = "macos") {
        Platform::Mac
    } else if cfg!(target_os = "windows") {
        Platform::Windows(windows_build())
    } else {
        Platform::Linux
    }
}

#[cfg(target_os = "windows")]
fn windows_build() -> Option<u32> {
    use windows::Win32::System::SystemInformation::OSVERSIONINFOW;
    let mut version = OSVERSIONINFOW {
        dwOSVersionInfoSize: size_of::<OSVERSIONINFOW>() as u32,
        ..Default::default()
    };
    // SAFETY: RtlGetVersion fills the struct it is given.
    let status = unsafe { windows::Wdk::System::SystemServices::RtlGetVersion(&mut version) };
    status.is_ok().then_some(version.dwBuildNumber)
}

#[cfg(not(target_os = "windows"))]
fn windows_build() -> Option<u32> {
    unreachable!("only Windows has a build")
}

/// gpui paints the backing; the native side only has to let it through.
fn appearance(config: &Config) -> WindowBackgroundAppearance {
    use super::Backdrop;
    match config.material {
        Some(Backdrop::Acrylic | Backdrop::Mica) => WindowBackgroundAppearance::MicaBackdrop,
        Some(Backdrop::Tabbed) => WindowBackgroundAppearance::MicaAltBackdrop,
        Some(Backdrop::None) | None if config.background.a < 1.0 => {
            WindowBackgroundAppearance::Transparent
        }
        Some(Backdrop::None) | None => WindowBackgroundAppearance::Opaque,
    }
}

pub fn apply(window: &mut Window, config: &Config) {
    let appearance = appearance(config);
    window.set_background_appearance(appearance);
    log::info!(target: "vibrancyConfig", "window material {appearance:?}, vibrancy {:?}, backing {:?}", config.vibrancy, config.background);
    if let Err(error) = platform_apply(window, config) {
        log::warn!(target: "vibrancyConfig", "window material not applied: {error:#}");
    }
}

/// Electron's `SetBackgroundMaterial`, from Windows 11 22H2 on; gpui only clears to transparent.
#[cfg(target_os = "windows")]
fn platform_apply(window: &impl HasWindowHandle, config: &Config) -> anyhow::Result<()> {
    use super::Backdrop;
    use anyhow::Context as _;
    use raw_window_handle::RawWindowHandle;
    use windows::Win32::{
        Foundation::HWND,
        Graphics::Dwm::{
            DWMSBT_MAINWINDOW, DWMSBT_NONE, DWMSBT_TABBEDWINDOW, DWMSBT_TRANSIENTWINDOW,
            DWMWA_SYSTEMBACKDROP_TYPE, DwmExtendFrameIntoClientArea, DwmSetWindowAttribute,
        },
        UI::Controls::MARGINS,
    };
    let Some(material) = config.material else {
        return Ok(());
    };
    if !matches!(platform(), Platform::Windows(Some(build)) if build >= 22621) {
        return Ok(());
    }
    let kind = match material {
        Backdrop::None => DWMSBT_NONE,
        Backdrop::Acrylic => DWMSBT_TRANSIENTWINDOW,
        Backdrop::Mica => DWMSBT_MAINWINDOW,
        Backdrop::Tabbed => DWMSBT_TABBEDWINDOW,
    };
    let RawWindowHandle::Win32(handle) = window.window_handle()?.as_raw() else {
        anyhow::bail!("not a Win32 window");
    };
    let hwnd = HWND(handle.hwnd.get() as *mut _);
    let edge = if material == Backdrop::None { 0 } else { -1 };
    let margins = MARGINS {
        cxLeftWidth: edge,
        cxRightWidth: edge,
        cyTopHeight: edge,
        cyBottomHeight: edge,
    };
    // SAFETY: a live window, its margins and one DWM_SYSTEMBACKDROP_TYPE value.
    unsafe {
        if let Err(error) = DwmExtendFrameIntoClientArea(hwnd, &margins) {
            log::warn!(target: "vibrancyConfig", "Failed to extend frame into client area: {error}");
        }
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            std::ptr::from_ref(&kind).cast(),
            size_of_val(&kind) as u32,
        )
    }
    .with_context(|| format!("Failed to set background material to {material:?}"))
}

/// Electron's `setVibrancy`: an effect view behind the content, following window activity.
#[cfg(target_os = "macos")]
fn platform_apply(window: &impl HasWindowHandle, config: &Config) -> anyhow::Result<()> {
    use anyhow::Context as _;
    use objc2::{MainThreadOnly as _, rc::Retained};
    use objc2_app_kit::{
        NSAutoresizingMaskOptions, NSView, NSVisualEffectBlendingMode, NSVisualEffectState,
        NSVisualEffectView, NSWindowOrderingMode,
    };
    use raw_window_handle::RawWindowHandle;
    let RawWindowHandle::AppKit(handle) = window.window_handle()?.as_raw() else {
        anyhow::bail!("not an AppKit window");
    };
    // SAFETY: gpui's live view for this window.
    let view: &NSView = unsafe { handle.ns_view.cast().as_ref() };
    let mtm = view.mtm();
    let content = view
        .window()
        .context("view has no window")?
        .contentView()
        .context("no content view")?;
    let existing = content
        .subviews()
        .iter()
        .find_map(|sub| sub.downcast::<NSVisualEffectView>().ok());
    let Some(material) = config.vibrancy.as_deref() else {
        if let Some(effect) = existing {
            effect.removeFromSuperview();
        }
        return Ok(());
    };
    let Some(material) = ns_material(material) else {
        anyhow::bail!("unknown vibrancy {material}");
    };
    let effect: Retained<NSVisualEffectView> = match existing {
        Some(effect) => effect,
        None => {
            let effect =
                NSVisualEffectView::initWithFrame(NSVisualEffectView::alloc(mtm), content.bounds());
            effect.setAutoresizingMask(
                NSAutoresizingMaskOptions::ViewWidthSizable
                    | NSAutoresizingMaskOptions::ViewHeightSizable,
            );
            effect.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
            effect.setState(NSVisualEffectState::FollowsWindowActiveState);
            content.addSubview_positioned_relativeTo(&effect, NSWindowOrderingMode::Below, None);
            effect
        }
    };
    effect.setMaterial(material);
    Ok(())
}

/// Electron's vibrancy names.
#[cfg(target_os = "macos")]
fn ns_material(name: &str) -> Option<objc2_app_kit::NSVisualEffectMaterial> {
    use objc2_app_kit::NSVisualEffectMaterial as M;
    Some(match name {
        "titlebar" => M::Titlebar,
        "selection" => M::Selection,
        "menu" => M::Menu,
        "popover" => M::Popover,
        "sidebar" => M::Sidebar,
        "header" => M::HeaderView,
        "sheet" => M::Sheet,
        "window" => M::WindowBackground,
        "hud" => M::HUDWindow,
        "fullscreen-ui" => M::FullScreenUI,
        "tooltip" => M::ToolTip,
        "content" => M::ContentBackground,
        "under-window" => M::UnderWindowBackground,
        "under-page" => M::UnderPageBackground,
        _ => return None,
    })
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn platform_apply(_: &impl HasWindowHandle, _: &Config) -> anyhow::Result<()> {
    Ok(())
}
