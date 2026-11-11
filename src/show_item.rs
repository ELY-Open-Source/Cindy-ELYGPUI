//! The `shell:show-item-in-folder` handler for a file path, then Electron's `shell.showItemInFolder`.

#[cfg(all(target_os = "macos", not(test)))]
mod finder;
#[cfg(all(target_os = "linux", not(test)))]
mod portal;
#[cfg(all(target_os = "windows", not(test)))]
mod shell;
#[cfg(test)]
pub mod tests;

use std::path::Path;

use crate::{
    file_path_policy::{allowed, is_absolute},
    node_path::normalize,
};

/// The handlers' answer to a call that names no absolute path.
pub const NO_TARGET: &str = "url 或 filePath 必须二选一";
pub const REFUSED: &str = "不允许访问该路径";
pub const MISSING: &str = "文件不存在";

/// The handler's `{ success, error }`; the file manager answers nothing, as Electron's call is void.
pub fn show(file_path: &str) -> Result<(), String> {
    if file_path.is_empty() || !is_absolute(file_path) {
        return Err(NO_TARGET.into());
    }
    // Explorer's select takes no forward slash; on POSIX this folds the path.
    let path = normalize(file_path);
    if !allowed(&path) {
        log::warn!(target: "show-item-in-folder", "refused {path}");
        return Err(REFUSED.into());
    }
    if !Path::new(&path).exists() {
        return Err(MISSING.into());
    }
    log::info!(target: "show-item-in-folder", "showing {path}");
    reveal(path);
    Ok(())
}

#[cfg(test)]
fn reveal(path: String) {
    tests::shown(path);
}

/// `ShowItemHelper` off the main thread: the portal's answer may take as long as it likes.
#[cfg(all(target_os = "linux", not(test)))]
fn reveal(path: String) {
    let spawned = std::thread::Builder::new()
        .name("show-item".into())
        .spawn(move || portal::show(&path));
    if let Err(error) = spawned {
        log::error!(target: "show-item-in-folder", "no thread for the file manager: {error}");
    }
}

#[cfg(all(target_os = "macos", not(test)))]
fn reveal(path: String) {
    finder::show(&path);
}

/// `ShowItemInFolderOnWorkerThread` on a COM thread of its own.
#[cfg(all(target_os = "windows", not(test)))]
fn reveal(path: String) {
    let spawned = std::thread::Builder::new()
        .name("show-item".into())
        .spawn(move || {
            if let Err(error) = shell::show(Path::new(&path)) {
                log::warn!(target: "show-item-in-folder", "can't open {path}: {error}");
            }
        });
    if let Err(error) = spawned {
        log::error!(target: "show-item-in-folder", "no thread for the shell: {error}");
    }
}
