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

#[cfg(target_os = "linux")]
fn portal_color_scheme() -> anyhow::Result<u32> {
    use zbus::zvariant::{OwnedValue, Value};
    let reply = zbus::blocking::Connection::session()?.call_method(
        Some("org.freedesktop.portal.Desktop"),
        "/org/freedesktop/portal/desktop",
        Some("org.freedesktop.portal.Settings"),
        "ReadOne",
        &("org.freedesktop.appearance", "color-scheme"),
    )?;
    let value: OwnedValue = reply.body().deserialize()?;
    let scheme = match &*value {
        Value::Value(inner) => u32::try_from(inner.as_ref())?,
        other => u32::try_from(other)?,
    };
    log::info!(target: "theme", "portal color-scheme {scheme}");
    Ok(scheme)
}
