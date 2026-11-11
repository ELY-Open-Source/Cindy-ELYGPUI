//! The `shell:open-external` handler: a web address handed to the system, as Electron hands it.

use gpui::{App, Task};
use url::Url;

/// The address the handler lets out: `http:` or `https:` alone.
fn web(value: &str) -> Option<Url> {
    let parsed = Url::parse(value).ok()?;
    matches!(parsed.scheme(), "http" | "https").then_some(parsed)
}

/// The handler's `success`. The billing mail and the macOS privacy panes come with their callers.
pub fn open(value: &str, cx: &App) -> Task<bool> {
    let Some(url) = web(value) else {
        log::warn!(target: "open-external", "refused an address that is no web page");
        return Task::ready(false);
    };
    log::info!(target: "open-external", "opening {}", url.origin().ascii_serialization());
    shell_open(url.into(), cx)
}

/// Electron's `shell.openExternal` for an address a handler has checked.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "the file menu calls it in 9.6h4d")
)]
pub fn hand(url: String, cx: &App) -> Task<bool> {
    shell_open(url, cx)
}

/// `XDGOpen` without waiting: a started `xdg-open` is the success.
#[cfg(all(target_os = "linux", not(test)))]
fn shell_open(url: String, _: &App) -> Task<bool> {
    let spawned = std::process::Command::new("xdg-open")
        .arg(&url)
        .env("MM_NOTTTY", "1")
        .spawn();
    Task::ready(match spawned {
        Ok(mut child) => {
            std::thread::spawn(move || match child.wait() {
                Ok(status) => log::info!(target: "open-external", "xdg-open exited: {status}"),
                Err(error) => log::warn!(target: "open-external", "xdg-open not reaped: {error}"),
            });
            true
        }
        Err(error) => {
            log::error!(target: "open-external", "xdg-open not started: {error}");
            false
        }
    })
}

/// `NSWorkspace openURL:configuration:completionHandler:`, once an application takes the address.
#[cfg(all(target_os = "macos", not(test)))]
fn shell_open(url: String, cx: &App) -> Task<bool> {
    let opened = match workspace::open(&url) {
        Ok(opened) => opened,
        Err(error) => {
            log::error!(target: "open-external", "{error}");
            return Task::ready(false);
        }
    };
    cx.background_spawn(async move {
        let opened = opened.await.unwrap_or(false);
        if !opened {
            log::error!(target: "open-external", "Failed to open URL");
        }
        opened
    })
}

/// `OpenExternalOnWorkerThread`: `ShellExecuteEx` on a COM thread.
#[cfg(all(target_os = "windows", not(test)))]
fn shell_open(url: String, cx: &App) -> Task<bool> {
    cx.background_spawn(async move {
        let opened = shell::open(&format!("\"{}\"", escape(&url)));
        if let Err(error) = &opened {
            log::error!(target: "open-external", "Failed to open: {error}");
        }
        opened.is_ok()
    })
}

/// Chromium's `EscapeExternalHandlerValue`: an escape already there stays.
#[cfg(any(windows, test))]
fn escape(spec: &str) -> String {
    let bytes = spec.as_bytes();
    let hex = |at: usize| bytes.get(at).is_some_and(u8::is_ascii_hexdigit);
    let mut out = String::with_capacity(spec.len());
    for (at, byte) in bytes.iter().copied().enumerate() {
        let kept = byte == b'%' && hex(at + 1) && hex(at + 2);
        let plain = byte.is_ascii_alphanumeric() || b";/?:@&=+$,!'()*-._~#[]".contains(&byte);
        match kept || plain {
            true => out.push(char::from(byte)),
            false => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Tests start no program: the address is recorded and the answer scripted.
#[cfg(test)]
fn shell_open(url: String, _: &App) -> Task<bool> {
    Task::ready(tests::opened(&url))
}

#[cfg(all(target_os = "windows", not(test)))]
mod shell;

#[cfg(all(target_os = "macos", not(test)))]
mod workspace;

#[cfg(test)]
pub mod tests;
