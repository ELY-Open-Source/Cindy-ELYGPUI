//! The handler's checks, Electron's image read and Node's `execFile` errors.

use std::{cell::RefCell, path::Path, time::Duration};

use futures::FutureExt as _;
use gpui::TestAppContext;
use image::{Rgba, RgbaImage};

use super::{bitmap, copy, exec};
use crate::show_item::{MISSING, NO_TARGET, REFUSED};

thread_local! {
    static COPIED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

pub fn copied(path: &str) -> Result<(), String> {
    COPIED.with_borrow_mut(|all| all.push(path.to_owned()));
    Ok(())
}

/// The paths copied on this thread since the last call.
pub fn take() -> Vec<String> {
    COPIED.take()
}

#[cfg(unix)]
#[gpui::test]
fn refused_paths_answer_with_cindys_errors(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let gone = dir.path().join("gone.png");
    let ask = |path: &str, cx: &mut TestAppContext| cx.update(|cx| copy(path, cx)).now_or_never();
    assert_eq!(ask("", cx), Some(Err(NO_TARGET.into())));
    assert_eq!(ask("a.png", cx), Some(Err(NO_TARGET.into())));
    assert_eq!(ask("/etc/ssh/a.png", cx), Some(Err(REFUSED.into())));
    assert_eq!(ask(gone.to_str().unwrap(), cx), Some(Err(MISSING.into())));
    assert!(take().is_empty());
}

#[cfg(unix)]
#[gpui::test]
fn a_file_is_copied_at_the_path_as_written(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.png"), "").unwrap();
    // Unlike the reveal, the copy does not normalize.
    let path = format!("{}/x/../a.png", dir.path().display());
    std::fs::create_dir(dir.path().join("x")).unwrap();
    let copying = cx.update(|cx| copy(&path, cx));
    cx.run_until_parked();
    assert_eq!(copying.now_or_never(), Some(Ok(())));
    assert_eq!(take(), [path]);
}

fn solid(dir: &Path, name: &str, width: u32, format: image::ImageFormat) {
    let image = RgbaImage::from_pixel(width, 1, Rgba([10, 20, 30, 255]));
    let image = image::DynamicImage::ImageRgba8(image).into_rgb8();
    image.save_with_format(dir.join(name), format).unwrap();
}

#[test]
fn an_image_is_read_by_its_bytes_not_its_name() {
    use image::ImageFormat::{Jpeg, Png};
    let dir = tempfile::tempdir().unwrap();
    let at = |name: &str| dir.path().join(name);
    let width = |name: &str| bitmap::read(at(name).to_str().unwrap()).map(|image| image.width());
    solid(dir.path(), "a.txt", 3, Png);
    solid(dir.path(), "b.png", 4, Jpeg);
    std::fs::write(at("c.png"), "GIF89a").unwrap();
    assert_eq!(width("a.txt"), Some(3));
    assert_eq!(width("b.png"), Some(4));
    // Neither a PNG nor a JPEG: `isEmpty()`.
    assert_eq!(width("c.png"), None);
    assert_eq!(width("gone.png"), None);
}

#[test]
fn the_bitmap_is_the_density_nearest_one() {
    use image::ImageFormat::Png;
    let dir = tempfile::tempdir().unwrap();
    let width = |name: &str| {
        bitmap::read(dir.path().join(name).to_str().unwrap()).map(|image| image.width())
    };
    // No file of its own: the least density over 1 stands in.
    solid(dir.path(), "a@3x.png", 3, Png);
    solid(dir.path(), "a@1.5x.png", 15, Png);
    assert_eq!(width("a.png"), Some(15));
    solid(dir.path(), "a@1x.png", 1, Png);
    assert_eq!(width("a.png"), Some(1));
    solid(dir.path(), "a.png", 7, Png);
    assert_eq!(width("a.png"), Some(7));
    // A name with its own density reads that file alone.
    assert_eq!(width("a@3x.png"), Some(3));
    assert_eq!(width("b@2x.png"), None);
    // A name with no extension takes the suffix at its end.
    solid(dir.path(), "plain@2x", 2, Png);
    assert_eq!(width("plain"), Some(2));
    // A packed or `user.js` name keeps both extensions after the suffix.
    solid(dir.path(), "c.tar@2x.gz", 4, Png);
    solid(dir.path(), "d.user@2x.js", 4, Png);
    assert_eq!(width("c.tar.gz"), None);
    assert_eq!(width("d.user.js"), None);
    solid(dir.path(), "c@2x.tar.gz", 5, Png);
    solid(dir.path(), "d@2x.USER.js", 6, Png);
    assert_eq!(width("c.tar.gz"), Some(5));
    assert_eq!(width("d.USER.js"), Some(6));
    // A folder's path keeps its last separator: the file sought lies inside it.
    std::fs::create_dir(dir.path().join("folder")).unwrap();
    solid(dir.path(), "folder@2x", 4, Png);
    assert_eq!(width("folder"), Some(4));
    for inside in ["folder/", "folder/.", "folder/./"] {
        assert_eq!(width(inside), None, "{inside}");
    }
    solid(&dir.path().join("folder"), "@2x", 9, Png);
    assert_eq!(width("folder/"), Some(9));
    // The first of two extensions is four letters at most.
    solid(dir.path(), "e.backup@2x.gz", 8, Png);
    assert_eq!(width("e.backup.gz"), Some(8));
}

#[cfg(unix)]
#[test]
fn a_path_through_a_parent_is_read_at_its_real_place() {
    let dir = tempfile::tempdir().unwrap();
    let at = |name: &str| dir.path().join(name);
    std::fs::create_dir_all(at("real")).unwrap();
    std::fs::create_dir_all(at("sub")).unwrap();
    std::fs::write(at("real/file.png"), "no image").unwrap();
    solid(&at("real"), "file@2x.png", 4, image::ImageFormat::Png);
    std::os::unix::fs::symlink(at("real/file.png"), at("alias.png")).unwrap();
    let width = |name: &str| bitmap::read(at(name).to_str().unwrap()).map(|image| image.width());
    assert_eq!(width("sub/../alias.png"), Some(4));
    // With no `..` the link's own name is searched.
    assert_eq!(width("alias.png"), None);
}

#[test]
fn a_deep_sample_keeps_its_high_byte() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("deep.png");
    let pixel = image::Rgba([0x80ff_u16, 0x12ff, 0xfefe, 0xffff]);
    image::ImageBuffer::from_pixel(1, 1, pixel)
        .save(&path)
        .unwrap();
    let read = bitmap::read(path.to_str().unwrap()).unwrap();
    assert_eq!(read.get_pixel(0, 0), &Rgba([128, 18, 254, 255]));
}

