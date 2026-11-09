//! `imageRef.ts`'s `parseUserContent`: a user row's text and its attachments.

use super::{one_line, references};
use crate::{
    rich_answer::js::{self, Js, Obj, utf16_len},
    theme_import::color::js_space,
};

/// `UserMessageContent`; `None` and empty lists stand for absent fields.
#[derive(Default)]
pub struct UserContent {
    pub text: String,
    pub images: Vec<Js>,
    pub files: Vec<Js>,
    pub session_references: Vec<Js>,
    pub reply_quote: Option<Js>,
    pub quotes_encoded: bool,
    pub pasted_text_ranges: Vec<Js>,
    pub slash_command_ranges: Option<Vec<Js>>,
    pub agent_references: Vec<Js>,
}

fn plain(text: String) -> UserContent {
    UserContent {
        text,
        ..UserContent::default()
    }
}

/// `basenameForAttachment`.
fn basename(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    match normalized.rsplit('/').next() {
        Some("") | None => "attachment".into(),
        Some(name) => name.into(),
    }
}

fn managed(url: &str) -> bool {
    url.starts_with("xdt-image://") || url.starts_with("cindy-media://")
}

/// `isValidAttachmentIntegrity`.
fn integrity(size: Option<&Js>, sha256: Option<&Js>) -> bool {
    let size_ok = size
        .and_then(Js::integer)
        .is_some_and(|n| n > 0.0 && n <= 9_007_199_254_740_991.0);
    let sha_ok = sha256.and_then(Js::str).is_some_and(|s| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    });
    size_ok && sha_ok
}

/// `coerceAnnotationStrokes`.
fn strokes(value: Option<&Js>) -> Option<Js> {
    let items = value?.arr().filter(|a| !a.is_empty())?;
    let mut out = Vec::new();
    for raw in items {
        let points = raw.obj()?.get("points")?.arr().filter(|p| !p.is_empty())?;
        let mut copied = Vec::new();
        for p in points {
            let p = p.obj()?;
            let coordinate = |key: &str| match p.get(key) {
                Some(Js::Num(n)) if n.is_finite() => Some(*n),
                _ => None,
            };
            let point = Obj::from([
                ("x".into(), Js::Num(coordinate("x")?)),
                ("y".into(), Js::Num(coordinate("y")?)),
            ]);
            copied.push(Js::Obj(point));
        }
        out.push(Js::Obj(Obj::from([("points".into(), Js::Arr(copied))])));
    }
    Some(Js::Arr(out))
}

/// `coerceImageRef`.
fn image(o: &Obj) -> Option<Js> {
    let url = o.get("url")?.str().filter(|u| managed(u))?;
    let mime = o.get("mimeType")?.str()?;
    let name = o
        .get("originalName")
        .and_then(Js::str)
        .or_else(|| o.get("name").and_then(Js::str))?;
    let mut out = Obj::new();
    out.insert("url".into(), Js::Str(url.into()));
    out.insert("mimeType".into(), Js::Str(mime.into()));
    out.insert("originalName".into(), Js::Str(name.into()));
    if integrity(o.get("size"), o.get("sha256")) {
        out.insert("size".into(), o["size"].clone());
        out.insert("sha256".into(), o["sha256"].clone());
    }
    if let Some(strokes) = strokes(o.get("annotationStrokes"))
        && let Some(source) = o
            .get("annotationSourceUrl")
            .and_then(Js::str)
            .filter(|u| managed(u))
    {
        out.insert("annotationSourceUrl".into(), Js::Str(source.into()));
        out.insert("annotationStrokes".into(), strokes);
    }
    Some(Js::Obj(out))
}

/// `isValidFileRef`.
fn file_ok(o: &Obj) -> bool {
    let named =
        matches!(o.get("name"), Some(Js::Str(_))) && matches!(o.get("path"), Some(Js::Str(_)));
    let (size, sha) = (o.get("size"), o.get("sha256"));
    named
        && ((size.is_none() && sha.is_none())
            || (size.is_some() && sha.is_some() && integrity(size, sha)))
}

/// `coercePastedTextRanges` and `coerceSlashCommandRanges`: ordered spans inside the text.
fn ranges(value: &[Js], text: &str, display: bool) -> Option<Vec<Js>> {
    let length = utf16_len(text) as f64;
    let mut previous = 0.0;
    let mut out = Vec::new();
    for candidate in value {
        let range = candidate.obj()?;
        let (start, end) = (range.get("start")?.integer()?, range.get("end")?.integer()?);
        let shown = range.get("display").filter(|d| matches!(d, Js::Str(_)));
        if (display && shown.is_none())
            || start < previous
            || start < 0.0
            || end <= start
            || end > length
        {
            return None;
        }
        let mut item = Obj::from([
            ("start".into(), Js::Num(start)),
            ("end".into(), Js::Num(end)),
        ]);
        if display {
            item.insert("display".into(), shown?.clone());
        } else {
            let units: Vec<u16> = text
                .encode_utf16()
                .skip(start as usize)
                .take((end - start) as usize)
                .collect();
            let span = String::from_utf16_lossy(&units);
            let mut chars = span.chars();
            if chars.next() != Some('/') || span.len() < 2 || span[1..].chars().any(js_space) {
                return None;
            }
        }
        previous = end;
        out.push(Js::Obj(item));
    }
    Some(out)
}

