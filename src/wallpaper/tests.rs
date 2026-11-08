use std::path::Path;

use serde_json::Value;

use super::{custom_url, media, read};
use crate::cindy_json;

const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn settings(text: &str, user_data: &Path) -> super::Settings {
    read(&cindy_json::object(text).unwrap_or_default(), user_data)
}

/// Expected values from Cindy's `normalizeAppearanceSettings` and `getWallpaperVisibility`.
#[test]
fn normalizes_as_cindy_does() {
    let inputs: Vec<Value> =
        serde_json::from_str(include_str!("../../tests/fixtures/appearance/input.json")).unwrap();
    let expected: Vec<Value> = serde_json::from_str(include_str!(
        "../../tests/fixtures/appearance/expected.json"
    ))
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    for (input, want) in inputs.iter().zip(&expected) {
        let name = &want["name"];
        let got = settings(input["text"].as_str().unwrap(), dir.path());
        let id = match want["wallpaperId"].as_str().unwrap() {
            "custom" => "none",
            id => id,
        };
        assert_eq!(got.id, id, "{name}: custom without a file falls back");
        assert_eq!(
            Some(got.overlay),
            want["wallpaperOverlay"].as_f64(),
            "{name}"
        );
        assert_eq!(
            got.visibility,
            want["wallpaperVisibility"].as_f64(),
            "{name}"
        );
        assert_eq!(got.blur, want["wallpaperBlur"].as_f64(), "{name}");
        let visibility = [got.visibility(false), got.visibility(true)];
        let want_visibility: Vec<f64> = serde_json::from_value(want["visibility"].clone()).unwrap();
        assert_eq!(visibility.to_vec(), want_visibility, "{name}");
    }
}

