//! `markdownTarget.ts`: what a Markdown link or an inline code span points at.

#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "inline code's targets come with 9.6h4, a remote task's with its chapter"
    )
)]

use std::sync::LazyLock;

use regex::Regex;
use url::Url;

use crate::{
    cindy_json::decode_uri_component,
    js_re,
    local_path::{
        drive,
        shape::{self, Kind},
    },
    theme_import::color::js_trim,
};

pub mod resolved;
#[cfg(test)]
pub mod tests;

/// `MarkdownLocalKind`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Local {
    Image,
    Model,
    Text,
    Directory,
}

/// Why a reference stays code or plain text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reason {
    Directory,
    PathLike,
    Scheme,
    Empty,
    NotATarget,
}

/// `MarkdownTarget`.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    External {
        href: String,
    },
    Anchor {
        id: String,
        href: String,
    },
    Audio {
        href: String,
    },
    LocalImageUrl {
        href: String,
    },
    Resolved {
        href: String,
        path: String,
        kind: Local,
        at: Place,
    },
    Candidate {
        href: String,
        original: String,
        kind: Local,
        at: Place,
    },
    Code {
        href: String,
        reason: Reason,
    },
    Plain {
        href: String,
        reason: Reason,
    },
}

/// A `:line[:column]` suffix.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Place {
    pub line: Option<u32>,
    pub column: Option<u32>,
}

/// `[1-9]\d{0,6}` over the whole of `text`.
fn number(text: &str) -> Option<u32> {
    let digits = text.len() <= 7 && text.bytes().all(|b| b.is_ascii_digit());
    (digits && !text.starts_with('0'))
        .then(|| text.parse().ok())
        .flatten()
}

fn file_url(text: &str) -> bool {
    text.get(..7)
        .is_some_and(|head| head.eq_ignore_ascii_case("file://"))
}

/// `splitLocalLineSuffix`: the reference without its line suffix, and the place it named.
pub fn split_line(raw: &str) -> (&str, Place) {
    let href = js_trim(raw);
    let whole = (href, Place::default());
    if shape::scheme_slashes(href) && !file_url(href) {
        return whole;
    }
    let Some(colon) = href.rfind(':').filter(|colon| *colon > 0) else {
        return whole;
    };
    let (before, last) = (&href[..colon], &href[colon + 1..]);
    if let Some((from, to)) = last.split_once('-')
        && let (Some(from), Some(to)) = (number(from), number(to))
    {
        let line = Place {
            line: Some(from),
            column: None,
        };
        return if to < from { whole } else { (before, line) };
    }
    let Some(last) = number(last) else {
        return whole;
    };
    let previous = before
        .rfind(':')
        .and_then(|colon| Some((colon, number(&before[colon + 1..])?)));
    let (base, at) = match previous {
        Some((colon, line)) => (
            &before[..colon],
            Place {
                line: Some(line),
                column: Some(last),
            },
        ),
        None => (
            before,
            Place {
                line: Some(last),
                column: None,
            },
        ),
    };
    if base.is_empty() { whole } else { (base, at) }
}

/// `SCHEME_RE`: a URL scheme and its colon.
pub fn scheme(value: &str) -> bool {
    static SCHEME: LazyLock<Regex> =
        LazyLock::new(|| js_re::regex("^[A-Za-z][A-Za-z0-9+.-]*:", false));
    SCHEME.is_match(value)
}

/// `hasUnsupportedScheme`.
fn unsupported_scheme(value: &str) -> bool {
    !drive(value) && !file_url(value) && scheme(value)
}

