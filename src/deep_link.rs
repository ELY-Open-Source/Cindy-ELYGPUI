//! `deepLinkSchemes.ts` and `deepLink.ts`: Cindy's deep links as text carries them.

use url::Url;

use crate::{cindy_json::decode_uri_component, theme_import::color::js_trim};

/// `DEEP_LINK_URL_PREFIXES`: the scheme links are built with, then the historic one.
const PREFIXES: [&str; 2] = ["cindy://", "xdt-maker://"];

/// `SESSION_DEEP_LINK_RE_SOURCE`.
pub const SESSION: &str =
    r"(?:cindy|xdt-maker)://session/[A-Za-z0-9%~_-]+(?:\?[A-Za-z0-9%&=~._-]*)?";

/// `PROJECT_DEEP_LINK_RE_SOURCE` without its lookahead, which `project_ends` is.
pub const PROJECT: &str = r"(?:cindy|xdt-maker)://project/[A-Za-z0-9%~._!*-]+";

/// `textContainsDeepLink`.
pub fn contains(text: &str) -> bool {
    PREFIXES.iter().any(|prefix| text.contains(prefix))
}

/// `isDeepLinkUrl`.
pub fn starts(url: &str) -> bool {
    PREFIXES.iter().any(|prefix| url.starts_with(prefix))
}

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

/// How the handoff reached its task.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Wake {
    Resumed,
    AlreadyActive,
    Created,
}

/// A session-card link's target.
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub session_id: String,
    pub title: Option<String>,
    pub wake: Wake,
    pub last_active: Option<String>,
}

/// `parseSessionCardHref`, behind the renderer's `session-card/` prefix check.
pub fn parse_card(href: &str) -> Option<Card> {
    strip(href, "session-card/")?;
    let url = Url::parse(href).ok()?;
    if url.host_str() != Some("session-card") {
        return None;
    }
    let id = decode_uri_component(url.path().trim_start_matches('/'))?;
    let id = js_trim(&id);
    let get = |name: &str| {
        let mut pairs = url.query_pairs();
        pairs.find(|(key, _)| key == name).map(|(_, value)| value)
    };
    let wake = match get("wake")?.as_ref() {
        "resumed" => Wake::Resumed,
        "already-active" => Wake::AlreadyActive,
        "created" => Wake::Created,
        _ => return None,
    };
    let (title, last) = (get("title")?, get("last_active")?);
    (!id.is_empty()).then(|| Card {
        session_id: id.to_owned(),
        title: Some(title.into_owned()).filter(|title| !js_trim(title).is_empty()),
        wake,
        last_active: Some(last.into_owned()).filter(|last| last != "null"),
    })
}

#[cfg(test)]
pub mod tests;
