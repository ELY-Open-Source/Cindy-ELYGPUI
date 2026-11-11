//! `shared/remoteMediaUrl.ts`: the address a `cindy-remote-media://` URL wraps.

use url::Url;

use crate::rich_answer::js::{self, Js};

/// `atob`: forgiving base64, the bytes of its binary string.
fn atob(input: &str) -> Option<Vec<u8>> {
    let space = |c: &char| matches!(c, '\t' | '\n' | '\x0c' | '\r' | ' ');
    let mut data: Vec<char> = input.chars().filter(|c| !space(c)).collect();
    if data.len().is_multiple_of(4) {
        let padding = data.iter().rev().take(2).take_while(|c| **c == '=').count();
        data.truncate(data.len() - padding);
    }
    if data.len() % 4 == 1 {
        return None;
    }
    let (mut out, mut bits, mut held) = (Vec::new(), 0u32, 0u32);
    for c in data {
        let value = match c {
            'A'..='Z' => c as u32 - 'A' as u32,
            'a'..='z' => c as u32 - 'a' as u32 + 26,
            '0'..='9' => c as u32 - '0' as u32 + 52,
            '+' => 62,
            '/' => 63,
            _ => return None,
        };
        (bits, held) = ((bits << 6) | value, held + 6);
        if held >= 8 {
            held -= 8;
            out.push((bits >> held) as u8);
        }
    }
    Some(out)
}

/// `fromBase64Url`: `TextDecoder` replaces what is no UTF-8 and drops a leading mark.
fn from_base64_url(input: &str) -> Option<String> {
    let bytes = atob(&input.replace('-', "+").replace('_', "/"))?;
    let text = String::from_utf8_lossy(&bytes);
    Some(text.strip_prefix('\u{feff}').unwrap_or(&text).to_owned())
}

/// A JSON text with each lone surrogate escape as U+FFFD's, which a Rust string can hold.
fn mended(token: &str) -> String {
    let bytes = token.as_bytes();
    let unit = |at: usize| {
        let hex = bytes.get(at + 2..at + 6)?;
        let escaped = bytes[at] == b'\\' && bytes[at + 1] == b'u';
        let hex = std::str::from_utf8(hex).ok().filter(|_| escaped)?;
        u32::from_str_radix(hex, 16)
            .ok()
            .filter(|_| !hex.starts_with('+'))
    };
    let (mut out, mut at) = (Vec::with_capacity(bytes.len()), 0);
    while at < bytes.len() {
        let high = |unit: &u32| (0xD800..0xDC00).contains(unit);
        let low = |unit: &u32| (0xDC00..0xE000).contains(unit);
        let step = match (bytes[at], unit(at)) {
            (b'\\', Some(first)) if high(&first) && unit(at + 6).is_some_and(|u| low(&u)) => 12,
            (b'\\', Some(first)) if high(&first) || low(&first) => {
                out.extend_from_slice(b"\\ufffd");
                at += 6;
                continue;
            }
            // Any other escape goes on with the character it escapes.
            (b'\\', _) => 2,
            _ => 1,
        };
        let end = (at + step).min(bytes.len());
        out.extend_from_slice(&bytes[at..end]);
        at = end;
    }
    String::from_utf8(out).expect("ASCII replaced ASCII")
}

/// Whether `decodeRemoteMediaOriginToken` reads an origin: a JSON object must name one.
fn names_origin(token: &str) -> bool {
    if token.is_empty() {
        return false;
    }
    // `JSON.parse` takes a lone surrogate escape; its fields are read for emptiness alone.
    let parsed = token
        .starts_with('{')
        .then(|| js::parse(&mended(token)).ok());
    let Some(Some(Js::Obj(record))) = &parsed else {
        return true;
    };
    let text = |key: &str| record.get(key).and_then(Js::str).filter(|s| !s.is_empty());
    match record.get("k").and_then(Js::str) {
        Some("ssh") => text("id").is_some() && text("wd").is_some(),
        Some("dev") => text("id").is_some(),
        _ => false,
    }
}

/// `parseRemoteMediaUrl(url)?.origUrl`.
pub fn orig(url: &str) -> Option<String> {
    if !url.starts_with("cindy-remote-media://") {
        return None;
    }
    let parsed = Url::parse(url).ok()?;
    // `URL.host` holds the port too.
    if parsed.host_str() != Some("m") || parsed.port().is_some() {
        return None;
    }
    let path = parsed.path().trim_start_matches('/');
    let mut segments = path.split('/');
    let (token, wrapped) = (segments.next()?, segments.next()?);
    if segments.next().is_some() || token.is_empty() || wrapped.is_empty() {
        return None;
    }
    let token = from_base64_url(token)?;
    let wrapped = from_base64_url(wrapped)?;
    (names_origin(&token) && !wrapped.is_empty()).then_some(wrapped)
}

#[cfg(test)]
mod tests;
