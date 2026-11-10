//! `useWallpaperBackground` and its CSS: one canvas behind the window, under a theme veil.

use std::{path::Path, sync::Arc};

use anyhow::Result;
use gpui::{
    AnyElement, App, BorrowAppContext as _, Hsla, InteractiveElement as _, IntoElement, ObjectFit,
    ParentElement, RenderImage, Styled, StyledImage as _, Window, div, img,
};
use image::RgbaImage;

use super::{Motion, Settings, media, video};
use crate::theme::{Appearance, Kind};
use crate::units;

/// What `background` holds: an image, or `none` under a custom video.
#[derive(Clone, Debug, PartialEq)]
enum Background {
    Preview(&'static str),
    Custom(String),
    Video,
}

fn background(settings: &Settings) -> Option<Background> {
    if settings.id == "custom" {
        let video = settings.custom_url.ends_with(".mp4");
        return Some(if video {
            Background::Video
        } else {
            Background::Custom(settings.custom_url.clone())
        });
    }
    media::preview(settings.id).map(|_| Background::Preview(settings.id))
}

/// A blurred canvas for one overscanned box, in device pixels.
pub(super) struct Blurred {
    key: (u32, u32, u32),
    version: u32,
    pub(super) image: Arc<RenderImage>,
}

pub struct Wallpaper {
    pub settings: Settings,
    active: bool,
    source: Option<Arc<RgbaImage>>,
    /// Counts source images, so a new one blurs again.
    version: u32,
    pub(super) image: Option<Arc<RenderImage>>,
    pub(super) blurred: Option<Blurred>,
    blurring: Option<(u32, u32, u32)>,
}

impl gpui::Global for Wallpaper {}

fn load(background: &Background, user_data: &Path) -> Result<RgbaImage> {
    match background {
        Background::Preview(id) => media::decode(media::preview(id).expect("checked")),
        Background::Custom(url) => media::decode(&media::custom_media(user_data, url)?),
        Background::Video => unreachable!("a video has no image"),
    }
}

impl Wallpaper {
    /// Reads the image off the main thread; until then the veiled surface shows.
    pub fn install(settings: Settings, user_data: &Path, cx: &mut App) {
        let background = background(&settings);
        let kind = match &background {
            None => "none",
            Some(Background::Preview(_)) => "bundled preview",
            Some(Background::Custom(_)) => "custom image",
            Some(Background::Video) => "custom video",
        };
        log::info!(target: "wallpaper", "wallpaper {}: {kind}", settings.id);
        cx.set_global(Self {
            active: background.is_some(),
            settings,
            source: None,
            version: 0,
            image: None,
            blurred: None,
            blurring: None,
        });
        cx.set_global(video::Video::new());
        install_video(&background, user_data, cx);
        let Some(background) = background.filter(|b| *b != Background::Video) else {
            return;
        };
        let user_data = user_data.to_owned();
        cx.spawn(async move |cx| {
            let executor = cx.background_executor().clone();
            let (first, dir) = (background.clone(), user_data.clone());
            let loaded = executor.spawn(async move { load(&first, &dir) });
            let loaded = loaded.await;
            let shown = cx.update(|cx| match loaded {
                Ok(source) => {
                    cx.update_global::<Self, _>(|w, cx| w.show(source, cx));
                    true
                }
                Err(error) => {
                    log::warn!(target: "wallpaper", "wallpaper image not shown: {error:#}");
                    false
                }
            });
            let Background::Preview(id) = background else { return };
            if !shown || !cx.update(wants_original) {
                return;
            }
            let original = executor.spawn(async move { media::cached_original(&user_data, id) });
            let original = original.await;
            cx.update(|cx| match original {
                Ok(Some(source)) => {
                    log::info!(target: "wallpaper-assets", "{id}: original from the cache");
                    cx.update_global::<Self, _>(|w, cx| w.show(source, cx));
                }
                Ok(None) => {
                    log::info!(target: "wallpaper-assets", "{id}: original not cached; keeping the preview")
                }
                Err(error) => {
                    log::warn!(target: "wallpaper-assets", "{id}: original unreadable; keeping the preview: {error:#}")
                }
            });
        })
        .detach();
    }

    pub(super) fn show(&mut self, source: RgbaImage, cx: &mut App) {
        self.image = Some(media::render(source.clone()));
        self.source = Some(Arc::new(source));
        self.version += 1;
        cx.refresh_windows();
    }

