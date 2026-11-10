//! `AgentActionRow.tsx`'s `extractToolResultMedia`: the media a tool result's JSON names.

use std::{cell::RefCell, collections::HashSet};

use indexmap::IndexMap;

use crate::rich_answer::js::{self, Js, Obj};

/// `ToolMediaKind`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Image,
    Video,
    Audio,
}

/// `ToolMediaModelFile`, whose provider is `cindy`.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelFile {
    pub url: String,
    pub format: String,
}

/// `ToolAudioTrack`.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioTrack {
    pub sound_effect: bool,
    pub audio_url: String,
    pub cover_url: Option<String>,
    pub title: String,
    pub tags: String,
    pub lyrics: String,
    pub duration_seconds: f64,
    pub suno_id: Option<String>,
}

/// `ToolMediaItem`.
#[derive(Clone, Debug, PartialEq)]
pub struct Media {
    pub kind: Kind,
    pub url: String,
    pub model_file: Option<ModelFile>,
    pub audio_track: Option<AudioTrack>,
    pub audio_in_card: bool,
    pub image_in_card: bool,
}

const MEDIA_STORE: &str = "cindy-media://";

fn fetchable(url: &str, legacy: &str) -> bool {
    url.starts_with(legacy) || url.starts_with(MEDIA_STORE)
}

fn text<'a>(fields: &'a Obj, key: &str) -> Option<&'a str> {
    fields.get(key)?.str()
}

/// A single url, then the list beside it.
fn urls<'a>(parsed: &'a Obj, one: &str, many: &str) -> impl Iterator<Item = &'a str> {
    let list = parsed.get(many).and_then(Js::arr).unwrap_or_default();
    text(parsed, one)
        .into_iter()
        .chain(list.iter().filter_map(Js::str))
}

/// `parseModelFiles`.
fn model_files(raw: Option<&Js>) -> Vec<ModelFile> {
    let entries = raw.and_then(Js::arr).unwrap_or_default();
    let model = |entry: &Js| {
        let fields = entry.obj()?;
        let url = text(fields, "url").filter(|_| text(fields, "provider") == Some("cindy"))?;
        if !url.starts_with(MEDIA_STORE) || !url.to_lowercase().ends_with(".glb") {
            return None;
        }
        let format = text(fields, "format").filter(|f| !f.is_empty());
        Some(ModelFile {
            url: url.to_owned(),
            format: format.unwrap_or("GLB").to_owned(),
        })
    };
    entries.iter().filter_map(model).collect()
}

/// `parseAudioTracks`.
fn audio_tracks(raw: Option<&Js>) -> Vec<AudioTrack> {
    let entries = raw.and_then(Js::arr).unwrap_or_default();
    let track = |entry: &Js| {
        let fields = entry.obj()?;
        let url = text(fields, "xdt_audio_url").filter(|u| fetchable(u, "xdt-audio://"))?;
        let owned = |key: &str| text(fields, key).unwrap_or_default().to_owned();
        let duration = fields.get("duration_seconds");
        Some(AudioTrack {
            sound_effect: text(fields, "kind") == Some("sound_effect"),
            audio_url: url.to_owned(),
            cover_url: text(fields, "cover_url")
                .filter(|u| fetchable(u, "xdt-image://"))
                .map(str::to_owned),
            title: owned("title"),
            tags: owned("tags"),
            lyrics: owned("lyrics"),
            duration_seconds: match duration {
                Some(Js::Num(seconds)) => *seconds,
                _ => 0.0,
            },
            suno_id: text(fields, "suno_id")
                .filter(|id| !id.is_empty())
                .map(str::to_owned),
        })
    };
    entries.iter().filter_map(track).collect()
}

fn bare_track(url: &str) -> AudioTrack {
    AudioTrack {
        sound_effect: false,
        audio_url: url.to_owned(),
        cover_url: None,
        title: String::new(),
        tags: String::new(),
        lyrics: String::new(),
        duration_seconds: 0.0,
        suno_id: None,
    }
}

