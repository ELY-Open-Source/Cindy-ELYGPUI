//! An SVG drawn as an image: like Blink's, it loads nothing outside itself.

use std::sync::{Arc, LazyLock};

use anyhow::{Context as _, Result};
use gpui::RenderImage;
use image::{Frame, RgbaImage};
use resvg::{
    tiny_skia::{Pixmap, Transform},
    usvg::{ImageHrefResolver, Options, Tree, fontdb::Database},
};

#[cfg(test)]
mod tests;

/// The pixels drawn per CSS pixel, so a zoomed image stays sharp.
const SCALE: f32 = 2.0;

/// gpui's cap on a texture's side.
const MAX: f32 = 8192.0;

static FONTS: LazyLock<Arc<Database>> = LazyLock::new(|| {
    let mut fonts = Database::new();
    fonts.load_system_fonts();
    Arc::new(fonts)
});

/// The SVG's image and its natural size; an `image` that names a file or an address is not read.
pub fn render(bytes: &[u8]) -> Result<(RenderImage, (u32, u32))> {
    let options = Options {
        fontdb: FONTS.clone(),
        image_href_resolver: ImageHrefResolver {
            resolve_data: ImageHrefResolver::default_data_resolver(),
            resolve_string: Box::new(|href, _| {
                log::warn!(target: "sessionView", "an SVG image's outside reference is not read: {} bytes", href.len());
                None
            }),
        },
        ..Options::default()
    };
    let tree = Tree::from_data(bytes, &options)?;
    let size = tree.size();
    let scale = SCALE.min(MAX / size.width()).min(MAX / size.height());
    let (width, height) = (size.width() * scale, size.height() * scale);
    let mut pixmap = Pixmap::new(width as u32, height as u32).context("an SVG has a size")?;
    let fit = Transform::from_scale(width / size.width(), height / size.height());
    resvg::render(&tree, fit, &mut pixmap.as_mut());
    let (wide, tall) = (pixmap.width(), pixmap.height());
    let mut pixels = pixmap.take();
    // gpui's frames are BGRA with straight alpha.
    for pixel in pixels.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
        let alpha = f32::from(pixel[3]) / 255.0;
        if alpha > 0.0 {
            for channel in &mut pixel[..3] {
                *channel = (f32::from(*channel) / alpha) as u8;
            }
        }
    }
    let frame = RgbaImage::from_raw(wide, tall, pixels).context("the pixels fill the frame")?;
    let natural = (size.width().round() as u32, size.height().round() as u32);
    Ok((RenderImage::new(vec![Frame::new(frame)]), natural))
}
