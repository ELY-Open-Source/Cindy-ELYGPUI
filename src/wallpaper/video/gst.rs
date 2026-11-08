//! A looping, muted video decoded by GStreamer into BGRA frames.

use std::{
    io::{Seek as _, SeekFrom, Write as _},
    os::fd::{AsRawFd as _, FromRawFd as _, OwnedFd},
    sync::{Arc, Mutex},
};

use anyhow::{Context as _, Result, anyhow};
use gstreamer::{self as gst, glib, prelude::*};
use gstreamer_app::{AppSink, AppSinkCallbacks};
use gstreamer_video::{VideoFrameExt as _, VideoFrameRef, VideoInfo};

use super::{Event, Fit};

pub struct Player {
    pipeline: gst::Pipeline,
    _memory: OwnedFd,
    size: gst::Element,
    fit: Arc<Mutex<Fit>>,
    native: Arc<Mutex<Option<(u32, u32)>>>,
    bus: Option<std::thread::JoinHandle<()>>,
}

fn caps(size: Option<(u32, u32)>) -> gst::Caps {
    let caps = gst::Caps::builder("video/x-raw")
        .field("format", "BGRA")
        .field("pixel-aspect-ratio", gst::Fraction::new(1, 1));
    match size {
        Some((w, h)) => caps
            .field("width", w as i32)
            .field("height", h as i32)
            .build(),
        None => caps.build(),
    }
}

/// One frame, tightly packed; blurred here, off the main thread.
fn frame(sample: &gst::Sample, fit: &Fit, native: Option<(u32, u32)>) -> Result<super::Frame> {
    let caps = sample.caps().context("sample without caps")?;
    let info = VideoInfo::from_caps(caps)?;
    let buffer = sample.buffer().context("sample without buffer")?;
    let video = VideoFrameRef::from_buffer_ref_readable(buffer, &info)?;
    let (w, h) = (video.width(), video.height());
    let stride = video.plane_stride()[0] as usize;
    let data = video.plane_data(0)?;
    let mut bgra = Vec::with_capacity((w * h * 4) as usize);
    for row in 0..h as usize {
        bgra.extend_from_slice(&data[row * stride..row * stride + w as usize * 4]);
    }
    let image = image::RgbaImage::from_raw(w, h, bgra).context("short frame")?;
    let image = if fit.sigma > 0.0 {
        image::imageops::fast_blur(&image, fit.frame_sigma(w, native.unwrap_or((w, h))))
    } else {
        image
    };
    Ok(super::Frame(image))
}

/// The checked bytes in a sealed memory file, so what plays is what was checked.
fn sealed(bytes: &[u8]) -> Result<OwnedFd> {
    let flags = libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING;
    // SAFETY: a NUL-terminated name; the result is checked before use.
    let fd = unsafe { libc::memfd_create(c"cindy-wallpaper-video".as_ptr(), flags) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error()).context("memfd_create");
    }
    // SAFETY: a new descriptor this function owns.
    let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
    file.write_all(bytes)?;
    file.seek(SeekFrom::Start(0))?;
    let seals = libc::F_SEAL_WRITE | libc::F_SEAL_SHRINK | libc::F_SEAL_GROW | libc::F_SEAL_SEAL;
    // SAFETY: fcntl on the descriptor `file` owns.
    if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_ADD_SEALS, seals) } < 0 {
        return Err(std::io::Error::last_os_error()).context("seal");
    }
    Ok(file.into())
}

/// A GStreamer error by domain and code; its text can name files.
fn reason(error: &glib::Error) -> String {
    format!("{} {}", error.domain().as_str(), error.code())
}

