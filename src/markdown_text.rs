//! `shared/markdownPreviewText.ts`: readable text from Markdown, without HTML or link
//! destinations, over the mdast remark-parse and remark-gfm build (here markdown-rs).

use std::{collections::HashMap, sync::LazyLock};

use markdown::{ParseOptions, mdast::Node};
use regex::Regex;

use crate::theme_import::color::{JS_SPACE, js_space, js_trim};

static IMG_TAG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"^{JS_SPACE}*<[Ii][Mm][Gg](?-u:\b)([^>]*)/?>{JS_SPACE}*$"
    ))
    .expect("valid pattern")
});
static ATTR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r#"([A-Za-z_:][A-Za-z0-9:._-]*){JS_SPACE}*={JS_SPACE}*(?:"([^"]*)"|'([^']*)'|([^"'=<>`\t\n\x0B\x0C\r \xA0\x{{1680}}\x{{2000}}-\x{{200A}}\x{{2028}}\x{{2029}}\x{{202F}}\x{{205F}}\x{{3000}}\x{{FEFF}}]+))"#
    ))
    .expect("valid pattern")
});
static ENTITY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"&(amp|quot|#39|apos|lt|gt);").expect("valid pattern"));
static SAFE_SRC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[Hh][Tt][Tt][Pp][Ss]?://").expect("valid pattern"));

/// `markdownPreviewText(markdown)`: `cindy-ui` blocks as text, then the projection.
pub fn preview(markdown: &str) -> String {
    project(&crate::rich_answer::text(markdown))
}

enum Item<'a> {
    Node(&'a Node),
    Space,
}

/// `projectMarkdownPreview`, not streaming, image alt text kept with no fallback.
fn project(markdown: &str) -> String {
    // micromark's preprocess: NUL reads as U+FFFD.
    let markdown = markdown.replace('\0', "\u{FFFD}");
    let root = markdown::to_mdast(&markdown, &ParseOptions::gfm())
        .expect("only MDX syntax fails to parse");
    let mut stack = vec![Item::Node(&root)];
    let mut out = String::new();
    while let Some(item) = stack.pop() {
        let node = match item {
            Item::Space => {
                out.push(' ');
                continue;
            }
            Item::Node(node) => node,
        };
        match node {
            Node::Text(t) => out.push_str(&t.value),
            Node::InlineCode(c) => out.push_str(&c.value),
            Node::Code(c) => {
                out.push(' ');
                out.push_str(&c.value);
                out.push(' ');
            }
            Node::Image(i) => out.push_str(js_trim(&i.alt)),
            Node::ImageReference(i) => out.push_str(js_trim(&i.alt)),
            Node::Html(h) => {
                if let Some(alt) = html_image_alt(&h.value) {
                    out.push_str(js_trim(&alt));
                    out.push(' ');
                }
            }
            Node::Break(_) | Node::ThematicBreak(_) => out.push(' '),
            node => {
                let Some(children) = node.children() else {
                    continue;
                };
                let inline = matches!(
                    node,
                    Node::Emphasis(_)
                        | Node::Strong(_)
                        | Node::Delete(_)
                        | Node::Link(_)
                        | Node::LinkReference(_)
                );
                if !inline {
                    out.push(' ');
                    stack.push(Item::Space);
                }
                stack.extend(children.iter().rev().map(Item::Node));
            }
        }
    }
    collapse(&out)
}

/// `.replace(/\s+/gu, ' ').trim()`.
fn collapse(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut space = false;
    for c in text.chars() {
        if js_space(c) {
            space = true;
            continue;
        }
        if space && !out.is_empty() {
            out.push(' ');
        }
        space = false;
        out.push(c);
    }
    out
}

/// `htmlImgToImageNode(node).alt`: an `<img>` alone in the HTML, with a safe `src`.
fn html_image_alt(html: &str) -> Option<String> {
    let raw = IMG_TAG.captures(html)?.get(1)?.as_str();
    let mut attrs: HashMap<String, String> = HashMap::new();
    for found in ATTR.captures_iter(raw) {
        let value = (2..=4)
            .find_map(|i| found.get(i))
            .map_or("", |m| m.as_str());
        attrs.insert(found[1].to_ascii_lowercase(), decode(value));
    }
    let src = js_trim(attrs.get("src")?);
    let safe = SAFE_SRC.is_match(src)
        || [
            "xdt-image://",
            "cindy-media://",
            "xdt-file://",
            "cindy-remote-media://",
        ]
        .iter()
        .any(|p| src.starts_with(p));
    if src.is_empty() || !safe {
        return None;
    }
    Some(attrs.remove("alt").unwrap_or_default())
}

/// `decodeHtmlAttribute`.
fn decode(value: &str) -> String {
    ENTITY
        .replace_all(value, |c: &regex::Captures| match &c[1] {
            "amp" => "&",
            "quot" => "\"",
            "lt" => "<",
            "gt" => ">",
            _ => "'",
        })
        .into_owned()
}

#[cfg(test)]
mod tests;
