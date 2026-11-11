//! `MarkdownRenderer.tsx`'s URL transforms over react-markdown's `defaultUrlTransform`.

use crate::deep_link;

/// Which transform a renderer runs with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Urls {
    /// `trustedUrlTransform`.
    Trusted,
    /// `previewSafeUrlTransform`.
    Preview,
    /// `allowlistedImageUrlTransform`.
    Allowlisted,
}

fn starts_ignoring_case(text: &str, prefix: &str) -> bool {
    (text.as_bytes().get(..prefix.len()))
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix.as_bytes()))
}

/// `/^[a-z][a-z0-9+.-]*:/i`.
fn schemed(url: &str) -> bool {
    let name = url.split(':').next().unwrap_or_default();
    name.len() < url.len()
        && name.starts_with(|ch: char| ch.is_ascii_alphabetic())
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || "+.-".contains(ch))
}

/// `defaultUrlTransform`: a URL with no protocol, or a safe one.
fn default(url: &str) -> String {
    let Some(colon) = url.find(':') else {
        return url.to_owned();
    };
    let before = |mark: char| url.find(mark).is_some_and(|at| colon > at);
    let safe = ["http", "https", "irc", "ircs", "mailto", "xmpp"];
    let safe = safe
        .iter()
        .any(|name| url[..colon].eq_ignore_ascii_case(name));
    if before('/') || before('?') || before('#') || safe {
        return url.to_owned();
    }
    String::new()
}

fn trusted(url: &str, src: bool) -> bool {
    let schemes = [
        "xdt-image://",
        "xdt-video://",
        "cindy-media://",
        "xdt-file://",
        "xdt-audio://",
        "cindy-remote-media://",
        "file://",
    ];
    let bytes = url.as_bytes();
    // A backslash is escaped by the time a transform runs, so a drive ends in a slash.
    let drive = bytes.len() > 2 && bytes[0].is_ascii_alphabetic() && &bytes[1..3] == b":/";
    schemes.iter().any(|scheme| url.starts_with(scheme))
        || deep_link::starts(url)
        || drive
        || (src && url.starts_with("data:"))
}

impl Urls {
    /// The address a renderer gets for `url` under the property `key`.
    pub fn transform(self, url: &str, key: &str) -> String {
        let src = key == "src";
        let kept = match self {
            Urls::Trusted => trusted(url, src),
            Urls::Preview if src => {
                let web =
                    starts_ignoring_case(url, "http://") || starts_ignoring_case(url, "https://");
                return if web { url.to_owned() } else { String::new() };
            }
            Urls::Allowlisted if src => {
                let barred = schemed(url) || url.starts_with("//");
                return if barred {
                    String::new()
                } else {
                    url.to_owned()
                };
            }
            Urls::Preview | Urls::Allowlisted => false,
        };
        if kept { url.to_owned() } else { default(url) }
    }
}