    /// The blurred canvas for this box, started off the main thread when it is missing.
    fn blurred_for(&mut self, key: (u32, u32, u32), cx: &mut App) -> Option<Arc<RenderImage>> {
        let image = self.blurred.as_ref().map(|b| b.image.clone());
        let version = self.version;
        let fresh = self
            .blurred
            .as_ref()
            .is_some_and(|b| b.key == key && b.version == version);
        let source = self.source.clone();
        if fresh || self.blurring.is_some() || source.is_none() {
            return image;
        }
        self.blurring = Some(key);
        let source = source.expect("checked above");
        let (w, h, sigma) = key;
        let task = cx.background_executor().spawn(async move {
            media::render(media::blurred(&source, (w, h), sigma as f32 / 100.0))
        });
        cx.spawn(async move |cx| {
            let image = task.await;
            cx.update(|cx| {
                let wallpaper = cx.global_mut::<Self>();
                wallpaper.blurring = None;
                wallpaper.blurred = Some(Blurred {
                    key,
                    version,
                    image,
                });
                cx.refresh_windows();
            });
        })
        .detach();
        image
    }
}

/// The video `<WallpaperVideo>` may play: the custom one, or a scene wallpaper's cached video
/// when the motion is dynamic. Found off the main thread.
fn install_video(background: &Option<Background>, user_data: &Path, cx: &mut App) {
    let settings = &cx.global::<Wallpaper>().settings;
    let official =
        matches!(background, Some(Background::Preview(_))) && settings.motion == Motion::Dynamic;
    if background != &Some(Background::Video) && !official {
        return;
    }
    let (id, url, user_data) = (
        settings.id,
        settings.custom_url.clone(),
        user_data.to_owned(),
    );
    let task = cx.background_executor().spawn(async move {
        anyhow::Ok(if id == "custom" {
            let bytes = media::custom_media(&user_data, &url)?;
            Some(video::Source::Custom(Arc::new(bytes)))
        } else {
            let bytes = media::cached_video(&user_data, id);
            bytes.map(|b| video::Source::Official(Arc::new(b)))
        })
    });
    cx.spawn(async move |cx| {
        let found = task.await;
        cx.update(|cx| match found {
            Ok(Some(source)) => {
                cx.global_mut::<video::Video>().source = Some(source);
                cx.refresh_windows();
            }
            Ok(None) => log::info!(target: "wallpaper-assets", "{id}: video not cached"),
            Err(error) => {
                log::warn!(target: "wallpaper-video", "Wallpaper media failed: {error:#}")
            }
        });
    })
    .detach();
}

/// `useWallpaperAsset(id, 'image', visibility > 0)`.
fn wants_original(cx: &mut App) -> bool {
    let dark = cx.global::<Appearance>().kind() == Kind::Dark;
    cx.global::<Wallpaper>().settings.visibility(dark) > 0.0
}

/// `data-wallpaper-active`: a background is set, so the canvas covers the window.
pub fn active(cx: &App) -> bool {
    cx.global::<Wallpaper>().active
}

/// A page surface the canvas shows through (`.bg-content-area`, docked sidebar and the like).
pub fn clear(color: Hsla, cx: &App) -> Hsla {
    if active(cx) {
        gpui::transparent_black()
    } else {
        color
    }
}

/// `GLASS_MAX_WALLPAPER_SHARE`.
const GLASS_SHARE: f64 = 0.16;

/// An in-page raised surface: under a wallpaper Cindy's own themes tint it with its fill.
pub fn glass(fill: Hsla, cx: &App) -> Hsla {
    let appearance = cx.global::<Appearance>();
    if !active(cx) || !crate::window_material::cindy_theme(appearance) {
        return fill;
    }
    let dark = appearance.kind() == Kind::Dark;
    let visibility = cx.global::<Wallpaper>().settings.visibility(dark);
    let floor = match visibility > 0.0 {
        true => ((1.0 - GLASS_SHARE / visibility).max(0.0) * 100.0).ceil(),
        false => 0.0,
    };
    let share = floor.max(if dark { 64.0 } else { 72.0 }) / 100.0;
    fill.opacity(share as f32)
}

/// `body::before` and `.app-wallpaper-video`: the surface, the cover image, the veil, then the
/// video. A blur overscans the box by three radii, as the CSS does.
pub fn canvas(window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    if !active(cx) {
        return None;
    }
    let appearance = cx.global::<Appearance>();
    let surface = appearance.token("surface");
    let dark = appearance.kind() == Kind::Dark;
    let wallpaper = cx.global::<Wallpaper>();
    let visibility = wallpaper.settings.visibility(dark);
    let blur = wallpaper.settings.blur.unwrap_or(0.0) as f32;
    let blurred = blur > 0.0 && visibility > 0.0;
    let sharp = wallpaper.image.clone();
    let overscan = units::window_px(if blurred { 3.0 * blur } else { 0.0 }, cx);
    let viewport = window.viewport_size();
    let (w, h) = (
        viewport.width + overscan * 2.0,
        viewport.height + overscan * 2.0,
    );
    let device = |len: gpui::Pixels| (f32::from(len) * window.scale_factor()).ceil() as u32;
    let sigma = if blurred {
        f32::from(units::window_px(blur, cx)) * window.scale_factor()
    } else {
        0.0
    };
    let fit = video::Fit {
        target: (device(w), device(h)),
        sigma,
    };
    video::sync(window, Some(fit), cx);
    let place = move |image: gpui::Img, fit: ObjectFit| {
        let placed = image.absolute().left(-overscan).top(-overscan).w(w).h(h);
        placed.object_fit(fit).into_any_element()
    };
    let image = if blurred {
        let key = (fit.target.0, fit.target.1, (sigma * 100.0).round() as u32);
        let image = cx.update_global::<Wallpaper, _>(|w, cx| w.blurred_for(key, cx));
        image.map(|image| place(img(image), ObjectFit::Fill))
    } else {
        sharp.map(|image| place(img(image), ObjectFit::Cover))
    };
    let veil = surface.opacity(1.0 - visibility as f32);
    let layer = || div().absolute().top_0().left_0().size_full();
    let moving = video::layer(window, |picture| place(picture, ObjectFit::Cover), cx);
    Some(
        layer()
            .debug_selector(|| "wallpaper-canvas".into())
            .overflow_hidden()
            .bg(surface)
            .children(image)
            .child(layer().bg(veil))
            .children(moving)
            .into_any_element(),
    )
}
