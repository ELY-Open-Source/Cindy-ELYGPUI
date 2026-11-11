//! The Markdown tree `MarkdownRenderer.tsx`'s remark parse builds, here on markdown-rs.
#![allow(dead_code)] // The tree's readers come with 9.6b.

mod autolinks;
mod flanks;
#[cfg(test)]
mod tests;

use markdown::{Constructs, ParseOptions, mdast::Node};

/// micromark's replacement for NUL.
const REPLACEMENT: char = '\u{FFFD}';

/// The tree of `text`.
pub fn parse(text: &str) -> Node {
    // micromark's preprocess: NUL reads as U+FFFD, in a chunk of its own.
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
