//! The bundled previews, `readClientWallpaperFile`, and images made ready to paint.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context as _, Result, bail};
use gpui::RenderImage;
use image::{RgbaImage, imageops};

macro_rules! previews {
    ($($id:literal => $file:literal),* $(,)?) => {
        /// `getBuiltinWallpaperBackground`: official ids show their bundled preview.
        pub fn preview(id: &str) -> Option<&'static [u8]> {
            match id {
                $($id => Some(include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"), "/resources/wallpapers/", $file
                ))),)*
                _ => None,
            }
        }
    };
}

previews! {
    "cindy-window" => "previews/cindy-window.webp",
    "cindy-studio" => "previews/cindy-studio.webp",
    "cindy-dream" => "previews/cindy-dream.webp",
    "cindy-whisper" => "thumbnails/cindy-whisper.webp",
    "cindy-neon" => "thumbnails/cindy-neon.webp",
    "cindy-prism" => "thumbnails/cindy-prism.webp",
    "cindy-night-glance" => "thumbnails/cindy-night-glance.webp",
    "cindy-scarlet-stroll" => "thumbnails/cindy-scarlet-stroll.webp",
    "night-launch" => "thumbnails/night-launch.webp",
    "silent-orbits" => "thumbnails/silent-orbits.webp",
    "dune-dawn" => "thumbnails/dune-dawn.webp",
    "planet-dawn" => "thumbnails/planet-dawn.webp",
    "indigo-folds" => "thumbnails/indigo-folds.webp",
    "dune-sphere" => "thumbnails/dune-sphere.webp",
    "ocean-clouds" => "thumbnails/ocean-clouds.webp",
    "rose-quartz" => "thumbnails/rose-quartz.webp",
}

fn regular_dir(path: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(path).context("client wallpaper store unreadable")?;
    if meta.file_type().is_symlink() {
        bail!("cindy-media: blob ancestor is a symlink");
    }
    if !meta.is_dir() {
        bail!("cindy-media: blob ancestor is not a directory");
    }
    Ok(())
}

/// `resolveHashRef` and `assertBlobPathContained` for a normalized client-wallpaper URL.
pub fn custom_path(user_data: &Path, url: &str) -> Result<PathBuf> {
    let name = url
        .strip_prefix("cindy-media://client-wallpaper/")
        .context("cindy-media: invalid client wallpaper url")?;
    let root = fs::canonicalize(user_data).context("userData")?;
    regular_dir(&root)?;
    let path = root
        .join("cindy-media/client-wallpaper")
        .join(&name[..2])
        .join(name);
    let meta = fs::symlink_metadata(&path).context("custom wallpaper missing")?;
    if meta.file_type().is_symlink() {
        bail!("cindy-media: blob destination is a symlink");
    }
    for parent in path.ancestors().skip(1).take_while(|p| *p != root) {
        regular_dir(parent)?;
    }
    Ok(path)
}

fn manifest() -> serde_json::Value {
    let text = include_str!("../../resources/wallpaper-video-manifest.json");
    serde_json::from_str(text).expect("the bundled manifest parses")
}

/// `resolveHashRef(sha256, ext, 'client-wallpaper')`.
fn cache_path(user_data: &Path, sha256: &str, ext: &str) -> PathBuf {
    let dir = user_data
        .join("cindy-media/client-wallpaper")
        .join(&sha256[..2]);
    dir.join(format!("{sha256}{ext}"))
}

/// The original image of an official wallpaper, as `ensureWallpaperAsset` checks its cache.
pub fn cached_original(user_data: &Path, id: &str) -> Result<Option<RgbaImage>> {
    let manifest = manifest();
    let asset = &manifest[id]["image"];
    let (Some(sha256), Some(size)) = (asset["sha256"].as_str(), asset["bytes"].as_u64()) else {
        bail!("{id} has no original image in the manifest");
    };
    let path = cache_path(user_data, sha256, ".webp");
    verified(&path, sha256, size)
        .map(|b| decode(&b))
        .transpose()
}

/// The cached video of a scene wallpaper (`isSceneWallpaper`): the bytes that verified.
pub fn cached_video(user_data: &Path, id: &str) -> Option<Vec<u8>> {
    let manifest = manifest();
    let asset = &manifest[id];
    let (sha256, size) = (asset["sha256"].as_str()?, asset["bytes"].as_u64()?);
    verified(&cache_path(user_data, sha256, ".mp4"), sha256, size)
}

/// `MAX_VIDEO_BYTES`: the largest custom media the picker takes.
const CUSTOM_VIDEO_MAX: u64 = 100 * 1024 * 1024;

/// `openVerifiedBlobFile` then a read of that handle: the custom media's bytes.
pub fn custom_media(user_data: &Path, url: &str) -> Result<Vec<u8>> {
    read_contained(user_data, url, || {})
}

/// The path is contained and names the opened file both before and after the open.
pub(super) fn read_contained(
    user_data: &Path,
    url: &str,
    opening: impl FnOnce(),
) -> Result<Vec<u8>> {
    use crate::bounded_read::{identity, open_no_follow, path_identity, read_file};
    let path = custom_path(user_data, url)?;
    let named = || path_identity(&path);
    let before = named().context("custom wallpaper unreadable")?;
    opening();
    let file = open_no_follow(&path).context("custom wallpaper unreadable")?;
    let handle = identity(&file)?;
    custom_path(user_data, url)?;
    let after = named().context("custom wallpaper unreadable")?;
    if before != handle || after != handle {
        bail!("cindy-media: media changed while opening");
    }
    let bytes = read_file(file, CUSTOM_VIDEO_MAX).context("custom wallpaper unreadable")?;
    bytes.context("custom wallpaper is not a file within the limit")
}

/// `matchesWallpaperVideo` over a bounded no-follow read; anything else is not cached.
pub fn verified(path: &Path, sha256: &str, size: u64) -> Option<Vec<u8>> {
    let bytes = crate::bounded_read::read(path, size).ok()??;
    let digest = format!("{:x}", <sha2::Sha256 as sha2::Digest>::digest(&bytes));
    (bytes.len() as u64 == size && digest == sha256).then_some(bytes)
}

pub fn decode(bytes: &[u8]) -> Result<RgbaImage> {
    Ok(image::load_from_memory(bytes)?.into_rgba8())
}

/// The part of `image` that `background-size: cover` shows in `box_px`, blurred by `sigma` box
/// pixels. It stays at most at source resolution, and coarser as the blur widens; gpui stretches
/// it over the box.
pub fn blurred(image: &RgbaImage, box_px: (u32, u32), sigma: f32) -> RgbaImage {
    let (w, h) = (box_px.0 as f32, box_px.1 as f32);
    let (iw, ih) = (image.width() as f32, image.height() as f32);
    let scale = f32::max(w / iw, h / ih);
    let (cw, ch) = ((w / scale).round().min(iw), (h / scale).round().min(ih));
    let crop = imageops::crop_imm(
        image,
        ((iw - cw) / 2.0) as u32,
        ((ih - ch) / 2.0) as u32,
        cw as u32,
        ch as u32,
    );
    let step = (sigma / 2.0).max(1.0);
    let (tw, th) = ((w / step).ceil().min(cw), (h / step).ceil().min(ch));
    let small = imageops::resize(&crop.to_image(), tw as u32, th as u32, imageops::Triangle);
    imageops::fast_blur(&small, sigma * tw / w)
}

/// gpui wants BGRA frames.
pub fn render(mut image: RgbaImage) -> Arc<RenderImage> {
    for pixel in image.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    Arc::new(RenderImage::new(vec![image::Frame::new(image)]))
}
