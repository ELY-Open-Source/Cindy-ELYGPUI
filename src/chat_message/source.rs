//! `messageSource.ts`'s readers: the sending device, plugin and group.

use super::one_line;
use crate::{
    rich_answer::js::{Js, Obj, utf16_len},
    theme_import::color::js_trim,
};

/// `sanitizeSourceName`: one line, 80 units at most.
fn name(value: Option<&Js>) -> Option<String> {
    let raw = value?.str()?;
    let replaced: String = raw
        .chars()
        .map(|c| match c {
            '\0'..='\u{1f}' | '\u{7f}' | '\u{2028}' | '\u{2029}' => ' ',
            '「' | '」' => '"',
            c => c,
        })
        .collect();
    let cleaned = one_line(&replaced);
    if cleaned.is_empty() {
        return None;
    }
    if utf16_len(&cleaned) <= 80 {
        return Some(cleaned);
    }
    let units: Vec<u16> = cleaned.encode_utf16().take(79).collect();
    Some(format!("{}…", String::from_utf16_lossy(&units)))
}

/// `readString`: a trimmed, nonblank string.
fn text<'a>(record: &'a Obj, key: &str) -> Option<&'a str> {
    let value = js_trim(record.get(key)?.str()?);
    (!value.is_empty()).then_some(value)
}

fn record<'a>(meta: Option<&'a Obj>, key: &str) -> Option<&'a Obj> {
    meta?.get(key)?.obj()
}

fn with_name(mut out: Obj, raw: &Obj) -> Js {
    if let Some(name) = name(raw.get("name")) {
        out.insert("name".into(), Js::Str(name));
    }
    Js::Obj(out)
}

/// `readMessageSourceGroup`.
pub fn group(meta: Option<&Obj>) -> Option<Js> {
    let raw = record(meta, "sourceGroup")?;
    let id = text(raw, "groupId")?;
    Some(with_name(
        Obj::from([("groupId".into(), Js::Str(id.into()))]),
        raw,
    ))
}

/// `readMessageSourceDevice`.
pub fn device(meta: Option<&Obj>) -> Option<Js> {
    let raw = record(meta, "sourceDevice")?;
    let id = text(raw, "deviceId")?;
    let platform = raw
        .get("platform")?
        .str()
        .filter(|p| matches!(*p, "mobile" | "desktop"))?;
    let out = Obj::from([
        ("deviceId".into(), Js::Str(id.into())),
        ("platform".into(), Js::Str(platform.into())),
    ]);
    Some(with_name(out, raw))
}

/// `readMessageSourcePlugin`.
pub fn plugin(meta: Option<&Obj>) -> Option<Js> {
    let raw = record(meta, "sourcePlugin")?;
    let id = text(raw, "pluginId")?;
    Some(with_name(
        Obj::from([("pluginId".into(), Js::Str(id.into()))]),
        raw,
    ))
}
