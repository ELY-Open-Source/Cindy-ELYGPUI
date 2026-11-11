//! `bareSpaceLocalImages.ts`: Markdown images whose local destination holds bare spaces.
#![allow(dead_code)] // The tree's reader comes with 9.6e.

use std::{ops::Range, sync::LazyLock};

use regex::Regex;

use crate::{
    local_path,
    theme_import::color::{js_space, js_trim},
};

#[cfg(test)]
mod tests;

/// `BareSpaceLocalImage`; `range` is the whole construct, in bytes.
#[derive(Debug, PartialEq)]
pub struct Image {
    pub range: Range<usize>,
    pub alt: String,
    pub url: String,
    pub title: Option<String>,
}

/// What a JavaScript dot refuses.
fn terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// `(?:[^\n\\]|\\.)*` over the whole of `title`.
fn escapes(title: &str) -> bool {
    let mut chars = title.chars();
    while let Some(c) = chars.next() {
        if c == '\n' || (c == '\\' && !chars.next().is_some_and(|next| !terminator(next))) {
            return false;
        }
    }
    true
}

/// `QUOTED_TITLE_RE`: the destination and the title a quoted tail gives it.
fn quoted(destination: &str) -> Option<(&str, &str)> {
    let quote = destination
        .chars()
        .next_back()
        .filter(|c| matches!(c, '"' | '\''))?;
    let body = &destination[..destination.len() - 1];
    let line = body.find(terminator).unwrap_or(body.len());
    // The longest destination first, as the greedy `.*\S` gives it.
    body.char_indices().rev().find_map(|(at, c)| {
        let head = body[..at].trim_end_matches([' ', '\t']);
        let fits = c == quote && head.len() < at && head.len() <= line && escapes(&body[at + 1..]);
        let solid = head.chars().next_back().is_some_and(|c| !js_space(c));
        (fits && solid).then(|| (head, &body[at + 1..]))
    })
}

/// `LOCAL_PREFIX_RE`.
fn local(destination: &str) -> bool {
    let rooted = destination.starts_with('/') && !destination.starts_with("//");
    let scheme = destination
        .get(..13)
        .is_some_and(|head| head.eq_ignore_ascii_case("xdt-image:///"));
    rooted || local_path::drive(destination) || scheme
}

/// `IMAGE_EXTENSION_RE`.
fn pictured(destination: &str) -> bool {
    const EXTENSIONS: [&str; 10] = [
        "png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "heic", "heif", "avif",
    ];
    let Some((_, extension)) = destination.rsplit_once('.') else {
        return false;
    };
    EXTENSIONS
        .iter()
        .any(|known| extension.eq_ignore_ascii_case(known))
}

/// `findBareSpaceLocalImages`.
pub fn find(text: &str) -> Vec<Image> {
    static IMAGE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"!\[([^\[\]\n]*)\]\(((?:[^()\n<>]|\([^()\n<>]*\))+)\)").expect("a regex")
    });
    let mut found = Vec::new();
    for image in IMAGE.captures_iter(text) {
        let whole = image.get(0).expect("a match");
        let slashes = text[..whole.start()]
            .bytes()
            .rev()
            .take_while(|b| *b == b'\\')
            .count();
        if slashes % 2 == 1 {
            continue;
        }
        let trimmed = js_trim(&image[2]);
        let (destination, title) = match quoted(trimmed) {
            Some((destination, title)) => (destination, Some(title.to_owned())),
            None => (trimmed, None),
        };
        if destination.contains([' ', '\t']) && local(destination) && pictured(destination) {
            found.push(Image {
                range: whole.range(),
                alt: image[1].to_owned(),
                url: destination.to_owned(),
                title,
            });
        }
    }
    found
}
