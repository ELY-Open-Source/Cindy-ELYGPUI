//! `useFileChipContextMenu` for a local task: a resolved file's menu, and what activating it does.

use ely_gpui_component::{
    menus::{Menu, MenuItem},
    primitives::IconName,
};
use gpui::{App, ClipboardItem, Modifiers, MouseButton, Task, Window};

use super::file::Resolved;
use crate::{
    browser_file, copy_media, file_location,
    i18n::Translator,
    link_open::{self, Kind, Target},
    open_path,
    session_view::links,
    show_item, toast,
};

#[cfg(test)]
mod tests;

const LOG: &str = "sessionView";

/// `isHtmlFilePath`.
pub fn html(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    [".html", ".htm", ".xhtml"]
        .iter()
        .any(|end| lower.ends_with(end))
}

fn words(key: &str, cx: &App) -> String {
    cx.global::<Translator>().t(key, &[])
}

/// `then` in the window that asked, once `task` settles.
fn after<T: 'static>(
    task: Task<T>,
    window: &Window,
    cx: &mut App,
    then: impl FnOnce(T, &Window, &mut App) + 'static,
) {
    let handle = window.window_handle();
    cx.spawn(async move |cx| {
        let done = task.await;
        let shown = handle.update(cx, |_, window, cx| then(done, window, cx));
        if let Err(error) = shown {
            log::warn!(target: LOG, "a file action's answer had no window: {error:#}");
        }
    })
    .detach();
}

/// `openHtmlFileByPreference` for a local task; a refusal's code names its toast.
pub fn page(path: &str, to: Option<Target>, window: &Window, cx: &mut App) {
    if to.unwrap_or_else(|| link_open::preference(Kind::Local, cx)) == Target::Sidebar {
        return links::sidebar(window, cx);
    }
    let opening = browser_file::open(path, cx);
    after(opening, window, cx, |opened, window, cx| {
        if let Err((code, error)) = opened {
            log::warn!(target: LOG, "the page did not open: {error}");
            let key = format!("chat.markdownRenderer.{}", code.name());
            links::failed(&key, window, cx);
        }
    });
}

/// `handleOpenWithDefault`.
fn with_app(file: &Resolved, window: &Window, cx: &mut App) {
    let opening = open_path::open(&file.path, cx);
    after(opening, window, cx, |opened, window, cx| {
        if let Err(error) = opened {
            log::warn!(target: LOG, "the file did not open: {error}");
            toast::error(window, error, toast::Options::default(), cx);
        }
    });
}

/// `handleReveal`.
fn reveal(file: &Resolved, window: &Window, cx: &mut App) {
    if let Err(error) = show_item::show(&file.path) {
        log::warn!(target: LOG, "the file was not shown: {error}");
        toast::error(window, error, toast::Options::default(), cx);
    }
}

/// `handleCopyFile`.
fn copy_file(file: &Resolved, window: &Window, cx: &mut App) {
    let copying = copy_media::copy(&file.path, cx);
    after(copying, window, cx, |copied, window, cx| match copied {
        Ok(()) => {
            let message = words("chat.markdownRenderer.fileCopied", cx);
            toast::success(window, message, toast::Options::default(), cx);
        }
        Err(error) => {
            log::warn!(target: LOG, "the file was not copied: {error}");
            toast::error(window, error, toast::Options::default(), cx);
        }
    });
}

/// `navigator.clipboard.writeText` with its toast; gpui's clipboard write reports no failure.
fn copy_text(text: &str, said: &str, window: &Window, cx: &mut App) {
    cx.write_to_clipboard(ClipboardItem::new_string(text.to_owned()));
    let message = words(said, cx);
    toast::success(window, message, toast::Options::default(), cx);
}

/// `handleOpenInSidebarFileBrowser`: no file browser before 18.0, so it fails as Cindy's fails.
fn locate(file: &Resolved, window: &Window, cx: &mut App) {
    if file.folder && file_location::rel(&file.cwd, &file.path).is_none() {
        let key = "chat.markdownRenderer.openInSidebarFileBrowserOutsideWorkdir";
        return links::failed(key, window, cx);
    }
    log::error!(target: LOG, "no sidebar file browser shows a file before 18.0");
    links::failed(
        "chat.markdownRenderer.openInSidebarFileBrowserFailed",
        window,
        cx,
    );
}