#[test]
fn the_custom_url_comes_from_its_own_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("custom-wallpaper.json");
    let url = format!("cindy-media://client-wallpaper/{HASH}.webp");
    std::fs::write(&file, format!(r#"{{"url": "{url}"}}"#)).unwrap();
    let custom = settings(r#"{"wallpaperId": "custom"}"#, dir.path());
    assert_eq!(
        (custom.id, custom.custom_url.as_str()),
        ("custom", url.as_str())
    );
    std::fs::write(&file, "[1]").unwrap();
    assert_eq!(
        settings(r#"{"wallpaperId": "custom"}"#, dir.path()).id,
        "none"
    );
    assert!(file.exists(), "an unreadable file stays");
    for bad in [
        format!("cindy-media://client-wallpaper/{HASH}.png"),
        format!("cindy-media://blobs/{HASH}.webp"),
        format!(
            "cindy-media://client-wallpaper/{}.webp",
            HASH.to_uppercase()
        ),
        format!("cindy-media://client-wallpaper/{HASH}0.mp4"),
    ] {
        assert_eq!(custom_url(Some(bad.clone())), "", "{bad}");
    }
    let video = format!("cindy-media://client-wallpaper/{HASH}.mp4");
    assert_eq!(custom_url(Some(video.clone())), video);
}

#[test]
fn logs_never_name_the_custom_media() {
    let dir = tempfile::tempdir().unwrap();
    let url = format!("cindy-media://client-wallpaper/{HASH}.webp");
    std::fs::write(
        dir.path().join("custom-wallpaper.json"),
        format!(r#"{{"url": "{url}"}}"#),
    )
    .unwrap();
    std::fs::write(
        dir.path().join("appearance-settings.json"),
        r#"{"wallpaperId": "custom"}"#,
    )
    .unwrap();
    let logged = format!("{:?}", crate::appearance_settings::read(dir.path()));
    assert!(
        logged.contains("custom: true") && !logged.contains(&HASH[..8]),
        "{logged}"
    );
    let bucket = dir.path().join("cindy-media/client-wallpaper/01");
    for step in 0..2 {
        let error = format!("{:#}", media::custom_path(dir.path(), &url).unwrap_err());
        assert!(
            !error.contains(&HASH[..8]) && !error.contains("client-wallpaper/"),
            "{error}"
        );
        if step == 0 {
            std::fs::create_dir_all(&bucket).unwrap();
        }
    }
}

#[test]
fn custom_media_must_stay_inside_the_store() {
    let dir = tempfile::tempdir().unwrap();
    let url = format!("cindy-media://client-wallpaper/{HASH}.webp");
    let bucket = dir.path().join("cindy-media/client-wallpaper/01");
    std::fs::create_dir_all(&bucket).unwrap();
    assert!(media::custom_path(dir.path(), &url).is_err(), "missing");
    std::fs::write(bucket.join(format!("{HASH}.webp")), b"x").unwrap();
    let path = media::custom_path(dir.path(), &url).unwrap();
    assert!(path.ends_with(format!("cindy-media/client-wallpaper/01/{HASH}.webp")));
    #[cfg(unix)]
    {
        let outside = tempfile::tempdir().unwrap();
        std::fs::remove_dir_all(&bucket).unwrap();
        std::os::unix::fs::symlink(outside.path(), &bucket).unwrap();
        std::fs::write(outside.path().join(format!("{HASH}.webp")), b"x").unwrap();
        let error = media::custom_path(dir.path(), &url).unwrap_err();
        assert!(
            format!("{error:#}").contains("ancestor is a symlink"),
            "{error:#}"
        );
    }
}

#[cfg(unix)]
#[test]
fn a_bucket_swapped_before_the_open_is_caught() {
    let dir = tempfile::tempdir().unwrap();
    let url = format!("cindy-media://client-wallpaper/{HASH}.webp");
    let bucket = dir.path().join("cindy-media/client-wallpaper/01");
    std::fs::create_dir_all(&bucket).unwrap();
    std::fs::write(bucket.join(format!("{HASH}.webp")), b"inside").unwrap();
    assert_eq!(media::custom_media(dir.path(), &url).unwrap(), b"inside");
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join(format!("{HASH}.webp")), b"outside").unwrap();
    let swap = || {
        std::fs::rename(&bucket, dir.path().join("kept")).unwrap();
        std::os::unix::fs::symlink(outside.path(), &bucket).unwrap();
    };
    let error = media::read_contained(dir.path(), &url, swap).unwrap_err();
    assert!(format!("{error:#}").contains("symlink"), "{error:#}");
}

#[cfg(unix)]
#[test]
fn a_fifo_swapped_in_fails_without_blocking() {
    let dir = tempfile::tempdir().unwrap();
    let url = format!("cindy-media://client-wallpaper/{HASH}.webp");
    let bucket = dir.path().join("cindy-media/client-wallpaper/01");
    std::fs::create_dir_all(&bucket).unwrap();
    let file = bucket.join(format!("{HASH}.webp"));
    std::fs::write(&file, b"inside").unwrap();
    let fifo = std::ffi::CString::new(file.to_str().unwrap()).unwrap();
    let swap = move || {
        std::fs::remove_file(&file).unwrap();
        // SAFETY: a NUL-terminated path.
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    };
    let (done, finished) = std::sync::mpsc::channel();
    let user_data = dir.path().to_owned();
    std::thread::spawn(move || {
        let _ = done.send(media::read_contained(&user_data, &url, swap).is_err());
    });
    let failed = finished.recv_timeout(std::time::Duration::from_secs(2));
    assert_eq!(failed, Ok(true), "rejected, not stuck on the FIFO");
    let named = crate::bounded_read::path_identity(&bucket.join(format!("{HASH}.webp")));
    assert!(named.is_err(), "lstat sees no regular file");
}

#[test]
fn every_official_id_has_its_preview() {
    for id in super::IDS {
        let bundled = media::preview(id).is_some();
        assert_eq!(bundled, !matches!(id, "none" | "custom"), "{id}");
    }
    let image = media::decode(media::preview("dune-dawn").unwrap()).unwrap();
    assert!(image.width() > 0);
}

#[test]
fn a_cached_original_must_match_its_manifest_entry() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.webp");
    std::fs::write(&file, b"cindy").unwrap();
    let sha = "e8afcc1fdec8f0d5d5ed1a3ab2bb4ff5e4b8d3b02a85fae4d2b8b9f4f1d5f1ab";
    let real = format!("{:x}", <sha2::Sha256 as sha2::Digest>::digest(b"cindy"));
    assert_eq!(
        media::verified(&file, &real, 5).as_deref(),
        Some(&b"cindy"[..])
    );
    assert_eq!(media::verified(&file, sha, 5), None, "wrong hash");
    assert_eq!(media::verified(&file, &real, 4), None, "over the size");
    assert_eq!(media::verified(&file, &real, 6), None, "short of the size");
    assert_eq!(
        media::verified(&dir.path().join("b"), &real, 5),
        None,
        "missing"
    );
    #[cfg(unix)]
    {
        let link = dir.path().join("link.webp");
        std::os::unix::fs::symlink(&file, &link).unwrap();
        assert_eq!(media::verified(&link, &real, 5), None, "a link");
    }
    assert!(
        media::cached_original(dir.path(), "dune-dawn")
            .unwrap()
            .is_none()
    );
}

#[test]
fn blur_crops_the_cover_box_first() {
    let mut image = image::RgbaImage::new(40, 10);
    for (x, _, pixel) in image.enumerate_pixels_mut() {
        pixel.0 = if x < 20 {
            [255, 0, 0, 255]
        } else {
            [0, 0, 255, 255]
        };
    }
    let out = media::blurred(&image, (200, 200), 4.0);
    assert_eq!(
        out.dimensions(),
        (10, 10),
        "the 10px square cover shows, at source size"
    );
    let left = out.get_pixel(0, 5).0;
    let right = out.get_pixel(9, 5).0;
    assert!(left[0] > 200 && right[2] > 200, "{left:?} {right:?}");
}

struct Canvas;

impl gpui::Render for Canvas {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        use gpui::{ParentElement as _, Styled as _};
        gpui::div().size_full().children(super::canvas(window, cx))
    }
}

fn paint(cx: &mut gpui::TestAppContext, text: &str) -> gpui::VisualTestContext {
    let dir = tempfile::tempdir().unwrap();
    let wallpaper = settings(text, dir.path());
    cx.update(|cx| {
        let initial = crate::os_appearance::Initial {
            os: gpui::WindowAppearance::Dark,
            gpui_agrees: true,
        };
        let appearance =
            crate::theme::Appearance::new(crate::theme::Mode::System, Some("cindy"), initial, None);
        cx.set_global(appearance.unwrap());
        cx.set_global(crate::units::Zoom(1.0));
        cx.set_global(crate::window_material::Material::load());
        super::Wallpaper::install(wallpaper, dir.path(), cx);
    });
    let (_, vcx) = cx.add_window_view(|_, _| Canvas);
    let mut vcx = vcx.clone();
    vcx.run_until_parked();
    vcx.update(|window, _| window.refresh());
    vcx.run_until_parked();
    vcx
}

#[gpui::test]
fn an_official_wallpaper_covers_the_window(cx: &mut gpui::TestAppContext) {
    let mut vcx = paint(cx, r#"{"wallpaperId": "dune-dawn", "wallpaperBlur": 4}"#);
    let canvas = vcx
        .debug_bounds("wallpaper-canvas")
        .expect("canvas painted");
    let viewport = vcx.update(|window, _| window.viewport_size());
    assert_eq!(canvas.size, viewport);
    vcx.update(|_, cx| {
        assert!(super::active(cx));
        let config = cx.global::<crate::window_material::Material>().config(cx);
        let opaque = crate::window_material::resolve(
            "default",
            true,
            crate::window_material::Platform::Mac,
            false,
            &Default::default(),
        );
        let mac = crate::window_material::resolve(
            "cindy",
            true,
            crate::window_material::Platform::Mac,
            true,
            &Default::default(),
        );
        assert_eq!(mac, opaque, "coverage turns the material off");
        assert_eq!(config.vibrancy, None);
        let area = gpui::red();
        assert_eq!(super::clear(area, cx), gpui::transparent_black());
        let wallpaper = cx.global::<super::Wallpaper>();
        assert!(wallpaper.image.is_some(), "the preview decoded");
        assert!(wallpaper.blurred.is_some(), "the blurred canvas is ready");
    });
}

#[gpui::test]
fn a_new_source_blurs_again(cx: &mut gpui::TestAppContext) {
    let mut vcx = paint(cx, r#"{"wallpaperId": "dune-dawn", "wallpaperBlur": 4}"#);
    let blurred = |vcx: &mut gpui::VisualTestContext| {
        vcx.update(|_, cx| {
            cx.global::<super::Wallpaper>()
                .blurred
                .as_ref()
                .unwrap()
                .image
                .id
        })
    };
    let first = blurred(&mut vcx);
    vcx.update(|window, cx| {
        let source = image::RgbaImage::new(8, 8);
        gpui::BorrowAppContext::update_global::<super::Wallpaper, _>(cx, |w, cx| {
            w.show(source, cx)
        });
        window.refresh();
    });
    vcx.run_until_parked();
    vcx.update(|window, _| window.refresh());
    vcx.run_until_parked();
    assert_ne!(blurred(&mut vcx), first, "the old blur is not reused");
}

#[gpui::test]
fn no_wallpaper_paints_nothing(cx: &mut gpui::TestAppContext) {
    let mut vcx = paint(cx, "{}");
    assert!(vcx.debug_bounds("wallpaper-canvas").is_none());
    vcx.update(|_, cx| assert!(!super::active(cx)));
}
