//! Chromium's `OpenFolderViaShell` and `OpenFileViaShell` through `InvokeShellExecute`.

use std::path::Path;

use windows::{
    Win32::{
        System::{
            Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize},
            SystemServices::SFGAO_FILESYSTEM,
        },
        UI::{
            Shell::{
                Common::ITEMIDLIST, SEE_MASK_CLASSNAME, SEE_MASK_IDLIST, SEE_MASK_NOASYNC,
                SHELLEXECUTEINFOW, SHParseDisplayName, ShellExecuteExW,
            },
            WindowsAndMessaging::SW_SHOWNORMAL,
        },
    },
    core::{HSTRING, PCWSTR, w},
};

pub fn open(path: &Path) -> windows::core::Result<()> {
    let folder = path.is_dir();
    let dir = match folder {
        true => path,
        false => path.parent().unwrap_or(path),
    };
    let (file, dir) = (HSTRING::from(path), HSTRING::from(dir));
    // SAFETY: the strings and the id list outlive the call; COM is paired on this thread.
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let mut list: *mut ITEMIDLIST = std::ptr::null_mut();
        let parsed = SHParseDisplayName(&file, None, &mut list, SFGAO_FILESYSTEM.0, None);
        let result = parsed.and_then(|()| {
            let mut info = SHELLEXECUTEINFOW {
                cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
                fMask: SEE_MASK_NOASYNC | SEE_MASK_IDLIST,
                nShow: SW_SHOWNORMAL.0,
                lpDirectory: PCWSTR(dir.as_ptr()),
                lpIDList: list.cast(),
                ..Default::default()
            };
            if folder {
                info.lpVerb = w!("explore");
                info.lpClass = w!("folder");
                info.fMask |= SEE_MASK_CLASSNAME;
            }
            ShellExecuteExW(&mut info)
        });
        if !list.is_null() {
            CoTaskMemFree(Some(list.cast()));
        }
        CoUninitialize();
        result
    }
}
