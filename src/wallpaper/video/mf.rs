//! A looping, muted video decoded by Media Foundation into BGRA frames, from memory.

use anyhow::{Context as _, Result, bail};
use windows::Win32::{
    Media::MediaFoundation::{
        IMFMediaType, IMFSourceReader, MF_MT_DEFAULT_STRIDE, MF_MT_FRAME_SIZE, MF_MT_MAJOR_TYPE,
        MF_MT_SUBTYPE, MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING,
        MF_SOURCE_READER_FIRST_VIDEO_STREAM, MF_SOURCE_READERF_ENDOFSTREAM, MF_VERSION,
        MFCreateAttributes, MFCreateMFByteStreamOnStream, MFCreateMediaType,
        MFCreateSourceReaderFromByteStream, MFMediaType_Video, MFSTARTUP_FULL, MFShutdown,
        MFStartup, MFVideoFormat_RGB32,
    },
    System::{
        Com::{
            COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize, StructuredStorage::PROPVARIANT,
        },
        Variant::VT_I8,
    },
    UI::Shell::SHCreateMemStream,
};

pub use super::paced::Player;
use super::{Event, Fit, paced::Decoder};

const STREAM: u32 = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;

/// A Media Foundation error by its HRESULT; its text can name files.
fn reason(error: &windows::core::Error) -> String {
    format!("HRESULT {:#010x}", error.code().0)
}

/// One successful `CoInitializeEx`, undone on drop.
struct Com;

impl Drop for Com {
    fn drop(&mut self) {
        // SAFETY: pairs the initialization this guard records, on the same thread.
        unsafe { CoUninitialize() };
    }
}

/// One successful `MFStartup`, undone on drop.
struct MediaFoundation;

impl Drop for MediaFoundation {
    fn drop(&mut self) {
        // SAFETY: pairs the startup this guard records.
        if let Err(error) = unsafe { MFShutdown() } {
            log::warn!(target: "wallpaper-video", "MFShutdown failed: {}", reason(&error));
        }
    }
}

/// Fields drop in order: the reader, then Media Foundation, then COM.
struct Reader {
    reader: IMFSourceReader,
    _media: MediaFoundation,
    _com: Com,
}

impl Reader {
    fn new(bytes: &[u8]) -> Result<Self> {
        // SAFETY: COM and Media Foundation start on this thread, which owns the reader.
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED)
                .ok()
                .map_err(|e| anyhow::anyhow!(reason(&e)))?;
            let com = Com;
            MFStartup(MF_VERSION, MFSTARTUP_FULL).map_err(|e| anyhow::anyhow!(reason(&e)))?;
            let media = MediaFoundation;
            let stream = SHCreateMemStream(Some(bytes)).context("memory stream")?;
            let reader = (|| {
                let bytes = MFCreateMFByteStreamOnStream(&stream)?;
                let mut attributes = None;
                MFCreateAttributes(&mut attributes, 1)?;
                let attributes = attributes.expect("created");
                attributes.SetUINT32(&MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1)?;
                MFCreateSourceReaderFromByteStream(&bytes, &attributes)
            })()
            .map_err(|e| anyhow::anyhow!(reason(&e)))?;
            Ok(Self {
                reader,
                _media: media,
                _com: com,
            })
        }
    }

    /// RGB32 at `size`, or at the source size.
    fn output(&self, size: Option<(u32, u32)>) -> windows::core::Result<()> {
        // SAFETY: a media type this reader's video stream accepts.
        unsafe {
            let wanted: IMFMediaType = MFCreateMediaType()?;
            wanted.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            wanted.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)?;
            if let Some((w, h)) = size {
                wanted.SetUINT64(&MF_MT_FRAME_SIZE, (u64::from(w) << 32) | u64::from(h))?;
            }
            self.reader.SetCurrentMediaType(STREAM, None, &wanted)
        }
    }
}

impl Decoder for Reader {
    fn start(&mut self, size: Option<(u32, u32)>, from: f64) -> Result<()> {
        self.output(size)
            .map_err(|e| anyhow::anyhow!("output type: {}", reason(&e)))?;
        let mut position = PROPVARIANT::default();
        // SAFETY: a VT_I8 position in 100 ns units, as SetCurrentPosition takes.
        unsafe {
            let value = &mut position.Anonymous.Anonymous;
            value.vt = VT_I8;
            value.Anonymous.hVal = (from * 10_000_000.0) as i64;
            self.reader
                .SetCurrentPosition(&windows::core::GUID::zeroed(), &position)
        }
        .map_err(|e| anyhow::anyhow!("seek: {}", reason(&e)))
    }

    fn next(&mut self) -> Result<Option<(f64, image::RgbaImage)>> {
        let (mut flags, mut at, mut sample) = (0u32, 0i64, None);
        // SAFETY: out pointers to locals for one synchronous read.
        unsafe {
            let read = self.reader.ReadSample(
                STREAM,
                0,
                None,
                Some(&mut flags),
                Some(&mut at),
                Some(&mut sample),
            );
            read.map_err(|e| anyhow::anyhow!("read: {}", reason(&e)))?;
        }
        if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
            return Ok(None);
        }
        let Some(sample) = sample else {
            return self.next();
        };
        // SAFETY: the current type describes the buffer; it stays locked while copied.
        unsafe {
            let kind = self
                .reader
                .GetCurrentMediaType(STREAM)
                .map_err(|e| anyhow::anyhow!(reason(&e)))?;
            let size = kind
                .GetUINT64(&MF_MT_FRAME_SIZE)
                .map_err(|e| anyhow::anyhow!(reason(&e)))?;
            let (w, h) = ((size >> 32) as usize, (size & 0xffff_ffff) as usize);
            let stride = kind
                .GetUINT32(&MF_MT_DEFAULT_STRIDE)
                .map_or((w * 4) as i32, |s| s as i32);
            let buffer = sample
                .ConvertToContiguousBuffer()
                .map_err(|e| anyhow::anyhow!(reason(&e)))?;
            let (mut data, mut length) = (std::ptr::null_mut(), 0u32);
            buffer
                .Lock(&mut data, None, Some(&mut length))
                .map_err(|e| anyhow::anyhow!(reason(&e)))?;
            let rows = std::slice::from_raw_parts(data, length as usize);
            let row_len = stride.unsigned_abs() as usize;
            if rows.len() < row_len * h || row_len < w * 4 {
                buffer.Unlock().map_err(|e| anyhow::anyhow!(reason(&e)))?;
                bail!("short frame");
            }
            let mut bgra = Vec::with_capacity(w * h * 4);
            for row in 0..h {
                let line = if stride < 0 { h - 1 - row } else { row };
                bgra.extend_from_slice(&rows[line * row_len..line * row_len + w * 4]);
            }
            buffer.Unlock().map_err(|e| anyhow::anyhow!(reason(&e)))?;
            for pixel in bgra.as_chunks_mut::<4>().0 {
                pixel[3] = 255;
            }
            let image =
                image::RgbaImage::from_raw(w as u32, h as u32, bgra).context("short frame")?;
            Ok(Some((at as f64 / 10_000_000.0, image)))
        }
    }
}

/// Media Foundation reads the checked bytes from a memory stream.
pub fn open(
    bytes: &[u8],
    fit: Fit,
    send: impl Fn(Event) + Send + Sync + 'static,
) -> Result<Player> {
    let bytes = bytes.to_vec();
    Ok(Player::spawn(fit, send, move || Reader::new(&bytes)))
}