impl Player {
    pub fn open(
        bytes: &[u8],
        fit: Fit,
        send: impl Fn(Event) + Send + Sync + 'static,
    ) -> Result<Self> {
        gst::init()?;
        let memory = sealed(bytes)?;
        let description = "fdsrc name=src ! decodebin ! videoconvert ! videoscale \
                           ! capsfilter name=size ! appsink name=sink sync=true max-buffers=1 drop=true";
        let pipeline = gst::parse::launch(description)?
            .downcast::<gst::Pipeline>()
            .map_err(|_| anyhow!("not a pipeline"))?;
        let by_name = |name| pipeline.by_name(name).with_context(|| format!("no {name}"));
        by_name("src")?.set_property("fd", memory.as_raw_fd());
        let size = by_name("size")?;
        size.set_property("caps", caps(None));
        let fit = Arc::new(Mutex::new(fit));
        let native = Arc::new(Mutex::new(None));
        let send = Arc::new(send);
        let sink = by_name("sink")?
            .downcast::<AppSink>()
            .map_err(|_| anyhow!("not an appsink"))?;
        let deliver = {
            let (fit, native, send, size) =
                (fit.clone(), native.clone(), send.clone(), size.clone());
            move |sample: gst::Sample| {
                let current = *fit.lock().expect("fit lock");
                let known = *native.lock().expect("native lock");
                match frame(&sample, &current, known) {
                    Ok(frame) => {
                        if known.is_none() {
                            let dims = frame.0.dimensions();
                            *native.lock().expect("native lock") = Some(dims);
                            size.set_property("caps", caps(Some(current.scaled(dims))));
                        }
                        send(Event::Frame(frame));
                        Ok(gst::FlowSuccess::Ok)
                    }
                    Err(error) => {
                        send(Event::Failed(format!("{error:#}")));
                        Err(gst::FlowError::Error)
                    }
                }
            }
        };
        let preroll = deliver.clone();
        sink.set_callbacks(
            AppSinkCallbacks::builder()
                .new_preroll(move |sink| {
                    preroll(sink.pull_preroll().map_err(|_| gst::FlowError::Eos)?)
                })
                .new_sample(move |sink| {
                    deliver(sink.pull_sample().map_err(|_| gst::FlowError::Eos)?)
                })
                .build(),
        );
        let bus = pipeline.bus().context("pipeline without bus")?;
        let looping = pipeline.downgrade();
        let watcher = std::thread::spawn(move || {
            use gst::MessageView;
            for message in bus.iter_timed(gst::ClockTime::NONE) {
                match message.view() {
                    MessageView::Eos(_) => {
                        let Some(pipeline) = looping.upgrade() else {
                            return;
                        };
                        let flags = gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT;
                        if pipeline.seek_simple(flags, gst::ClockTime::ZERO).is_err() {
                            send(Event::Failed("loop seek failed".into()));
                        }
                    }
                    MessageView::Error(error) => send(Event::Failed(reason(&error.error()))),
                    MessageView::Application(_) => return,
                    _ => {}
                }
            }
        });
        Ok(Self {
            pipeline,
            _memory: memory,
            size,
            fit,
            native,
            bus: Some(watcher),
        })
    }

    /// `play()` or `pause()`; a paused video still shows its current frame.
    pub fn play(&self, playing: bool) -> Result<()> {
        let state = if playing {
            gst::State::Playing
        } else {
            gst::State::Paused
        };
        self.pipeline.set_state(state)?;
        Ok(())
    }

    /// New box: the cover size changes, and with it the blur's scale.
    pub fn fit(&self, fit: Fit) {
        *self.fit.lock().expect("fit lock") = fit;
        if let Some(dims) = *self.native.lock().expect("native lock") {
            self.size.set_property("caps", caps(Some(fit.scaled(dims))));
        }
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        if let Err(error) = self.pipeline.set_state(gst::State::Null) {
            log::warn!(target: "wallpaper-video", "pipeline did not stop: {error}");
        }
        let bus = self.pipeline.bus().expect("pipeline bus");
        let stop = gst::Structure::new_empty("stop");
        if bus.post(gst::message::Application::new(stop)).is_ok()
            && let Some(watcher) = self.bus.take()
        {
            watcher.join().expect("bus watcher panicked");
        }
    }
}
