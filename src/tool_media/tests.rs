use super::{AudioTrack, Kind, Media, dedupe_by_url, extract};
use crate::{
    generated_files::tests::{object, text, yields},
    rich_answer::js::Js,
};

fn track(t: &AudioTrack) -> Js {
    let kind = if t.sound_effect {
        "sound_effect"
    } else {
        "music"
    };
    object(vec![
        ("kind", Some(text(kind))),
        ("audioUrl", Some(text(&t.audio_url))),
        ("coverUrl", t.cover_url.as_deref().map(text)),
        ("title", Some(text(&t.title))),
        ("tags", Some(text(&t.tags))),
        ("lyrics", Some(text(&t.lyrics))),
        ("durationSeconds", Some(Js::Num(t.duration_seconds))),
        ("sunoId", t.suno_id.as_deref().map(text)),
    ])
}

fn kind(kind: Kind) -> &'static str {
    match kind {
        Kind::Image => "image",
        Kind::Video => "video",
        Kind::Audio => "audio",
    }
}

pub fn shown(list: &[Media]) -> Js {
    let item = |m: &Media| {
        let model = m.model_file.as_ref().map(|model| {
            object(vec![
                ("provider", Some(text("cindy"))),
                ("url", Some(text(&model.url))),
                ("format", Some(text(&model.format))),
            ])
        });
        object(vec![
            ("kind", Some(text(kind(m.kind)))),
            ("url", Some(text(&m.url))),
            ("modelFile", model),
            ("audioTrack", m.audio_track.as_ref().map(track)),
            ("audioInCard", m.audio_in_card.then_some(Js::Bool(true))),
            ("imageInCard", m.image_in_card.then_some(Js::Bool(true))),
        ])
    };
    Js::Arr(list.iter().map(item).collect())
}

#[test]
fn tool_media_matches_cindys() {
    yields("extractToolResultMedia", |args| {
        shown(&extract(args[0].str().unwrap()))
    });
}

#[test]
fn media_dedupes_as_cindys() {
    yields("dedupeToolMediaByUrl", |args| {
        let media = |entry: &Js| Media {
            kind: [Kind::Image, Kind::Video, Kind::Audio]
                .into_iter()
                .find(|k| Some(kind(*k)) == entry.obj().unwrap()["kind"].str())
                .unwrap(),
            url: entry.obj().unwrap()["url"].str().unwrap().to_owned(),
            model_file: None,
            audio_track: None,
            audio_in_card: false,
            image_in_card: false,
        };
        let list = args[0].arr().unwrap().iter().map(media).collect();
        shown(&dedupe_by_url(list))
    });
}
