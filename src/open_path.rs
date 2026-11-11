//! The `shell:open-path` handler: the target checked, then Electron's `shell.openPath`.

use std::path::{Path, PathBuf};

use gpui::{App, Task};

use crate::{
    dev_profile::canonical::resolve,
    file_path_policy::{allowed, is_absolute},
};

/// `stripWindowsDriveUriSlash`: `/D:/x` back to `D:/x`, `/D:` to `D:/`.
pub fn strip_drive_uri_slash(value: &str) -> String {
    let b = value.as_bytes();
    let drive = b.len() >= 3 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b':';
    match (drive, b.get(3)) {
        (true, None) => format!("{}/", &value[1..]),
        (true, Some(b'/' | b'\\')) => value[1..].to_owned(),
        _ => value.to_owned(),
    }
}

/// `resolveShellOpenPathTarget` for paths; `cindy-media://` comes with the media store.
pub fn target(value: &str) -> Option<PathBuf> {
    let value = match cfg!(windows) {
        true => strip_drive_uri_slash(value),
        false => value.to_owned(),
    };
    (!value.is_empty() && is_absolute(&value)).then(|| resolve(Path::new(&value)))
}

/// The handler's `{ success, error }`: the error, once the open settles.
pub fn open(value: &str, cx: &App) -> Task<Result<(), String>> {
    let Some(path) = target(value) else {
        return Task::ready(Err("Path must be absolute".into()));
    };
    if !allowed(&path.to_string_lossy()) {
        log::warn!(target: "open-path", "refused {}", path.display());
        return Task::ready(Err("不允许访问该路径".into()));
    }
    log::info!(target: "open-path", "opening {}", path.display());
    shell_open(path, cx)
}

/// Electron's `shell.openPath` for a path a handler has checked: the error, once it settles.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "the file menu calls it in 9.6h4d")
)]
pub fn hand(path: &str, cx: &App) -> Task<Result<(), String>> {
    shell_open(PathBuf::from(path), cx)
}

/// `XDGOpen` without waiting: `xdg-open` started in `dir` and reaped when it ends.
#[cfg(all(target_os = "linux", not(test)))]
pub fn xdg_open(what: &std::ffi::OsStr, dir: &Path) {
    let spawned = std::process::Command::new("xdg-open")
        .arg(what)
        .current_dir(dir)
        .env("MM_NOTTTY", "1")
        .spawn();
    match spawned {
        Ok(mut child) => {
            std::thread::spawn(move || match child.wait() {
                Ok(status) => log::info!(target: "open-path", "xdg-open exited: {status}"),
                Err(error) => log::warn!(target: "open-path", "xdg-open not reaped: {error}"),
            });
        }
        Err(error) => log::error!(target: "open-path", "xdg-open not started: {error}"),
    }
}

/// Electron's Linux `openPath` never settles, so nothing returns.
#[cfg(all(target_os = "linux", not(test)))]
fn shell_open(path: PathBuf, _: &App) -> Task<Result<(), String>> {
    xdg_open(path.as_os_str(), path.parent().unwrap_or(&path));
    Task::ready(Ok(()))
}

/// `NSWorkspace openURL` on the main thread.
#[cfg(all(target_os = "macos", not(test)))]
fn shell_open(path: PathBuf, _: &App) -> Task<Result<(), String>> {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::{NSString, NSURL};
    let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
    let opened = NSWorkspace::sharedWorkspace().openURL(&url);
    Task::ready(match opened {
        true => Ok(()),
        false => Err("Failed to open path".into()),
    })
}

/// `OpenPathOnThread` on a COM thread: `explore` for a folder, the default verb for a file.
#[cfg(all(target_os = "windows", not(test)))]
fn shell_open(path: PathBuf, cx: &App) -> Task<Result<(), String>> {
    cx.background_spawn(async move {
        match shell::open(&path) {
            Ok(()) => Ok(()),
            Err(error) => {
                log::warn!(target: "open-path", "ShellExecuteEx failed: {error}");
                Err("Failed to open path".into())
            }
        }
    })
}

#[cfg(all(target_os = "windows", not(test)))]
mod shell;

/// Tests start no program: the path is recorded and the answer scripted.
#[cfg(test)]
fn shell_open(path: PathBuf, _: &App) -> Task<Result<(), String>> {
    Task::ready(tests::opened(&path))
}

#[cfg(test)]
pub mod tests;
