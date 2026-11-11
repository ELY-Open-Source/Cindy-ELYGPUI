//! `markdownImageTargets.ts`: the addresses a Markdown text hands the `img` renderer.

use std::{cell::RefCell, collections::HashMap, sync::LazyLock};

use markdown::mdast::Node;
use regex::Regex;

use crate::markdown_media;

static IMG: LazyLock<Regex> = LazyLock::new(|| Regex::new("(?i)<img").expect("a fixed regex"));

fn visit(node: &Node, urls: &mut Vec<String>) {
    if let Node::Image(image) = node {
        let url = &image.url;
        // A managed video draws as a video, and no address counts twice.
        if !url.is_empty() && !markdown_media::is_managed_video(url) && !urls.contains(url) {
            urls.push(url.clone());
        }
    }
    let children = node.children().into_iter().flatten();
    children.for_each(|child| visit(child, urls));
}

/// `extractRenderedMarkdownImageTargets`; `normalizeMathDelimiters` joins with 9.8.
pub fn extract(markdown: &str) -> Vec<String> {
    if !markdown.contains("![") && !IMG.is_match(markdown) {
        return Vec::new();
    }
    let mut urls = Vec::new();
    visit(&super::transformed(markdown, true), &mut urls);
    urls
}

/// `MarkdownImageTargetCache`: a message's content and its targets, by client id.
#[derive(Default)]
pub struct Cache(RefCell<HashMap<String, (String, Vec<String>)>>);

impl Cache {
    /// `extractCachedRenderedMarkdownImageTargets`: a changed content is read again.
    pub fn targets(&self, key: &str, markdown: &str) -> Vec<String> {
        let mut held = self.0.borrow_mut();
        if let Some((content, targets)) = held.get(key)
            && content == markdown
        {
            return targets.clone();
        }
        let targets = extract(markdown);
        held.insert(key.to_owned(), (markdown.to_owned(), targets.clone()));
        targets
    }
}
