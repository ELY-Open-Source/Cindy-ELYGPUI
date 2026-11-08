//! `WallpaperVideo.tsx`: one muted, looping decoder behind the window, under the veil.

#[cfg(target_os = "linux")]
mod gst;
#[cfg(target_os = "linux")]
use gst::Player;
#[cfg(target_os = "macos")]
mod avf;
#[cfg(target_os = "macos")]
use avf::Player;
#[cfg(target_os = "windows")]
mod none;
#[cfg(target_os = "windows")]
use none::Player;

use std::{sync::Arc, time::Instant};

use anyhow::{Context as _, Result};
use gpui::{
    AnyElement, App, IntoElement, ObjectFit, ParentElement, RenderImage, Styled, StyledImage as _,
    Window, div, img,
};

use super::Motion;
use crate::motion::{self, ReducedMotion, Tween};
use crate::theme::{Appearance, Kind};

/// A decoded frame, its bytes already in gpui's BGRA order.
pub struct Frame(pub image::RgbaImage);

pub enum Event {
    Frame(Frame),
    Failed(String),
}

/// The box the video covers and its blur, in device pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fit {
    pub target: (u32, u32),
    pub sigma: f32,
}

impl Fit {
    fn cover(&self, native: (u32, u32)) -> f32 {
        let (w, h) = (self.target.0 as f32, self.target.1 as f32);
        f32::max(w / native.0 as f32, h / native.1 as f32)
    }

    /// The decode size: the cover size, never past the source, coarser as the blur widens.
    pub fn scaled(&self, native: (u32, u32)) -> (u32, u32) {
        let step = (self.sigma / 2.0).max(1.0);
        let scale = self.cover(native).min(1.0) / step;
        let len = |n: u32| ((n as f32 * scale).ceil() as u32).max(1);
        (len(native.0), len(native.1))
    }

    /// The blur in pixels of a frame `width` wide decoded from a `native` source.
    pub fn frame_sigma(&self, width: u32, native: (u32, u32)) -> f32 {
        self.sigma * width as f32 / (native.0 as f32 * self.cover(native))
    }
}

/// The bytes the decoder plays, as they were read and checked: never reopened by path.
#[derive(Clone, PartialEq)]
pub enum Source {
    Custom(Arc<Vec<u8>>),
    Official(Arc<Vec<u8>>),
}

impl std::fmt::Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (kind, bytes) = match self {
            Self::Custom(bytes) => ("custom", bytes),
            Self::Official(bytes) => ("official", bytes),
        };
        write!(f, "{kind} video, {} bytes", bytes.len())
    }
}

pub struct Video {
    pub source: Option<Source>,
    player: Option<Player>,
    /// Counts players, so a dropped one's late frames are ignored.
    generation: u32,
    playing: bool,
    failed: bool,
    fit: Option<Fit>,
    frame: Option<Arc<RenderImage>>,
    retired: Vec<Arc<RenderImage>>,
    shown: Tween,
}

impl gpui::Global for Video {}

impl Video {
    pub fn new() -> Self {
        Self {
            source: None,
            player: None,
            generation: 0,
            playing: false,
            failed: false,
            fit: None,
            frame: None,
            retired: Vec::new(),
            shown: Tween::still(0.0, Instant::now()),
        }
    }

    fn stop(&mut self) {
        self.player = None;
        self.generation += 1;
        self.retired.extend(self.frame.take());
        self.shown = Tween::still(0.0, Instant::now());
    }

    fn fail(&mut self, reason: &str) {
        log::warn!(target: "wallpaper-video", "Wallpaper media failed: {reason}");
        self.failed = true;
        self.stop();
    }
}

/// `<WallpaperVideo>` and its hidden gate: whether a decoder exists, and whether it plays.
fn wanted(source: &Source, motion: Motion, reduced: bool, visible: bool) -> Option<bool> {
    let plays = match source {
        Source::Custom(_) => motion == Motion::Dynamic && !reduced,
        Source::Official(_) if reduced || motion == Motion::Static => return None,
        Source::Official(_) => true,
    };
    Some(plays && visible)
}

