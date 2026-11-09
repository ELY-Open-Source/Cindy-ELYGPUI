//! `payloadSummary.ts`'s tool result readers: delivered media, files and card references.
#![cfg_attr(
    not(test),
    expect(dead_code, reason = "`projectHistoryView` reads them in 9.1b")
)]

use std::sync::LazyLock;

use regex::Regex;

use crate::rich_answer::js::{self, Js, Obj, utf16_len};

mod files;
pub use files::tool_result_files;

static BLOB: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^cindy-media://blobs/[0-9a-f]{64}\.([a-z0-9]+)$").expect("valid pattern")
});

/// `ExtractedPayloadToolMediaLike`.
#[derive(Debug, PartialEq)]
pub struct Media {
    pub kind: &'static str,
    pub url: String,
    pub title: Option<String>,
    pub previewable: bool,
    pub actions: Option<Actions>,
}

/// `ExtractedPayloadToolMediaActionsLike`, whose provider is always `mivo`.
#[derive(Clone, Debug, PartialEq)]
pub struct Actions {
    pub job_id: String,
    pub buttons: Vec<Button>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Button {
    pub custom_id: String,
    pub label: Option<String>,
    pub emoji: Option<String>,
}

/// `PayloadToolFile`.
#[derive(Debug, PartialEq)]
pub struct File {
    pub url: String,
    pub title: String,
}

/// `readPayloadString`: a nonempty string, untrimmed.
fn string(value: Option<&Js>) -> Option<&str> {
    value?.str().filter(|s| !s.is_empty())
}

/// `parseToolResultPayload`: only the documented `ghost_call` envelope unwraps.
pub fn parse_tool_result(text: &str) -> Option<Obj> {
    let mut parsed = js::parse(text).ok()?;
    let Js::Obj(outer) = &mut parsed else {
        return None;
    };
    let outer = std::mem::take(outer);
    let inner = match outer.get("ok") {
        Some(Js::Bool(true)) => outer.get("result").and_then(Js::obj),
        _ => None,
    };
    let Some(inner) = inner else {
        return Some(outer);
    };
    // `{ ...inner, ...outer }`: outer values win, inner keys keep their places.
    let mut out = Obj::new();
    for (k, v) in js::entries(inner).chain(js::entries(&outer)) {
        out.insert(k.to_owned(), v.clone());
    }
    if inner.get("_xdt_render_image") == Some(&Js::Bool(false)) {
        out.insert("_xdt_render_image".into(), Js::Bool(false));
    }
    Some(out)
}

/// `managedToolMediaKind`.
pub fn managed_media_kind(url: &str) -> Option<&'static str> {
    for (scheme, kind) in [
        ("xdt-image://", "image"),
        ("xdt-video://", "video"),
        ("xdt-audio://", "audio"),
    ] {
        if url.starts_with(scheme) {
            return Some(kind);
        }
    }
    let ext = BLOB.captures(url)?.get(1)?.as_str();
    match ext {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "avif" | "bmp" | "svg" => Some("image"),
        "mp4" | "webm" | "mov" | "m4v" => Some("video"),
        "mp3" | "wav" | "m4a" | "ogg" | "flac" | "aac" | "opus" => Some("audio"),
        _ => None,
    }
}

/// `parsePayloadToolMediaActions`.
fn media_actions(raw: Option<&Js>) -> Option<Actions> {
    let record = raw?.obj()?;
    if record.get("provider")?.str()? != "mivo" {
        return None;
    }
    let job_id = string(record.get("jobId"))?.to_owned();
    let buttons: Vec<Button> = record
        .get("buttons")?
        .arr()?
        .iter()
        .filter_map(|raw| {
            let button = raw.obj();
            let field = |key: &str| string(button.and_then(|b| b.get(key))).map(str::to_owned);
            Some(Button {
                custom_id: field("customId")?,
                label: field("label"),
                emoji: field("emoji"),
            })
        })
        .collect();
    (!buttons.is_empty()).then_some(Actions { job_id, buttons })
}

/// `extractPayloadToolResultMedia`.
pub fn tool_result_media(text: &str) -> Vec<Media> {
    if !text.contains("xdt_") {
        return Vec::new();
    }
    let Some(parsed) = parse_tool_result(text) else {
        return Vec::new();
    };
    if parsed.get("_xdt_render_image") == Some(&Js::Bool(false)) {
        return Vec::new();
    }
    let actions = media_actions(parsed.get("_xdt_actions"));
    let mut items: Vec<Media> = Vec::new();
    let mut push = |kind, url: &str, title: Option<&str>, actions: Option<&Actions>| {
        let previewable = ["http://", "https://", "data:image/"]
            .iter()
            .any(|p| url.starts_with(p));
        items.push(Media {
            kind,
            url: url.to_owned(),
            title: title.map(str::to_owned),
            previewable,
            actions: actions.cloned(),
        });
    };
    let managed =
        |url: &str, scheme: &str| url.starts_with(scheme) || url.starts_with("cindy-media://");
    let urls = |key: &str| -> Vec<&str> {
        parsed
            .get(key)
            .and_then(Js::arr)
            .unwrap_or_default()
            .iter()
            .filter_map(Js::str)
            .collect()
    };
    for (kind, single, many, scheme) in [
        ("image", "xdt_image_url", "xdt_image_urls", "xdt-image://"),
        ("video", "xdt_video_url", "xdt_video_urls", "xdt-video://"),
    ] {
        if let Some(url) = parsed
            .get(single)
            .and_then(Js::str)
            .filter(|u| managed(u, scheme))
        {
            push(kind, url, None, actions.as_ref());
        }
        for url in urls(many).into_iter().filter(|u| managed(u, scheme)) {
            push(kind, url, None, actions.as_ref());
        }
    }
    let tracks = parsed
        .get("xdt_audio_tracks")
        .filter(|t| **t != Js::Null)
        .or_else(|| parsed.get("_xdt_audio_tracks"));
    let mut audio = false;
    for raw in tracks.and_then(Js::arr).unwrap_or_default() {
        let track = raw.obj();
        let field = |key: &str| string(track.and_then(|t| t.get(key)));
        let url = field("xdt_audio_url").or_else(|| field("audioUrl"));
        if let Some(url) = url.filter(|u| managed(u, "xdt-audio://")) {
            push("audio", url, field("title"), None);
            audio = true;
        }
    }
    if !audio {
        for url in urls("xdt_audio_urls")
            .into_iter()
            .filter(|u| managed(u, "xdt-audio://"))
        {
            push("audio", url, None, None);
        }
    }
    for url in parsed
        .get("xdt_media_produced")
        .and_then(Js::arr)
        .unwrap_or_default()
    {
        if let Some(url) = url.str()
            && let Some(kind) = managed_media_kind(url)
        {
            push(kind, url, None, None);
        }
    }
    let mut seen = std::collections::HashSet::new();
    items.retain(|item| seen.insert(item.url.clone()));
    items
}

/// `extractPayloadToolCardIds`.
pub fn tool_card_ids(text: &str) -> Vec<String> {
    let Some(parsed) = parse_tool_result(text) else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for key in ["xdt_card_id", "xdt_anchor_card_id"] {
        if let Some(id) = parsed.get(key).and_then(Js::str)
            && (1..=128).contains(&utf16_len(id))
            && !out.iter().any(|o| o == id)
        {
            out.push(id.to_owned());
        }
    }
    out
}

#[cfg(test)]
mod tests;
