//! What an SVG image may load, by `logs/review/9.6i3c1-r1-review-external-blink.log`.

use image::{ExtendedColorType, ImageEncoder as _, codecs::png::PngEncoder};

use super::render;

/// The alpha at the middle of a 40 by 20 SVG that holds one `image` of `href`.
fn middle(href: &str) -> u8 {
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><image href="{href}" width="40" height="20"/></svg>"#
    );
    let (image, natural) = render(svg.as_bytes()).unwrap();
    assert_eq!(natural, (40, 20));
    let size = image.size(0);
    assert_eq!((size.width.0, size.height.0), (80, 40));
    image.as_bytes(0).unwrap()[(20 * 80 + 40) * 4 + 3]
}

#[test]
fn an_image_inside_draws_and_one_outside_is_not_read() {
    let mut png = Vec::new();
    let white = [255u8; 4 * 4 * 4];
    PngEncoder::new(&mut png)
        .write_image(&white, 4, 4, ExtendedColorType::Rgba8)
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("private.png");
    std::fs::write(&file, &png).unwrap();
    let inside: String = png.iter().map(|byte| format!("%{byte:02X}")).collect();
    assert_eq!(middle(&format!("data:image/png,{inside}")), 255);
    assert_eq!(middle(file.to_str().unwrap()), 0);
    assert_eq!(middle(&format!("file://{}", file.display())), 0);
    assert_eq!(middle("https://example.test/a.png"), 0);
}
