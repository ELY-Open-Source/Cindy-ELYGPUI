//! Electron's `nativeImage.createFromPath` and the bitmap `clipboard.writeImage` takes from it.

use std::path::{Component, Path};

use image::{ImageFormat, RgbaImage};

/// `kScaleFactorPairs`, in Electron's order.
const SCALES: [(&str, f32); 11] = [
    ("@2x", 2.0),
    ("@3x", 3.0),
    ("@1x", 1.0),
    ("@4x", 4.0),
    ("@5x", 5.0),
    ("@1.25x", 1.25),
    ("@1.33x", 1.33),
    ("@1.4x", 1.4),
    ("@1.5x", 1.5),
    ("@1.8x", 1.8),
    ("@2.5x", 2.5),
];

/// `AddImageSkiaRepFromPath`: the bytes as a PNG, else as a JPEG, whatever the name says.
fn decode(path: &Path) -> Option<RgbaImage> {
    let bytes = std::fs::read(path).ok()?;
    let read = |format| image::load_from_memory_with_format(&bytes, format).ok();
    let image = read(ImageFormat::Png).or_else(|| read(ImageFormat::Jpeg))?;
    if image.color().bytes_per_pixel() == image.color().channel_count() {
        return Some(image.into_rgba8());
    }
    // A 16-bit sample keeps its high byte (`9.6h4b-r1-review-probes.log`).
    let deep = image.into_rgba16();
    let bytes = deep.as_raw().iter().map(|sample| (sample >> 8) as u8);
    RgbaImage::from_raw(deep.width(), deep.height(), bytes.collect())
}

/// `kCommonDoubleExtensionSuffixes`.
const PACKED: [&str; 9] = ["bz", "bz2", "gz", "lz", "lzma", "lzo", "xz", "z", "zst"];

/// `base::FilePath`'s `ExtensionSeparatorPosition`, on a name or on a whole path.
fn dot(path: &str) -> Option<usize> {
    if path == "." || path == ".." {
        return None;
    }
    let last = path.rfind('.')?;
    let head = &path[..last];
    let Some(before) = head.rfind('.') else {
        return Some(last);
    };
    if head.rfind('/').is_some_and(|slash| before < slash) {
        return Some(last);
    }
    let packed = PACKED
        .iter()
        .any(|known| path[last + 1..].eq_ignore_ascii_case(known));
    let double = path[before + 1..].eq_ignore_ascii_case("user.js")
        || (packed && (2..=5).contains(&(last - before)));
    Some(if double { before } else { last })
}

/// `BaseName`: the trailing separators go, then what follows the last one stays.
fn base(path: &str) -> &str {
    let bytes = path.as_bytes();
    let (mut end, mut stripped) = (path.len(), None);
    for at in (2..=path.len()).rev() {
        if bytes[at - 1] != b'/' {
            break;
        }
        // Two separators that start the path stay, unless more than two did.
        if at != 2 || stripped == Some(3) || bytes[0] != b'/' {
            (end, stripped) = (at - 1, Some(at));
        }
    }
    let kept = &path[..end];
    match kept.rfind('/') {
        Some(slash) if slash + 1 < kept.len() => &kept[slash + 1..],
        _ => kept,
    }
}

/// `InsertBeforeExtensionASCII`, on the path as written: `folder/` gives `folder/@2x`.
fn beside(path: &str, suffix: &str) -> Option<String> {
    let name = base(path);
    if matches!(name, "" | "." | "..") {
        return None;
    }
    let (stem, extension) = match dot(name) {
        Some(at) => (&path[..dot(path)?], &name[at..]),
        None => (path, ""),
    };
    Some(format!("{stem}{suffix}{extension}"))
}

/// `PopulateImageSkiaRepsFromPath`, then the representation `AsBitmap` picks for scale 1.
fn pick(path: &str) -> Option<RgbaImage> {
    let name = base(path);
    let stem = &name[..dot(name).unwrap_or(name.len())];
    // `*@*x`: the name sets its own density, so no other file is read.
    if stem
        .strip_suffix('x')
        .is_some_and(|rest| rest.contains('@'))
    {
        return decode(Path::new(path));
    }
    let first = decode(Path::new(path)).map(|image| (1.0, image));
    let others = SCALES.iter().filter_map(|(suffix, scale)| {
        let image = decode(Path::new(&beside(path, suffix)?))?;
        Some((*scale, image))
    });
    // An exact 1, else the least density over it; the first found wins a tie.
    let mut best: Option<(f32, RgbaImage)> = None;
    for (scale, image) in first.into_iter().chain(others) {
        if best.as_ref().is_none_or(|(held, _)| scale < *held) {
            best = Some((scale, image));
        }
    }
    best.map(|(_, image)| image)
}

/// Skia's `SkMulDiv255Round`.
fn premultiplied(channel: u8, alpha: u8) -> u8 {
    let product = u32::from(channel) * u32::from(alpha) + 128;
    ((product + (product >> 8)) >> 8) as u8
}

/// Skia's raster pipeline in `f32`, as its PNG encoder unpremultiplies (`9.6h4b-electron-unpremul.log`).
fn straight(channel: u8, alpha: u8) -> u8 {
    const UNIT: f32 = 1.0 / 255.0;
    let (channel, alpha) = (f32::from(channel) * UNIT, f32::from(alpha) * UNIT);
    let value = (channel * (1.0 / alpha)).clamp(0.0, 1.0);
    (value * 255.0).round_ties_even() as u8
}

/// `NormalizePath`: a path through `..` is the real path, when it has one.
fn real(path: &str) -> String {
    let climbs = Path::new(path)
        .components()
        .any(|part| part == Component::ParentDir);
    let real = climbs.then(|| std::fs::canonicalize(path).ok()).flatten();
    let real = real.and_then(|real| real.to_str().map(str::to_owned));
    real.unwrap_or_else(|| path.to_owned())
}

/// The image as the clipboard's PNG holds it: decoded premultiplied, then encoded straight.
pub fn read(path: &str) -> Option<RgbaImage> {
    let mut image = pick(&real(path))?;
    for pixel in image.pixels_mut() {
        let alpha = pixel[3];
        for channel in &mut pixel.0[..3] {
            *channel = match alpha {
                0 => 0,
                255 => *channel,
                _ => straight(premultiplied(*channel, alpha), alpha),
            };
        }
    }
    Some(image)
}
