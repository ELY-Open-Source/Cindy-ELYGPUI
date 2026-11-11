//! `openFileInBrowser.ts`: a local HTML page, and the address the system browser takes for it.

mod convert;
#[cfg(test)]
mod tests;

use std::path::Path;

use gpui::{App, Task};

use crate::{file_path_policy::allowed, node_path::win32, open_external, open_path};

const TARGET: &str = "shell:open-file-in-browser";

/// `BrowserFileOpenErrorCode`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Code {
    InvalidTarget,
    PathNotAllowed,
    UnsupportedType,
    NotFound,
    OpenFailed,
}

impl Code {
    /// The code as the IPC error carries it.
    pub fn name(self) -> &'static str {
        match self {
            Self::InvalidTarget => "BROWSER_FILE_INVALID_TARGET",
            Self::PathNotAllowed => "BROWSER_FILE_PATH_NOT_ALLOWED",
            Self::UnsupportedType => "BROWSER_FILE_UNSUPPORTED_TYPE",
            Self::NotFound => "BROWSER_FILE_NOT_FOUND",
            Self::OpenFailed => "BROWSER_FILE_OPEN_FAILED",
        }
    }
}

/// The handler's failure: its code and Cindy's message.
pub type Failure = (Code, String);

/// `BrowserFileTarget` of an absolute path: the path and its `file:` URL.
#[derive(Debug, PartialEq)]
pub struct Target {
    pub path: String,
    pub url: String,
}

/// `isBrowserOpenablePath`: the name ends in `.html` or `.htm`, whatever its case.
pub fn openable(path: &str) -> bool {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    match name.rfind('.') {
        Some(dot) if dot > 0 => {
            let extension = name[dot..].to_lowercase();
            extension == ".html" || extension == ".htm"
        }
        _ => false,
    }
}

fn absolute(path: &str, windows: bool) -> bool {
    match windows {
        true => win32::is_absolute(path),
        false => path.starts_with('/'),
    }
}

/// `resolveBrowserFileTarget` under one platform's rules; `Err` where Cindy's throws.
fn target_on(value: &str, windows: bool, cwd: &str) -> Result<Option<Target>, ()> {
    // A `file:` URL, the built-in browser's input, comes with 17.0.
    if !absolute(value, windows) {
        return Ok(None);
    }
    let url = convert::to_url(value, windows, cwd).ok_or(())?;
    Ok(Some(Target {
        path: value.to_owned(),
        url,
    }))
}

/// `resolveBrowserFileTarget` on this host, from the current folder.
pub fn target(value: &str) -> Result<Option<Target>, ()> {
    if !cfg!(windows) {
        return target_on(value, false, "");
    }
    // A rooted Windows path with no device takes the current folder's.
    let cwd = std::env::current_dir().map_err(|error| {
        log::error!(target: TARGET, "no current folder: {error}");
    })?;
    target_on(value, true, &cwd.to_string_lossy())
}

/// What the checks before the open refuse.
fn checked(value: &str) -> Result<Target, Failure> {
    let failed = |code: Code, error: &str| (code, error.to_owned());
    let target = target(value)
        .map_err(|()| failed(Code::OpenFailed, "无法在系统浏览器中打开该本地页面"))?
        .ok_or_else(|| failed(Code::InvalidTarget, "路径必须是绝对路径或本地 file:// URL"))?;
    if !allowed(&target.path) {
        return Err(failed(Code::PathNotAllowed, "不允许访问该路径"));
    }
    if !openable(&target.path) {
        return Err(failed(Code::UnsupportedType, "该文件类型不支持浏览器查看"));
    }
    if !Path::new(&target.path).exists() {
        return Err(failed(Code::NotFound, "文件不存在"));
    }
    Ok(target)
}

/// `handleOpenFileInBrowser`: the page's URL to the system, else the path to `shell.openPath`.
pub fn open(value: &str, cx: &App) -> Task<Result<(), Failure>> {
    let target = match checked(value) {
        Ok(target) => target,
        Err(failure) => {
            log::warn!(target: TARGET, "refused: {}", failure.0.name());
            return Task::ready(Err(failure));
        }
    };
    log::info!(target: TARGET, "opening {}", target.path);
    let external = open_external::hand(target.url, cx);
    cx.spawn(async move |cx| {
        if external.await {
            return Ok(());
        }
        log::warn!(target: TARGET, "openExternal failed for {}, fallback: openPath", target.path);
        let opening = cx.update(|cx| open_path::hand(&target.path, cx));
        opening.await.map_err(|error| (Code::OpenFailed, error))
    })
}
