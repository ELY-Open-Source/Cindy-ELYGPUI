//! `remarkHtmlImages` with `htmlImage.ts`: a lone safe `<img>` becomes an image node.

use std::sync::LazyLock;

use markdown::mdast::{Image, Node};
use regex::Regex;

use crate::{js_re, theme_import::color::js_trim};

/// Replaces every html node that is one safe `<img>`.
pub fn transform(node: &mut Node) {
    if let Node::Html(html) = node
        && let Some(image) = image(&html.value)
    {
        *node = Node::Image(Image {
            position: html.position.clone(),
            ..image
        });
    }
    for child in node.children_mut().into_iter().flatten() {
        transform(child);
    }
}

/// `decodeHtmlAttribute`.
fn decode(value: &str) -> String {
    static ENTITY: LazyLock<Regex> =
        LazyLock::new(|| js_re::regex("&(amp|quot|#39|apos|lt|gt);", false));
    let plain = |found: &regex::Captures| match &found[1] {
        "amp" => "&",
        "quot" => "\"",
        "#39" | "apos" => "'",
        "lt" => "<",
        _ => ">",
    };
    ENTITY.replace_all(value, plain).into_owned()
}

/// `SAFE_DIMENSION_RE`.
fn dimension(value: &str) -> bool {
    (1..=4).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_digit())
}

/// `isSafeHtmlImageSrc`.
fn safe(src: &str) -> bool {
    static WEB: LazyLock<Regex> = LazyLock::new(|| js_re::regex("^https?://", true));
    const SCHEMES: [&str; 4] = [
        "xdt-image://",
        "cindy-media://",
        "xdt-file://",
        "cindy-remote-media://",
    ];
    WEB.is_match(src) || SCHEMES.iter().any(|scheme| src.starts_with(scheme))
}

/// `htmlImgToImageNode`, less the node's place.
fn image(html: &str) -> Option<Image> {
    static TAG: LazyLock<Regex> = LazyLock::new(|| js_re::regex(r"^\s*<img\b([^>]*)/?>\s*$", true));
    static ATTR: LazyLock<Regex> = LazyLock::new(|| {
        let source = r#"([A-Za-z_:][A-Za-z0-9:._-]*)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'=<>`]+))"#;
        js_re::regex(source, false)
    });
    let raw = TAG.captures(html)?.get(1).expect("the tag's group");
    // `parseAttributes`: a later attribute of one name wins.
    let attribute = |name: &str| {
        let named = |found: &regex::Captures| found[1].eq_ignore_ascii_case(name);
        let found = ATTR.captures_iter(raw.as_str()).filter(named).last()?;
        let value = (2..=4).find_map(|group| found.get(group));
        Some(decode(value.map_or("", |value| value.as_str())))
    };
    let src = attribute("src")?;
    let src = js_trim(&src);
    if src.is_empty() || !safe(src) {
        return None;
    }
    let size = |name: &str| attribute(name).filter(|value| dimension(value));
    Some(Image {
        position: None,
        url: src.to_owned(),
        alt: attribute("alt").unwrap_or_default(),
        title: attribute("title").filter(|title| !title.is_empty()),
        width: size("width"),
        height: size("height"),
        raw: None,
    })
}
