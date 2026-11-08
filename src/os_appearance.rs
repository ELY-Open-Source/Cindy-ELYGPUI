use gpui::{App, WindowAppearance};

/// The OS appearance at start, and whether gpui already reports it.
pub struct Initial {
    pub os: WindowAppearance,
    pub gpui_agrees: bool,
}

/// gpui learns Linux's color scheme only after the window opens; ask the portal first.
#[cfg(target_os = "linux")]
pub fn initial(cx: &App) -> Initial {
    let os = match portal_color_scheme() {
        Ok(1) => WindowAppearance::Dark,
        Ok(_) => WindowAppearance::Light,
        Err(error) => {
            log::warn!(target: "theme", "portal color-scheme unread ({error:#}); using gpui's");
            return Initial {
                os: cx.window_appearance(),
                gpui_agrees: true,
            };
        }
    };
    Initial {
        os,
        gpui_agrees: false,
    }
}

#[cfg(not(target_os = "linux"))]
pub fn initial(cx: &App) -> Initial {
    Initial {
        os: cx.window_appearance(),
        gpui_agrees: true,
    }
}

/// One portal setting, unwrapped from its variant.
#[cfg(target_os = "linux")]
fn portal_read<T>(namespace: &str, key: &str) -> anyhow::Result<T>
where
    T: for<'a> TryFrom<&'a zbus::zvariant::Value<'a>, Error = zbus::zvariant::Error>,
{
    use zbus::zvariant::{OwnedValue, Value};
    let reply = zbus::blocking::Connection::session()?.call_method(
        Some("org.freedesktop.portal.Desktop"),
        "/org/freedesktop/portal/desktop",
        Some("org.freedesktop.portal.Settings"),
        "ReadOne",
        &(namespace, key),
    )?;
    let value: OwnedValue = reply.body().deserialize()?;
    Ok(match &*value {
        Value::Value(inner) => T::try_from(inner.as_ref())?,
        other => T::try_from(other)?,
    })
}

#[cfg(target_os = "linux")]
fn portal_color_scheme() -> anyhow::Result<u32> {
    let scheme = portal_read("org.freedesktop.appearance", "color-scheme")?;
    log::info!(target: "theme", "portal color-scheme {scheme}");
    Ok(scheme)
}

/// `prefers-reduced-motion` as Chromium reads it: GTK's `enable-animations` turned off.
#[cfg(target_os = "linux")]
pub fn reduced_motion() -> anyhow::Result<bool> {
    let animations: bool = portal_read("org.gnome.desktop.interface", "enable-animations")?;
    Ok(!animations)
}

/// `prefers-reduced-motion` as Chromium reads it: Reduce motion in Accessibility.
#[cfg(target_os = "macos")]
pub fn reduced_motion() -> anyhow::Result<bool> {
    let workspace = objc2_app_kit::NSWorkspace::sharedWorkspace();
    Ok(workspace.accessibilityDisplayShouldReduceMotion())
}

/// `prefers-reduced-motion` as Chromium reads it: client-area animation turned off.
#[cfg(target_os = "windows")]
pub fn reduced_motion() -> anyhow::Result<bool> {
    use windows::{
        Win32::UI::WindowsAndMessaging::{
            SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
        },
        core::BOOL,
    };
    let mut animations = BOOL(1);
    let out = std::ptr::from_mut(&mut animations).cast();
    // SAFETY: SPI_GETCLIENTAREAANIMATION writes one BOOL to `out`.
    unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some(out),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )?;
    }
    Ok(!animations.as_bool())
}
