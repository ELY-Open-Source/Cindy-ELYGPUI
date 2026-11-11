//! The `media:copy-to-clipboard` handler for a file path: the file itself on the clipboard.

#[cfg(any(target_os = "linux", test))]
mod bitmap;
#[cfg(any(not(target_os = "linux"), test))]
mod exec;
#[cfg(test)]
pub mod tests;

use std::path::Path;

use gpui::{App, AppContext as _, Task};

use crate::{
    file_path_policy::{allowed, is_absolute},
    show_item::{MISSING, NO_TARGET, REFUSED},
};

/// `execFile`'s `timeout`.
#[cfg(all(not(target_os = "linux"), not(test)))]
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// The handler's `{ success, error }`: the error, once the clipboard holds the file or refused it.
pub fn copy(file_path: &str, cx: &App) -> Task<Result<(), String>> {
    if file_path.is_empty() || !is_absolute(file_path) {
        return Task::ready(Err(NO_TARGET.into()));
    }
    if !allowed(file_path) {
        log::warn!(target: "media:copy-to-clipboard", "refused {file_path}");
        return Task::ready(Err(REFUSED.into()));
    }
    if !Path::new(file_path).exists() {
        return Task::ready(Err(MISSING.into()));
    }
    log::info!(target: "media:copy-to-clipboard", "copying {file_path}");
    let path = file_path.to_owned();
    cx.background_spawn(async move {
        let copied = write(&path);
        if let Err(error) = &copied {
            log::warn!(target: "media:copy-to-clipboard", "not copied: {error}");
        }
        copied
    })
}

#[cfg(test)]
fn write(path: &str) -> Result<(), String> {
    tests::copied(path)
}

/// A path with no known kind is an image here: `nativeImage` reads it, a bitmap is written.
#[cfg(all(target_os = "linux", not(test)))]
fn write(path: &str) -> Result<(), String> {
    use std::sync::Mutex;
    /// The clipboard's content lives in the one that wrote it, so it is kept.
    static BOARD: Mutex<Option<arboard::Clipboard>> = Mutex::new(None);
    let image = bitmap::read(path).ok_or("无法读取图片数据")?;
    let mut board = BOARD.lock().unwrap_or_else(|held| held.into_inner());
    if board.is_none() {
        *board = Some(arboard::Clipboard::new().map_err(|error| error.to_string())?);
    }
    let data = arboard::ImageData {
        width: image.width() as usize,
        height: image.height() as usize,
        bytes: image.into_raw().into(),
    };
    let board = board.as_mut().expect("the clipboard was just opened");
    board.set_image(data).map_err(|error| error.to_string())
}

/// `copyFileToClipboardMacOS`: the path stays in argv, out of the AppleScript source.
#[cfg(all(target_os = "macos", not(test)))]
fn write(path: &str) -> Result<(), String> {
    let script = [
        "-e",
        "on run argv",
        "-e",
        "set the clipboard to (POSIX file (item 1 of argv))",
        "-e",
        "end run",
        path,
    ];
    exec::run("osascript", &script, TIMEOUT)
}

/// `copyFileToClipboardWindows`: a single-quoted PowerShell literal reads doubled apostrophes alone.
#[cfg(all(target_os = "windows", not(test)))]
fn write(path: &str) -> Result<(), String> {
    let command = format!(
        "$ErrorActionPreference = 'Stop'; Set-Clipboard -LiteralPath '{}'",
        path.replace('\'', "''")
    );
    let args = ["-NoProfile", "-NonInteractive", "-Command", &command];
    exec::run("powershell", &args, TIMEOUT)
}