fn open(source: &Source, fit: Fit, generation: u32, cx: &mut App) -> Result<Player> {
    let (bytes, kind) = match source {
        Source::Custom(bytes) => (bytes, "custom"),
        Source::Official(bytes) => (bytes, "official"),
    };
    let (send, mut events) = futures::channel::mpsc::unbounded();
    let player = Player::open(bytes, fit, move |event| {
        let _ = send.unbounded_send(event);
    })
    .with_context(|| format!("{kind} wallpaper video"))?;
    log::info!(target: "wallpaper-video", "{kind} wallpaper video opened");
    cx.spawn(async move |cx| {
        while let Some(event) = futures::StreamExt::next(&mut events).await {
            cx.update(|cx| receive(event, generation, cx));
        }
    })
    .detach();
    Ok(player)
}

fn receive(event: Event, generation: u32, cx: &mut App) {
    let length = motion::length(motion::BASE, cx);
    let video = cx.global_mut::<Video>();
    if generation != video.generation {
        return;
    }
    match event {
        Event::Frame(Frame(image)) => {
            if video.frame.is_none() {
                video.shown.toward(1.0, Instant::now(), length);
            }
            let image = Arc::new(RenderImage::new(vec![image::Frame::new(image)]));
            video.retired.extend(video.frame.replace(image));
        }
        Event::Failed(reason) => video.fail(&reason),
    }
    cx.refresh_windows();
}

/// Starts, pauses or drops the decoder for the settings, reduced motion and visibility; a
/// render passes the box, a visibility change reuses the last one.
pub fn sync(window: &Window, fit: Option<Fit>, cx: &mut App) {
    let video = cx.global_mut::<Video>();
    video.fit = fit.or(video.fit);
    let (Some(source), Some(fit)) = (video.source.clone(), video.fit) else {
        return;
    };
    let wallpaper = cx.global::<super::Wallpaper>();
    let motion_setting = wallpaper.settings.motion;
    let dark = cx.global::<Appearance>().kind() == Kind::Dark;
    let shown = wallpaper.settings.visibility(dark) > 0.0;
    let reduced = cx.global::<ReducedMotion>().0;
    let video = cx.global_mut::<Video>();
    let want = wanted(&source, motion_setting, reduced, window.is_visible());
    let Some(playing) = want.filter(|_| shown && !video.failed) else {
        if video.player.is_some() {
            video.stop();
        }
        return;
    };
    if let Some(player) = &video.player {
        player.fit(fit);
        if video.playing != playing {
            video.playing = playing;
            if let Err(error) = player.play(playing) {
                video.fail(&format!("playback rejected: {error:#}"));
            }
        }
        return;
    }
    let generation = video.generation + 1;
    let opened = open(&source, fit, generation, cx).and_then(|player| {
        player.play(playing)?;
        Ok(player)
    });
    let video = cx.global_mut::<Video>();
    match opened {
        Ok(player) => {
            video.generation = generation;
            video.player = Some(player);
            video.playing = playing;
        }
        Err(error) => video.fail(&format!("{error:#}")),
    }
}

/// `.app-wallpaper-video`: the surface, then the frame at the wallpaper's visibility.
pub fn layer(
    window: &mut Window,
    placed: impl FnOnce(gpui::Img) -> AnyElement,
    cx: &mut App,
) -> Option<AnyElement> {
    let video = cx.global_mut::<Video>();
    for image in video.retired.drain(..) {
        window.drop_image(image).expect("drop a retired frame");
    }
    let frame = video.frame.clone()?;
    let (shown, moving) = video.shown.at(Instant::now());
    if moving {
        window.request_animation_frame();
    }
    let appearance = cx.global::<Appearance>();
    let dark = appearance.kind() == Kind::Dark;
    let visibility = cx.global::<super::Wallpaper>().settings.visibility(dark) as f32;
    let picture = img(frame).object_fit(ObjectFit::Cover).opacity(visibility);
    Some(
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .overflow_hidden()
            .bg(appearance.token("surface"))
            .opacity(shown)
            .child(placed(picture))
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests;