/// `looksLikeBareFileReference`.
pub fn bare_file(value: &str) -> bool {
    static BARE: LazyLock<Regex> =
        LazyLock::new(|| js_re::regex(r#"^[^\s:<>()\[\]{}"'`]+\.[A-Za-z0-9]{1,10}$"#, false));
    BARE.is_match(value)
}

/// `normalizeSandboxHref`: `sandbox:/C:/x` and `sandbox:///mnt/x` as bare absolute paths.
pub fn sandbox(raw: &str) -> &str {
    let rest = match raw.get(..8) {
        Some(head) if head.eq_ignore_ascii_case("sandbox:") && !raw.contains('\n') => &raw[8..],
        _ => return raw,
    };
    let path = rest.trim_start_matches('/');
    match () {
        () if drive(path) => path,
        () if path.len() < rest.len() => &rest[rest.len() - path.len() - 1..],
        () => raw,
    }
}

/// `isAmbiguousPathShape`: a shape code or prose could share.
pub fn ambiguous(href: &str, original: Option<&str>) -> bool {
    let raw = original.unwrap_or(href);
    let absolute = file_url(raw) || raw.starts_with('/') || drive(raw);
    !shape::directory(raw) && !absolute && !shape::file(href)
}

/// The remote's answer on a reference: `fs:stat`'s verdict.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Verdict {
    File,
    Directory,
    Nonfile,
    Unknown,
}

/// `decideRemoteLit`: whether the reference lights, and as a directory.
pub fn remote_lit(verdict: Option<Verdict>, href: &str, original: Option<&str>) -> Option<bool> {
    match verdict? {
        Verdict::File => Some(false),
        Verdict::Directory => Some(true),
        Verdict::Unknown if !ambiguous(href, original) => {
            Some(shape::directory(original.unwrap_or(href)))
        }
        Verdict::Unknown | Verdict::Nonfile => None,
    }
}

/// `toMarkdownLocalKind`.
fn local(kind: Kind) -> Option<Local> {
    match kind {
        Kind::Image => Some(Local::Image),
        Kind::Model => Some(Local::Model),
        Kind::Text => Some(Local::Text),
        Kind::External | Kind::Directory => None,
    }
}

/// `classifyLocalCandidate`.
fn candidate(original: &str, href: &str, kind: Local, files: &[(String, String)]) -> Target {
    let (href, at) = (href.to_owned(), split_line(original).1);
    match shape::known(&href, files).filter(|path| !path.is_empty()) {
        Some(path) => Target::Resolved {
            href,
            path: path.to_owned(),
            kind,
            at,
        },
        None => Target::Candidate {
            href,
            original: original.to_owned(),
            kind,
            at,
        },
    }
}

/// The file an `xdt-file://` link names, when it is an absolute path and no image.
fn xdt_file_path(raw: &str) -> Option<String> {
    static IMAGE: LazyLock<Regex> =
        LazyLock::new(|| js_re::regex(r"\.(?i-u:png|jpe?g|gif|webp|svg|bmp|ico)$", false));
    let direct = match raw
        .strip_prefix("xdt-file://")
        .filter(|rest| rest.starts_with('/'))
    {
        Some(rest) => decode_uri_component(rest.split(['?', '#']).next().unwrap_or(rest))?,
        None => String::new(),
    };
    let path = match direct.is_empty() {
        false => direct,
        true => {
            let url = Url::parse(raw).ok()?;
            let query = url.query_pairs().find(|(key, _)| key == "path");
            match query
                .map(|(_, value)| value.into_owned())
                .filter(|path| !path.is_empty())
            {
                Some(path) => path,
                None if url.host_str().is_none_or(str::is_empty) => {
                    decode_uri_component(url.path())?
                }
                None => return None,
            }
        }
    };
    let b = path.as_bytes();
    let slashed = b.len() >= 4 && b[0] == b'/' && b[1].is_ascii_alphabetic() && &b[2..4] == b":/";
    let path = if slashed { &path[1..] } else { path.as_str() };
    let absolute = path.starts_with('/') || drive(path);
    (absolute && !IMAGE.is_match(path)).then(|| path.to_owned())
}

/// `classifyMarkdownLinkTarget`; `files` are the task's attached files, name then path.
pub fn link(href: Option<&str>, files: &[(String, String)]) -> Target {
    static WEB: LazyLock<Regex> = LazyLock::new(|| js_re::regex("^(?i-u:https?)://", false));
    let raw = sandbox(js_trim(href.unwrap_or("")));
    let file = raw
        .starts_with("xdt-file://")
        .then(|| xdt_file_path(raw))
        .flatten();
    let raw = file.as_deref().unwrap_or(raw);
    let href = raw.to_owned();
    if raw.is_empty() {
        return Target::Plain {
            href,
            reason: Reason::Empty,
        };
    }
    if let Some(id) = raw.strip_prefix('#') {
        let id = decode_uri_component(id).unwrap_or_else(|| id.to_owned());
        return Target::Anchor { id, href };
    }
    if raw.starts_with("xdt-audio://") {
        return Target::Audio { href };
    }
    if raw.starts_with("xdt-image://") {
        return Target::LocalImageUrl { href };
    }
    if raw.starts_with("xdt-file://") {
        return Target::LocalImageUrl {
            href: shape::xdt_file(raw),
        };
    }
    if WEB.is_match(raw) {
        return Target::External { href };
    }
    if unsupported_scheme(raw) {
        return Target::Plain {
            href,
            reason: Reason::Scheme,
        };
    }
    let (path, _) = split_line(raw);
    let kind = shape::classify(path);
    if let Some(kind) = local(kind) {
        return candidate(raw, path, kind, files);
    }
    if kind == Kind::Directory || shape::directory(path) {
        return match path.trim_end_matches(['/', '\\']) {
            "" => Target::Code {
                href: path.to_owned(),
                reason: Reason::Directory,
            },
            stripped => candidate(raw, stripped, Local::Text, files),
        };
    }
    Target::Plain {
        href,
        reason: Reason::NotATarget,
    }
}

/// `classifyInlineCodeTarget`.
pub fn inline_code(text: &str) -> Option<Target> {
    let untrimmed = js_trim(text) != text;
    if text.is_empty() || untrimmed || text.contains('\n') || unsupported_scheme(text) {
        return None;
    }
    let (href, at) = split_line(text);
    if shape::directory(href) {
        let stripped = href.trim_end_matches(['/', '\\']);
        return (!stripped.is_empty()).then(|| Target::Candidate {
            href: stripped.to_owned(),
            original: text.to_owned(),
            kind: Local::Text,
            at: Place::default(),
        });
    }
    let href = href.to_owned();
    match local(shape::classify(&href)) {
        Some(kind) => Some(Target::Candidate {
            href,
            original: text.to_owned(),
            kind,
            at,
        }),
        None if shape::file(&href) || bare_file(&href) => Some(Target::Code {
            href,
            reason: Reason::PathLike,
        }),
        None => None,
    }
}
