//! Chromium's `ShowItemInFolderOnWorkerThread`: the folder opened with the file selected.

use std::path::Path;

use windows::{
    Win32::{
        System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize},
        UI::Shell::{Common::ITEMIDLIST, SHOpenFolderAndSelectItems, SHParseDisplayName},
    },
    core::HSTRING,
};

/// An id list freed when it goes.
struct Item(*mut ITEMIDLIST);

impl Item {
    /// The desktop folder's `ParseDisplayName`, which `SHParseDisplayName` calls.
    fn parse(path: &HSTRING) -> windows::core::Result<Self> {
        let mut list = std::ptr::null_mut();
        // SAFETY: `list` is a valid out pointer and the string outlives the call.
        unsafe { SHParseDisplayName(path, None, &mut list, 0, None)? };
        Ok(Self(list))
    }
}

impl Drop for Item {
    fn drop(&mut self) {
        // SAFETY: the shell allocated the list with the COM allocator.
        unsafe { CoTaskMemFree(Some(self.0.cast())) };
    }
}

pub fn show(path: &Path) -> windows::core::Result<()> {
    let Some(dir) = path.parent() else {
        return Ok(());
    };
    // Parsing fails for `C:`; it must be `C:\`.
    let mut dir = dir.as_os_str().to_owned();
    if !dir.to_string_lossy().ends_with('\\') {
        dir.push("\\");
    }
    // SAFETY: COM is paired on this thread and the id lists live through the call.
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let result = (|| {
            let folder = Item::parse(&HSTRING::from(&*dir))?;
            let file = Item::parse(&HSTRING::from(path))?;
            SHOpenFolderAndSelectItems(folder.0, Some(&[file.0.cast_const()]), 0)
        })();
        CoUninitialize();
        result
    }
}
