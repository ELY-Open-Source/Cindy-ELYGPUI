//! A looping, muted video decoded by AVFoundation into BGRA frames.

use std::{io::Write as _, path::Path};

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

pub use super::paced::Player;
use super::{Event, Fit, paced::Decoder};

/// The checked bytes in a private temp file, read by one `AVAssetReader` at a time.
struct Reader {
    _dir: tempfile::TempDir,
    asset: Retained<AVURLAsset>,
    track: Retained<AVAssetTrack>,
    open: Option<(Retained<AVAssetReader>, Retained<AVAssetReaderTrackOutput>)>,
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

impl Reader {
    fn new(dir: tempfile::TempDir, file: &Path) -> Result<Self> {
        let path = NSString::from_str(file.to_str().context("temp path is not UTF-8")?);
        let url = NSURL::fileURLWithPath(&path);
        // SAFETY: a file URL and no options.
        let asset = unsafe { AVURLAsset::URLAssetWithURL_options(&url, None) };
        let track = track(&asset)?;
        Ok(Self {
            _dir: dir,
            asset,
            track,
            open: None,
        })
    }
}

impl Decoder for Reader {
    fn start(&mut self, size: Option<(u32, u32)>, from: f64) -> Result<()> {
        if let Some((reader, _)) = self.open.take() {
            // SAFETY: a reader this decoder started.
            unsafe { reader.cancelReading() };
        }
        self.open = Some(reader(&self.asset, &self.track, size, from)?);
        Ok(())
    }

    fn next(&mut self) -> Result<Option<(f64, image::RgbaImage)>> {
        let (reader, output) = self.open.as_ref().expect("started");
        objc2::rc::autoreleasepool(|_| next(reader, output))
    }
}

/// AVFoundation reads a URL, so the checked bytes go to a private temp directory.
pub fn open(
    bytes: &[u8],
    fit: Fit,
    send: impl Fn(Event) + Send + Sync + 'static,
) -> Result<Player> {
    let dir = tempfile::Builder::new()
        .prefix("cindy-wallpaper-")
        .tempdir()?;
    let file = dir.path().join("video.mp4");
    std::fs::File::create_new(&file)?.write_all(bytes)?;
    Ok(Player::spawn(fit, send, move || Reader::new(dir, &file)))
}