const MAX_ENTRIES: usize = 512;
const MAX_CHARACTERS: usize = 32 * 1024 * 1024;

/// `toolMetadataCache`: results by their text, with its UTF-16 length; the oldest first.
#[derive(Default)]
struct Cache {
    results: IndexMap<String, (Vec<Media>, usize)>,
    characters: usize,
}

thread_local! {
    static CACHE: RefCell<Cache> = RefCell::default();
}

/// `extractToolResultMedia` through `getToolResultMetadata`'s cache.
pub fn extract(result: &str) -> Vec<Media> {
    let named = ["xdt_image_url", "xdt_video_url", "xdt_audio_url"];
    if !named.iter().any(|key| result.contains(key)) {
        return Vec::new();
    }
    let hit = CACHE.with_borrow_mut(|cache| {
        let entry = cache.results.shift_remove(result)?;
        cache.results.insert(result.to_owned(), entry.clone());
        Some(entry.0)
    });
    if let Some(media) = hit {
        return media;
    }
    let media = read(result);
    let characters = result.encode_utf16().count();
    if characters <= MAX_CHARACTERS {
        CACHE.with_borrow_mut(|cache| {
            let entry = (media.clone(), characters);
            cache.results.insert(result.to_owned(), entry);
            cache.characters += characters;
            while cache.results.len() > MAX_ENTRIES || cache.characters > MAX_CHARACTERS {
                let (_, (_, oldest)) = cache.results.shift_remove_index(0).expect("an entry");
                cache.characters -= oldest;
            }
        });
    }
    media
}

fn read(result: &str) -> Vec<Media> {
    let parsed = js::parse(result).ok();
    let Some(parsed) = parsed.as_ref().and_then(Js::obj) else {
        return Vec::new();
    };
    if parsed.get("_xdt_render_image") == Some(&Js::Bool(false)) {
        return Vec::new();
    }
    let flag = |key: &str| parsed.get(key) == Some(&Js::Bool(true));
    let item = |kind, url: &str| Media {
        kind,
        url: url.to_owned(),
        model_file: None,
        audio_track: None,
        audio_in_card: false,
        image_in_card: false,
    };
    let mut items = Vec::new();
    let mut models = model_files(parsed.get("_xdt_model_files")).into_iter();
    let images = urls(parsed, "xdt_image_url", "xdt_image_urls");
    for url in images.filter(|u| fetchable(u, "xdt-image://")) {
        items.push(Media {
            model_file: models.next(),
            image_in_card: flag("xdt_images_in_card"),
            ..item(Kind::Image, url)
        });
    }
    let videos = urls(parsed, "xdt_video_url", "xdt_video_urls");
    for url in videos.filter(|u| fetchable(u, "xdt-video://")) {
        items.push(item(Kind::Video, url));
    }
    let tracks = match parsed.get("xdt_audio_tracks") {
        None | Some(Js::Null) => parsed.get("_xdt_audio_tracks"),
        tracks => tracks,
    };
    let mut tracks = audio_tracks(tracks);
    if tracks.is_empty() {
        let bare = parsed.get("xdt_audio_urls").and_then(Js::arr);
        let bare = bare.unwrap_or_default().iter().filter_map(Js::str);
        tracks.extend(
            bare.filter(|u| fetchable(u, "xdt-audio://"))
                .map(bare_track),
        );
    }
    for track in tracks {
        let base = item(Kind::Audio, &track.audio_url);
        items.push(Media {
            audio_in_card: flag("xdt_audio_in_card"),
            audio_track: Some(track),
            ..base
        });
    }
    dedupe_by_url(items)
}

/// `dedupeToolMediaByUrl`: the first of each url, an empty url dropped.
pub fn dedupe_by_url(media: Vec<Media>) -> Vec<Media> {
    let mut seen = HashSet::new();
    media
        .into_iter()
        .filter(|item| !item.url.is_empty() && seen.insert(item.url.clone()))
        .collect()
}

#[cfg(test)]
pub(crate) mod tests;
