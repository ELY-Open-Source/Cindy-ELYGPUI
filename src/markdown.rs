//! The Markdown tree `MarkdownRenderer.tsx`'s remark parse builds, here on markdown-rs.
#![allow(dead_code)] // The tree's reader comes with 9.6e.

mod autolinks;
mod cjk_urls;
mod flanks;
pub mod hast;
mod html_images;
pub mod image_targets;
mod local_images;
mod path_links;
mod raw_destinations;
mod session_links;
#[cfg(test)]
mod tests;

use markdown::{Constructs, ParseOptions, mdast::Node};

/// micromark's replacement for NUL.
const REPLACEMENT: char = '\u{FFFD}';

/// micromark's preprocess: NUL reads as U+FFFD, in a chunk of its own, at these bytes.
fn preprocess(text: &str) -> (String, Vec<usize>) {
    let mut source = String::with_capacity(text.len());
    let mut nuls = Vec::new();
    for ch in text.chars() {
        if ch == '\0' {
            nuls.push(source.len());
            source.push(REPLACEMENT);
        } else {
            source.push(ch);
        }
    }
    (source, nuls)
}

/// The tree of `text`.
pub fn parse(text: &str) -> Node {
    let (source, nuls) = preprocess(text);
    let options = ParseOptions {
        constructs: Constructs {
            math_flow: true,
            math_text: true,
            ..Constructs::gfm()
        },
        gfm_strikethrough_single_tilde: false,
        chunk_ends: nuls.clone(),
        attention_flanks: Some(Box::new(move |bytes, slice, at| {
            flanks::judge(bytes, &nuls, slice, at)
        })),
        ..ParseOptions::default()
    };
    let mut root = markdown::to_mdast(&source, &options).expect("only MDX syntax fails to parse");
    autolinks::transform(&mut root);
    root
}

/// The tree remark-parse alone builds: CommonMark, with no extension.
fn plain(text: &str) -> Node {
    let (source, chunk_ends) = preprocess(text);
    let options = ParseOptions {
        chunk_ends,
        ..ParseOptions::default()
    };
    markdown::to_mdast(&source, &options).expect("only MDX syntax fails to parse")
}

/// The tree after `REMARK_PLUGINS`' transformers; `privileged` is `REMARK_PLUGINS_PRIVILEGED`.
fn transformed(text: &str, privileged: bool) -> Node {
    let mut tree = parse(text);
    cjk_urls::transform(&mut tree);
    html_images::transform(&mut tree);
    local_images::transform(&mut tree, text);
    if privileged {
        session_links::transform(&mut tree);
    }
    path_links::transform(&mut tree);
    raw_destinations::transform(&mut tree);
    tree
}
