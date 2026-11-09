//! The handler's checks before the shell sees a path.

use gpui::TestAppContext;

use super::{open, strip_drive_uri_slash};

#[test]
fn a_uri_drive_slash_comes_off() {
    assert_eq!(strip_drive_uri_slash("/D:/x"), "D:/x");
    assert_eq!(strip_drive_uri_slash("/d:\\x"), "d:\\x");
    assert_eq!(strip_drive_uri_slash("/D:"), "D:/");
    assert_eq!(strip_drive_uri_slash("/Dx/y"), "/Dx/y");
    assert_eq!(strip_drive_uri_slash("/D:x"), "/D:x");
}

#[cfg(unix)]
#[test]
fn only_an_absolute_path_is_a_target() {
    use super::target;
    assert_eq!(target(""), None);
    assert_eq!(target("work/app"), None);
    assert_eq!(target("cindy-media://a"), None);
    assert_eq!(
        target("/a/./b/../c/").as_deref(),
        Some(std::path::Path::new("/a/c"))
    );
}

#[cfg(unix)]
#[gpui::test]
fn refused_paths_answer_with_cindys_errors(cx: &mut TestAppContext) {
    use futures::FutureExt as _;
    let (relative, system) = cx.update(|cx| (open("work", cx), open("/etc/x/../ssh", cx)));
    assert_eq!(
        relative.now_or_never(),
        Some(Err("Path must be absolute".into()))
    );
    assert_eq!(system.now_or_never(), Some(Err("不允许访问该路径".into())));
}

#[cfg(windows)]
#[gpui::test]
fn the_windows_roots_are_refused(cx: &mut TestAppContext) {
    use futures::FutureExt as _;
    let windows = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
    let opening = cx.update(|cx| open(&format!("{windows}\\System32"), cx));
    assert_eq!(opening.now_or_never(), Some(Err("不允许访问该路径".into())));
}