/// `parsePersistedSessionReferenceMetadata`: up to eight, all or none.
fn session_references(value: Option<&Js>) -> Vec<Js> {
    let Some(items) = value.and_then(Js::arr).filter(|a| a.len() <= 8) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in items {
        let Some(i) = item.obj() else {
            return Vec::new();
        };
        let session = i
            .get("sessionId")
            .and_then(Js::str)
            .filter(|s| !s.is_empty());
        let range = i
            .get("range")
            .and_then(Js::str)
            .filter(|r| matches!(*r, "recent" | "around-anchor"));
        let count = i
            .get("messageCount")
            .and_then(Js::integer)
            .filter(|n| (0.0..=20.0).contains(n));
        let truncated = i.get("truncated").filter(|t| matches!(t, Js::Bool(_)));
        let message = i.get("messageClientId");
        let message_ok = message.is_none_or(|m| m.str().is_some_and(|m| !m.is_empty()));
        let (Some(session), Some(range), Some(count), Some(truncated), true) =
            (session, range, count, truncated, message_ok)
        else {
            return Vec::new();
        };
        let mut o = Obj::from([("sessionId".into(), Js::Str(session.into()))]);
        if let Some(m) = message {
            o.insert("messageClientId".into(), m.clone());
        }
        o.insert("range".into(), Js::Str(range.into()));
        o.insert("messageCount".into(), Js::Num(count));
        o.insert("truncated".into(), truncated.clone());
        out.push(Js::Obj(o));
    }
    out
}

/// `readChatReplyQuote` with `chatReplyQuoteExcerpt`.
fn reply_quote(value: Option<&Js>) -> Option<Js> {
    let raw = value?.obj()?;
    let client = raw
        .get("clientId")?
        .str()
        .filter(|c| !c.is_empty() && utf16_len(c) <= 256)?;
    let role = raw
        .get("role")?
        .str()
        .filter(|r| matches!(*r, "user" | "assistant"))?;
    let excerpt = raw.get("excerpt")?.str()?;
    let line = one_line(excerpt);
    let line = line.as_str();
    let excerpt = match utf16_len(line) > 280 {
        true => {
            let units: Vec<u16> = line.encode_utf16().take(279).collect();
            format!("{}…", String::from_utf16_lossy(&units))
        }
        false => line.to_owned(),
    };
    Some(Js::Obj(Obj::from([
        ("clientId".into(), Js::Str(client.into())),
        ("role".into(), Js::Str(role.into())),
        ("excerpt".into(), Js::Str(excerpt)),
    ])))
}

fn blocks(items: &[Js]) -> UserContent {
    let mut out = UserContent::default();
    let mut text = String::new();
    for block in items {
        let Some(b) = block.obj() else {
            continue;
        };
        let get = |key: &str| b.get(key).and_then(Js::str);
        let kind = get("type");
        if kind == Some("text")
            && let Some(part) = get("text")
        {
            text.push_str(part);
        }
        if kind == Some("image") {
            let name = get("originalName")
                .map(str::to_owned)
                .or_else(|| get("path").map(basename));
            let mut fields = Obj::new();
            for (key, value) in [
                ("url", b.get("url").cloned()),
                ("mimeType", b.get("mimeType").cloned()),
            ] {
                if let Some(value) = value {
                    fields.insert(key.into(), value);
                }
            }
            if let Some(name) = name {
                fields.insert("originalName".into(), Js::Str(name));
            }
            out.images.extend(image(&fields));
        }
        if kind == Some("file")
            && let Some(path) = get("path")
        {
            let name = get("name")
                .or(get("originalName"))
                .map_or_else(|| basename(path), str::to_owned);
            let file = Obj::from([
                ("name".into(), Js::Str(name)),
                ("path".into(), Js::Str(path.into())),
            ]);
            out.files.push(Js::Obj(file));
        }
    }
    out.text = text;
    out
}

/// `parseUserContent`.
pub fn parse(content: &Js) -> UserContent {
    match content {
        Js::Str(s) if s.starts_with(['{', '[']) => match js::parse(s) {
            Ok(parsed) => parse(&parsed),
            Err(_) => plain(s.clone()),
        },
        Js::Str(s) => plain(s.clone()),
        Js::Arr(items) => blocks(items),
        Js::Obj(o) => {
            let Some(Js::Str(text)) = o.get("text") else {
                return plain(js::stringify(content, ""));
            };
            let list = |key: &str| o.get(key).and_then(Js::arr).unwrap_or_default();
            UserContent {
                text: text.clone(),
                images: list("images")
                    .iter()
                    .filter_map(|i| i.obj().and_then(image))
                    .collect(),
                files: list("files")
                    .iter()
                    .filter(|f| f.obj().is_some_and(file_ok))
                    .cloned()
                    .collect(),
                session_references: session_references(o.get("sessionReferences")),
                reply_quote: reply_quote(o.get("replyQuote")),
                quotes_encoded: o.get("quotesEncoded") == Some(&Js::Bool(true)),
                pasted_text_ranges: o
                    .get("pastedTextRanges")
                    .and_then(Js::arr)
                    .and_then(|r| ranges(r, text, true))
                    .unwrap_or_default(),
                slash_command_ranges: o
                    .get("slashCommandRanges")
                    .and_then(Js::arr)
                    .and_then(|r| ranges(r, text, false)),
                agent_references: references::read_all(o.get("agentReferences"), text),
            }
        }
        Js::Null => plain(String::new()),
        other => plain(js::string(Some(other))),
    }
}
