//! Electron's macOS `ShowItemInFolder`: the file selected in a Finder window.

use objc2_app_kit::NSWorkspace;
use objc2_foundation::NSString;

/// `selectFile:inFileViewerRootedAtPath:`, on the main thread.
pub fn show(path: &str) {
    let (file, root) = (NSString::from_str(path), NSString::from_str(""));
    if !NSWorkspace::sharedWorkspace().selectFile_inFileViewerRootedAtPath(Some(&file), &root) {
        log::warn!(target: "show-item-in-folder", "NSWorkspace failed to select file {path}");
    }
}
