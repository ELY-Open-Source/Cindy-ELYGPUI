//! A looping, muted video decoded by AVFoundation into BGRA frames.

use std::{
    io::Write as _,
    path::Path,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result, bail};
use objc2::{rc::Retained, runtime::AnyObject};
use objc2_av_foundation::{
    AVAssetReader, AVAssetReaderStatus, AVAssetReaderTrackOutput, AVAssetTrack, AVMediaTypeVideo,
    AVURLAsset,
};
use objc2_core_foundation::CFString;
use objc2_core_media::{CMTime, CMTimeRange, kCMTimePositiveInfinity};
use objc2_core_video::{
    CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow, CVPixelBufferGetHeight,
    CVPixelBufferGetWidth, CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags,
    CVPixelBufferUnlockBaseAddress, kCVPixelBufferHeightKey, kCVPixelBufferPixelFormatTypeKey,
    kCVPixelBufferWidthKey, kCVPixelFormatType_32BGRA,
};
use objc2_foundation::{NSArray, NSDictionary, NSError, NSNumber, NSString, NSURL};

use super::{Event, Fit, Frame};

struct State {
    playing: bool,
    fit: Fit,
    refit: bool,
    stop: bool,
}

type Control = Arc<(Mutex<State>, Condvar)>;

pub struct Player {
    control: Control,
    thread: Option<std::thread::JoinHandle<()>>,
}

/// An NSError by domain and code; its text can name files.
fn reason(error: &NSError) -> String {
    format!("{} {}", error.domain(), error.code())
}

/// The bridged CoreVideo key as a Foundation string.
fn key(key: &CFString) -> &NSString {
    // SAFETY: CFString and NSString are toll-free bridged.
    unsafe { &*std::ptr::from_ref(key).cast::<NSString>() }
}

fn track(asset: &AVURLAsset) -> Result<Retained<AVAssetTrack>> {
    let (send, loaded) = std::sync::mpsc::channel();
    let block = block2::RcBlock::new(
        move |tracks: *mut NSArray<AVAssetTrack>, error: *mut NSError| {
            // SAFETY: AVFoundation passes live objects or null.
            let tracks = unsafe { Retained::retain(tracks) };
            let error = unsafe { Retained::retain(error) }.map(|e| reason(&e));
            let _ = send.send(tracks.and_then(|t| t.firstObject()).ok_or(error));
        },
    );
    let video = unsafe { AVMediaTypeVideo }.context("no video media type")?;
    // SAFETY: the block only sends on a channel.
    unsafe { asset.loadTracksWithMediaType_completionHandler(video, &block) };
    match loaded.recv().context("tracks never loaded")? {
        Ok(track) => Ok(track),
        Err(Some(reason)) => bail!("tracks: {reason}"),
        Err(None) => bail!("no video track"),
    }
}

fn reader(
    asset: &AVURLAsset,
    track: &AVAssetTrack,
    size: Option<(u32, u32)>,
    from: f64,
) -> Result<(Retained<AVAssetReader>, Retained<AVAssetReaderTrackOutput>)> {
    let bgra = NSNumber::numberWithUnsignedInt(kCVPixelFormatType_32BGRA);
    let mut keys = vec![key(unsafe { kCVPixelBufferPixelFormatTypeKey })];
    let mut values: Vec<Retained<AnyObject>> = vec![bgra.into()];
    if let Some((w, h)) = size {
        keys.extend([
            key(unsafe { kCVPixelBufferWidthKey }),
            key(unsafe { kCVPixelBufferHeightKey }),
        ]);
        values.extend([
            NSNumber::numberWithUnsignedInt(w).into(),
            NSNumber::numberWithUnsignedInt(h).into(),
        ]);
    }
    let values: Vec<&AnyObject> = values.iter().map(|v| &**v).collect();
    let settings = NSDictionary::from_slices(&keys, &values);
    // SAFETY: a video track, BGRA pixel buffer attributes, one reader per output.
    unsafe {
        let reader = AVAssetReader::assetReaderWithAsset_error(asset)
            .map_err(|e| anyhow::anyhow!(reason(&e)))?;
        let output = AVAssetReaderTrackOutput::assetReaderTrackOutputWithTrack_outputSettings(
            track,
            Some(&settings),
        );
        output.setAlwaysCopiesSampleData(false);
        reader.addOutput(&output);
        reader.setTimeRange(CMTimeRange {
            start: CMTime::with_seconds(from, 600),
            duration: kCMTimePositiveInfinity,
        });
        if !reader.startReading() {
            bail!(
                "reader did not start: {}",
                reader.error().map_or("no error".into(), |e| reason(&e))
            );
        }
        Ok((reader, output))
    }
}

