//! `deepLinkSchemes.ts` and `deepLink.ts`: Cindy's deep links as text carries them.

#![cfg_attr(
    not(test),
    expect(dead_code, reason = "9.5e's bubble and 9.5f's chips read the links")
)]

use crate::cindy_json::decode_uri_component;

/// `DEEP_LINK_URL_PREFIXES`: the scheme links are built with, then the historic one.
const PREFIXES: [&str; 2] = ["cindy://", "xdt-maker://"];

/// `SESSION_DEEP_LINK_RE_SOURCE`.
pub const SESSION: &str =
    r"(?:cindy|xdt-maker)://session/[A-Za-z0-9%~_-]+(?:\?[A-Za-z0-9%&=~._-]*)?";

/// `PROJECT_DEEP_LINK_RE_SOURCE` without its lookahead, which `project_ends` is.
pub const PROJECT: &str = r"(?:cindy|xdt-maker)://project/[A-Za-z0-9%~._!*-]+";

/// A project link may not stop before a quote or a parenthesis that its folder name held.
pub fn project_ends(text: &str, end: usize) -> bool {
    !text[end..].starts_with(['\'', '('])
}

/// `stripDeepLinkPathPrefix`.
pub fn strip<'a>(url: &'a str, path: &str) -> Option<&'a str> {
    let prefix = PREFIXES
        .into_iter()
        .find(|prefix| url.starts_with(prefix))?;
    url[prefix.len()..].strip_prefix(path)
}

/// `isMarkdownEscaped`: an odd run of backslashes before `at`.
fn escaped(text: &[u8], at: usize) -> bool {
    let run = text[..at].iter().rev().take_while(|byte| **byte == b'\\');
    run.count() % 2 == 1
}

/// `findMarkdownLabelStart`: the `[` that the `]` at `close` balances, on one line.
pub fn label_start(text: &str, close: usize) -> Option<usize> {
    let text = text.as_bytes();
    if escaped(text, close) {
        return None;
    }
    let mut depth = 1;
    for at in (0..close).rev() {
        match text[at] {
            b'\n' => return None,
            b']' if !escaped(text, at) => depth += 1,
            b'[' if !escaped(text, at) => {
                depth -= 1;
                if depth == 0 {
                    return Some(at);
                }
            }
            _ => {}
        }
    }
    None
}

/// `unescapeMarkdownLabelBrackets`.
pub fn unescape_label(label: &str) -> String {
    let mut out = String::with_capacity(label.len());
    let mut chars = label.chars().peekable();
    while let Some(c) = chars.next() {
        match chars.peek() {
            Some('[' | ']') if c == '\\' => out.extend(chars.next()),
            _ => out.push(c),
        }
    }
    out
}

/// `SessionDeepLinkTarget`.
#[derive(Debug, PartialEq)]
pub struct Session {
    pub session_id: String,
    pub message_client_id: Option<String>,
    pub device_id: Option<String>,
}

/// The link's path without its hash, split from its query; `None` for an empty path.
fn path_and_query<'a>(href: &'a str, kind: &str) -> Option<(&'a str, Option<&'a str>)> {
    let rest = strip(href, kind)?;
    let rest = rest.split_once('#').map_or(rest, |(rest, _)| rest);
    let (path, query) = match rest.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (rest, None),
    };
    let path = path.trim_end_matches('/');
    (!path.is_empty()).then_some((path, query))
}

/// `parseSessionDeepLinkHref`.
pub fn parse_session(href: &str) -> Option<Session> {
    let (path, query) = path_and_query(href, "session/")?;
    let mut target = Session {
        session_id: decode_uri_component(path)?,
        message_client_id: None,
        device_id: None,
    };
    for pair in query.unwrap_or_default().split('&') {
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };
        let slot = match key {
            "message" => &mut target.message_client_id,
            "device" => &mut target.device_id,
            _ => continue,
        };
        if slot.is_none() {
            *slot = decode_uri_component(value).filter(|value| !value.is_empty());
        }
    }
    Some(target)
}

/// `parseProjectDeepLinkHref`: the working directory.
pub fn parse_project(href: &str) -> Option<String> {
    let (path, _) = path_and_query(href, "project/")?;
    decode_uri_component(path)
}

#[cfg(test)]
pub mod tests;