#[test]
fn a_clear_pixel_takes_skias_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.png");
    let mut image = RgbaImage::new(3, 1);
    image.put_pixel(0, 0, Rgba([200, 100, 28, 6]));
    image.put_pixel(1, 0, Rgba([9, 9, 9, 0]));
    image.put_pixel(2, 0, Rgba([1, 2, 3, 255]));
    image.save(&path).unwrap();
    let read = bitmap::read(path.to_str().unwrap()).unwrap();
    // `9.6h4b-electron-unpremul.log`: 200 and 100 premultiply to 5 and 2, 28 to 1, which is 42.
    assert_eq!(read.get_pixel(0, 0), &Rgba([212, 85, 42, 6]));
    assert_eq!(read.get_pixel(1, 0), &Rgba([0, 0, 0, 0]));
    assert_eq!(read.get_pixel(2, 0), &Rgba([1, 2, 3, 255]));
}

#[cfg(unix)]
#[test]
fn a_failed_helper_answers_with_nodes_message() {
    let long = Duration::from_secs(10);
    assert_eq!(exec::run("sh", &["-c", "exit 0"], long), Ok(()));
    assert_eq!(
        exec::run("sh", &["-c", "echo oops >&2; exit 3"], long),
        Err("Command failed: sh -c echo oops >&2; exit 3\noops\n".into())
    );
    assert_eq!(
        exec::run("nope-xyz", &["a b"], long),
        Err("spawn nope-xyz ENOENT".into())
    );
    assert_eq!(
        exec::run("sh", &["-c", "kill -9 $$"], long),
        Err("Command failed: sh -c kill -9 $$\n".into())
    );
    assert_eq!(
        exec::run("sh", &["-c", "sleep 5"], Duration::from_millis(200)),
        Err("Command failed: sh -c sleep 5\n".into())
    );
    let flood = "head -c 70000 /dev/zero | tr '\\0' a";
    assert_eq!(
        exec::run("sh", &["-c", flood], long),
        Err("stdout maxBuffer length exceeded".into())
    );
    assert_eq!(
        exec::run("sh", &["-c", &format!("{flood} >&2")], long),
        Err("stderr maxBuffer length exceeded".into())
    );
    // A byte that is no text counts as the three of U+FFFD.
    let raw = |count: usize, to: &str| format!("head -c {count} /dev/zero | tr '\\0' '\\377' {to}");
    assert_eq!(exec::run("sh", &["-c", &raw(21845, "")], long), Ok(()));
    assert_eq!(
        exec::run("sh", &["-c", &raw(21846, "")], long),
        Err("stdout maxBuffer length exceeded".into())
    );
    assert_eq!(
        exec::run("sh", &["-c", &raw(21846, ">&2")], long),
        Err("stderr maxBuffer length exceeded".into())
    );
    // A sequence the end cuts short is U+FFFD too: 65534 and three pass the limit.
    let cut = |count: usize| format!("head -c {count} /dev/zero | tr '\\0' a; printf '\\342'");
    assert_eq!(exec::run("sh", &["-c", &cut(65533)], long), Ok(()));
    assert_eq!(
        exec::run("sh", &["-c", &cut(65534)], long),
        Err("stdout maxBuffer length exceeded".into())
    );
    // The limit counts bytes: 21846 characters of three bytes pass it, 21845 do not.
    let wide = |count: usize| {
        format!("printf '%s' \"$(head -c {count} /dev/zero | tr '\\0' a | sed 's/a/中/g')\"")
    };
    assert_eq!(exec::run("sh", &["-c", &wide(21845)], long), Ok(()));
    assert_eq!(
        exec::run("sh", &["-c", &wide(21846)], long),
        Err("stdout maxBuffer length exceeded".into())
    );
}