/// The next frame and its time, or `None` at the end.
fn next(
    reader: &AVAssetReader,
    output: &AVAssetReaderTrackOutput,
) -> Result<Option<(f64, image::RgbaImage)>> {
    // SAFETY: an output of a reader that is reading.
    let Some(sample) = (unsafe { output.copyNextSampleBuffer() }) else {
        return match unsafe { reader.status() } {
            AVAssetReaderStatus::Completed => Ok(None),
            _ => bail!(
                "reading stopped: {}",
                unsafe { reader.error() }.map_or("no error".into(), |e| reason(&e))
            ),
        };
    };
    let at = unsafe { sample.presentation_time_stamp().seconds() };
    let buffer = unsafe { sample.image_buffer() }.context("sample without image")?;
    let read_only = CVPixelBufferLockFlags::ReadOnly;
    if unsafe { CVPixelBufferLockBaseAddress(&buffer, read_only) } != 0 {
        bail!("pixel buffer did not lock");
    }
    let (w, h) = (
        CVPixelBufferGetWidth(&buffer),
        CVPixelBufferGetHeight(&buffer),
    );
    let stride = CVPixelBufferGetBytesPerRow(&buffer);
    let base = CVPixelBufferGetBaseAddress(&buffer).cast::<u8>();
    // SAFETY: the locked buffer holds `h` rows of `stride` bytes.
    let data = unsafe { std::slice::from_raw_parts(base, stride * h) };
    let mut bgra = Vec::with_capacity(w * h * 4);
    for row in 0..h {
        bgra.extend_from_slice(&data[row * stride..row * stride + w * 4]);
    }
    unsafe { CVPixelBufferUnlockBaseAddress(&buffer, read_only) };
    let image = image::RgbaImage::from_raw(w as u32, h as u32, bgra).context("short frame")?;
    Ok(Some((at, image)))
}

/// The decode loop: paced by frame times, paused on request, restarted at the end.
fn run(file: &Path, control: &Control, send: &dyn Fn(Event)) -> Result<()> {
    let path = NSString::from_str(file.to_str().context("temp path is not UTF-8")?);
    // SAFETY: a file URL and no options.
    let asset =
        unsafe { AVURLAsset::URLAssetWithURL_options(&NSURL::fileURLWithPath(&path), None) };
    let track = track(&asset)?;
    let (mut native, mut from) = (None, 0.0);
    loop {
        let fit = control.0.lock().expect("state lock").fit;
        let (reader, output) = reader(&asset, &track, native.map(|n| fit.scaled(n)), from)?;
        let mut clock: Option<(Instant, f64)> = None;
        loop {
            let frame = objc2::rc::autoreleasepool(|_| next(&reader, &output))?;
            let Some((at, image)) = frame else {
                from = 0.0;
                break;
            };
            let dims = image.dimensions();
            let first = native.is_none();
            native.get_or_insert(dims);
            let image = if fit.sigma > 0.0 {
                image::imageops::fast_blur(&image, fit.frame_sigma(dims.0, native.expect("set")))
            } else {
                image
            };
            let (state, wake) = &**control;
            let mut state = state.lock().expect("state lock");
            let (start, zero) = *clock.get_or_insert((Instant::now(), at));
            let due = start + Duration::from_secs_f64((at - zero).max(0.0));
            while !state.stop && !state.refit && state.playing && Instant::now() < due {
                state = wake
                    .wait_timeout(state, due - Instant::now())
                    .expect("state lock")
                    .0;
            }
            if state.stop {
                return Ok(());
            }
            send(Event::Frame(Frame(image)));
            let paused = !state.playing;
            while !state.stop && !state.refit && !state.playing {
                state = wake.wait(state).expect("state lock");
            }
            if paused {
                clock = Some((Instant::now(), at));
            }
            if state.stop {
                return Ok(());
            }
            if first || std::mem::take(&mut state.refit) {
                from = at;
                break;
            }
        }
        // SAFETY: a reader this loop created.
        unsafe { reader.cancelReading() };
    }
}

impl Player {
    pub fn open(
        bytes: &[u8],
        fit: Fit,
        send: impl Fn(Event) + Send + Sync + 'static,
    ) -> Result<Self> {
        let dir = tempfile::Builder::new()
            .prefix("cindy-wallpaper-")
            .tempdir()?;
        let file = dir.path().join("video.mp4");
        std::fs::File::create_new(&file)?.write_all(bytes)?;
        let state = State {
            playing: false,
            fit,
            refit: false,
            stop: false,
        };
        let control: Control = Arc::new((Mutex::new(state), Condvar::new()));
        let shared = control.clone();
        let thread = std::thread::spawn(move || {
            if let Err(error) = run(&file, &shared, &send) {
                send(Event::Failed(format!("{error:#}")));
            }
            drop(dir);
        });
        Ok(Self {
            control,
            thread: Some(thread),
        })
    }

    pub fn play(&self, playing: bool) -> Result<()> {
        self.control.0.lock().expect("state lock").playing = playing;
        self.control.1.notify_all();
        Ok(())
    }

    pub fn fit(&self, fit: Fit) {
        let mut state = self.control.0.lock().expect("state lock");
        if state.fit != fit {
            state.fit = fit;
            state.refit = true;
            self.control.1.notify_all();
        }
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.control.0.lock().expect("state lock").stop = true;
        self.control.1.notify_all();
        if let Some(thread) = self.thread.take() {
            thread.join().expect("decoder thread panicked");
        }
    }
}
