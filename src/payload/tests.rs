use super::*;
use crate::message_rows::tests::{cases, golden};

fn opt(out: &mut Obj, key: &str, value: Option<&String>) {
    if let Some(value) = value {
        out.insert(key.into(), Js::Str(value.clone()));
    }
}

fn media(item: &Media) -> Js {
    let mut out = Obj::new();
    out.insert("kind".into(), Js::Str(item.kind.into()));
    out.insert("url".into(), Js::Str(item.url.clone()));
    opt(&mut out, "title", item.title.as_ref());
    out.insert("previewable".into(), Js::Bool(item.previewable));
    if let Some(actions) = &item.actions {
        let buttons = actions.buttons.iter().map(|b| {
            let mut button = Obj::new();
            button.insert("customId".into(), Js::Str(b.custom_id.clone()));
            opt(&mut button, "label", b.label.as_ref());
            opt(&mut button, "emoji", b.emoji.as_ref());
            Js::Obj(button)
        });
        let mut shown = Obj::new();
        shown.insert("provider".into(), Js::Str("mivo".into()));
        shown.insert("jobId".into(), Js::Str(actions.job_id.clone()));
        shown.insert("buttons".into(), Js::Arr(buttons.collect()));
        out.insert("actions".into(), Js::Obj(shown));
    }
    Js::Obj(out)
}

fn file(item: &File) -> Js {
    let mut out = Obj::new();
    out.insert("url".into(), Js::Str(item.url.clone()));
    out.insert("title".into(), Js::Str(item.title.clone()));
    Js::Obj(out)
}

#[test]
fn media_matches_cindys() {
    let golden = golden();
    for (input, expected) in cases(&golden, "extractPayloadToolResultMedia") {
        let text = input.str().unwrap();
        let got = Js::Arr(tool_result_media(text).iter().map(media).collect());
        assert_eq!(&got, expected, "{text:?}");
    }
}

#[test]
fn files_match_cindys() {
    let golden = golden();
    for (input, expected) in cases(&golden, "extractPayloadToolResultFiles") {
        let text = input.str().unwrap();
        let got = Js::Arr(tool_result_files(text).iter().map(file).collect());
        assert_eq!(&got, expected, "{text:?}");
    }
}

#[test]
fn card_ids_match_cindys() {
    let golden = golden();
    for (input, expected) in cases(&golden, "extractPayloadToolCardIds") {
        let text = input.str().unwrap();
        let got = Js::Arr(tool_card_ids(text).into_iter().map(Js::Str).collect());
        assert_eq!(&got, expected, "{text:?}");
    }
}

#[test]
fn an_envelope_keeps_inner_keys_first_and_outer_values() {
    let parsed =
        parse_tool_result(r#"{"ok":true,"z":1,"result":{"z":2,"a":3,"_xdt_render_image":false}}"#)
            .unwrap();
    let keys: Vec<&str> = js::entries(&parsed).map(|(k, _)| k).collect();
    assert_eq!(keys, ["z", "a", "_xdt_render_image", "ok", "result"]);
    assert_eq!(parsed["z"], Js::Num(1.0));
    assert_eq!(parsed["_xdt_render_image"], Js::Bool(false));
}