/// `onActivate` about the faces' `openDefault`.
pub fn activate(file: &Resolved, held: (Modifiers, MouseButton), window: &Window, cx: &mut App) {
    let apart = link_open::separate(held.0, held.1);
    log::info!(target: LOG, "a resolved file activated, apart: {apart}");
    match (apart, file.browsable()) {
        (true, true) => page(&file.path, Some(Target::External), window, cx),
        (true, false) => with_app(file, window, cx),
        (false, _) if !file.folder && html(&file.path) => page(&file.path, None, window, cx),
        // `onOpen`: the lightboxes and the file browser come with 9.13.
        (false, _) => log::warn!(target: LOG, "nothing opens a resolved file before 9.13"),
    }
}

fn row(
    file: &Resolved,
    (key, icon): (&str, IconName),
    cx: &App,
    run: impl Fn(&Resolved, &Window, &mut App) + 'static,
) -> MenuItem {
    let file = file.clone();
    MenuItem::new(words(key, cx))
        .icon(icon)
        .on_click(move |window, cx| run(&file, window, cx))
}

/// The rows; Ely's `Folder` and `Clipboard` stand in for `folder-tree` and `clipboard-copy`.
pub fn menu(file: &Resolved, cx: &App) -> Menu {
    let shift = words("chat.markdownRenderer.shiftClick", cx);
    let named = |key: &str| format!("chat.markdownRenderer.{key}");
    let mut menu = Menu::new();
    if file.browsable() {
        let sidebar = (named("openInSidebarBrowser"), IconName::PanelRight);
        let browser = (named("openInDefaultBrowser"), IconName::Globe);
        let source = (named("viewSourceFile"), IconName::FileCode);
        menu = menu
            .item(row(
                file,
                (&sidebar.0, sidebar.1),
                cx,
                |file, window, cx| page(&file.path, Some(Target::Sidebar), window, cx),
            ))
            .item(
                row(file, (&browser.0, browser.1), cx, |file, window, cx| {
                    page(&file.path, Some(Target::External), window, cx)
                })
                .keys(shift),
            )
            // `onViewSource` and `TextLightbox` come with 9.13.
            .item(row(file, (&source.0, source.1), cx, |_, _, _| {
                log::warn!(target: LOG, "no source preview opens before 9.13");
            }))
            .separator();
    } else {
        let listed = !file.cwd.is_empty();
        if listed {
            let key = named("openInSidebarFileBrowser");
            menu = menu.item(row(file, (&key, IconName::Folder), cx, locate));
        }
        if !file.folder {
            let open = ("chat.media.openWithApp", IconName::AppWindow);
            menu = menu.item(row(file, open, cx, with_app).keys(shift));
        }
        if listed || !file.folder {
            menu = menu.separator();
        }
    }
    let show = named("revealFile");
    menu = menu.item(row(file, (&show, IconName::FolderOpen), cx, reveal));
    if !file.browsable() {
        let key = named("copyFile");
        menu = menu.item(row(file, (&key, IconName::Copy), cx, copy_file));
    }
    let key = named("copyFilePath");
    menu = menu.item(row(
        file,
        (&key, IconName::Clipboard),
        cx,
        |file, window, cx| copy_text(&file.path, "chat.markdownRenderer.pathCopied", window, cx),
    ));
    let place = (!file.folder).then(|| file_location::format(&file.cwd, &file.path, file.at));
    if let Some(place) = place.flatten() {
        let key = named("copyLocation");
        let copy = move |_: &Resolved, window: &Window, cx: &mut App| {
            copy_text(&place, "chat.markdownRenderer.locationCopied", window, cx)
        };
        menu = menu.item(row(file, (&key, IconName::Clipboard), cx, copy));
    }
    menu
}
