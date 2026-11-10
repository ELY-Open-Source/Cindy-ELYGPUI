//! `userMessageLinkify.ts`: the links in a user message's plain text. Ranges are bytes.

#![cfg_attr(not(test), expect(dead_code, reason = "9.5e's bubble draws the links"))]

use std::{ops::Range, sync::LazyLock};

use regex::Regex;

use crate::{
    deep_link::{self, PROJECT, SESSION},
    js_re,
    theme_import::color::js_trim,
    url_boundary::{self, Options},
};

/// What a match links to.
#[derive(Debug, PartialEq)]
pub enum Link {
    Url,
    /// An absolute local image path.
    Img,
    Session(Target),
    Project(Target),
}

/// A deep link, with the title its Markdown form gave.
#[derive(Debug, PartialEq)]
pub struct Target {
    pub href: String,
    pub label: Option<String>,
}

/// `LinkifyMatch`.
#[derive(Debug, PartialEq)]
pub struct Match {
    pub range: Range<usize>,
    pub link: Link,
}

const IMAGE: &str = r#"(?:[A-Za-z]:[\\/]|/)[^\s<>"']*?\.(?i-u:png|jpe?g|gif|webp|svg|bmp|ico)"#;

fn wrapped(source: &str) -> Regex {
    js_re::regex(&format!(r"\]\(({source})\)"), false)
}

/// `[.,;:!?]+$` taken off a bare deep link.
fn unglued(link: &str) -> &str {
    link.trim_end_matches(['.', ',', ';', ':', '!', '?'])
}

/// The URLs: the scanner's matches, each cut where its address ends, scanned on from the cut.
fn urls(text: &str, out: &mut Vec<Match>) {
    let mut from = 0;
    while let Some(found) = url_boundary::bare_http().find_at(text, from) {
        let options = Options {
            prefix: &text[..found.start()],
            ..Options::default()
        };
        let end = found.start() + url_boundary::clip(found.as_str(), &options);
        from = found.end();
        if end > found.start() {
            out.push(Match {
                range: found.start()..end,
                link: Link::Url,
            });
            from = end;
        }
    }
}

/// `[label](href)` links: the label runs back to its balanced `[`.
fn markdown(
    text: &str,
    close: &Regex,
    valid: fn(&str) -> bool,
    out: &mut Vec<(Range<usize>, Target)>,
) {
    for found in close.captures_iter(text) {
        let (whole, href) = (found.get(0).expect("a match"), &found[1]);
        if !valid(href) {
            continue;
        }
        let Some(open) = deep_link::label_start(text, whole.start()) else {
            continue;
        };
        let label = deep_link::unescape_label(&text[open + 1..whole.start()]);
        let label = js_trim(&label);
        let target = Target {
            href: href.to_owned(),
            label: (!label.is_empty() && label != href).then(|| label.to_owned()),
        };
        out.push((open..whole.end(), target));
    }
}

fn bare(range: Range<usize>, href: &str) -> (Range<usize>, Target) {
    let target = Target {
        href: href.to_owned(),
        label: None,
    };
    (range.start..range.start + href.len(), target)
}

fn sessions(text: &str) -> Vec<(Range<usize>, Target)> {
    static CLOSE: LazyLock<Regex> = LazyLock::new(|| wrapped(SESSION));
    static BARE: LazyLock<Regex> = LazyLock::new(|| js_re::regex(SESSION, false));
    let mut out = Vec::new();
    let valid = |href: &str| deep_link::parse_session(href).is_some();
    markdown(text, &CLOSE, valid, &mut out);
    for found in BARE.find_iter(text) {
        out.push(bare(found.range(), unglued(found.as_str())));
    }
    out
}

fn projects(text: &str) -> Vec<(Range<usize>, Target)> {
    static CLOSE: LazyLock<Regex> = LazyLock::new(|| wrapped(PROJECT));
    static BARE: LazyLock<Regex> = LazyLock::new(|| js_re::regex(PROJECT, false));
    let mut out = Vec::new();
    let valid = |href: &str| deep_link::parse_project(href).is_some();
    markdown(text, &CLOSE, valid, &mut out);
    for found in BARE.find_iter(text) {
        let href = unglued(found.as_str());
        if deep_link::project_ends(text, found.end()) && valid(href) {
            out.push(bare(found.range(), href));
        }
    }
    out
}

/// `findLinkifyMatches`: every link, in order, none over another.
pub fn find(text: &str) -> Vec<Match> {
    static IMG: LazyLock<Regex> = LazyLock::new(|| js_re::regex(IMAGE, false));
    let mut matches = Vec::new();
    urls(text, &mut matches);
    matches.extend(IMG.find_iter(text).map(|found| Match {
        range: found.range(),
        link: Link::Img,
    }));
    let linked = |link: fn(Target) -> Link| {
        move |(range, target): (Range<usize>, Target)| Match {
            range,
            link: link(target),
        }
    };
    matches.extend(sessions(text).into_iter().map(linked(Link::Session)));
    matches.extend(projects(text).into_iter().map(linked(Link::Project)));
    // No two kinds start at one place, so the order is total.
    matches.sort_by_key(|found| found.range.start);
    let mut accepted: Vec<Match> = Vec::new();
    for candidate in matches {
        let free = |last: &Match| candidate.range.start >= last.range.end;
        if accepted.last().is_none_or(free) {
            accepted.push(candidate);
        }
    }
    accepted
}

#[cfg(test)]
mod tests;
