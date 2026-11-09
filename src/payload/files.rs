//! `extractPayloadToolResultFiles`; byte indices, as the lexer compares ASCII only.

use std::sync::LazyLock;

use regex::Regex;

use super::{File, parse_tool_result};
use crate::{
    rich_answer::js::{self, Js},
    theme_import::color::{JS_SPACE, js_trim},
};

fn space_set() -> &'static str {
    JS_SPACE.trim_start_matches('[').trim_end_matches(']')
}

static URL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r#"xdt-file://[^{}"<>\\)]+"#, space_set())).expect("valid pattern")
});
static DECLARED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"^(?:xdt-file://[^{}]+|cindy-media://blobs/[0-9a-f]{{64}}\.glb)$",
        space_set()
    ))
    .expect("valid pattern")
});
static DRIVE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z]:[\\/]").expect("valid pattern"));
static MARKER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i:(?-u:\b)(?:br|rb|fr|rf|[rubf])|\$@|@\$|[$@])$").expect("valid pattern")
});
static OPERATOR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:[=\[(,?]|=>|&&|\|\||(?-u:\b)return)$").expect("valid pattern")
});
static CONDITIONAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\?(?:[^;\r\n]|\r?\n[ \t])*:$").expect("valid pattern"));
static KEY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r#"[{{,]{JS_SPACE}*(?:[A-Za-z0-9_$]+|["'][^"']+["']){JS_SPACE}*:$"#
    ))
    .expect("valid pattern")
});
static SEMICOLON: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!("^{JS_SPACE}*;")).expect("valid pattern"));

const QUOTES: [u8; 3] = *b"\"'`";

/// `absoluteToolFilePath`.
fn absolute_path(url: &str) -> Option<String> {
    let path = match url.strip_prefix("xdt-file://") {
        Some(rest) if url.starts_with("xdt-file:///") => {
            crate::cindy_json::decode_uri_component(rest).unwrap_or_else(|| rest.to_owned())
        }
        _ => {
            let parsed = url::Url::parse(url).ok()?;
            let found = parsed.query_pairs().find(|(k, _)| k == "path");
            found.map(|(_, v)| v.into_owned()).unwrap_or_default()
        }
    };
    (path.starts_with('/') || DRIVE.is_match(&path)).then_some(path)
}

struct Token {
    start: usize,
    end: usize,
    comment: bool,
    closed: bool,
}

/// `sourceContextTokens`.
fn tokens(text: &str) -> Vec<Token> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i..].starts_with(b"xdt-file://")
            && let Some(m) = URL.find_at(text, i).filter(|m| m.start() == i)
        {
            i = m.end();
            continue;
        }
        let start = i;
        let quote = b[i];
        if QUOTES.contains(&quote) {
            let triple = quote != b'`' && b[i..].starts_with(&[quote; 3]);
            let delimiter: &[u8] = if triple { &b[i..i + 3] } else { &b[i..i + 1] };
            i += delimiter.len();
            let mut closed = false;
            while i < b.len() {
                if b[i] == b'\\' {
                    i = (i + 2).min(b.len());
                } else if b[i..].starts_with(delimiter) {
                    i += delimiter.len();
                    closed = true;
                    break;
                } else {
                    i += 1;
                }
            }
            out.push(Token {
                start,
                end: i,
                comment: false,
                closed,
            });
        } else if b[i..].starts_with(b"/*") {
            let closing = text[i + 2..].find("*/").map(|at| i + 2 + at);
            i = closing.map_or(b.len(), |at| at + 2);
            out.push(Token {
                start,
                end: i,
                comment: true,
                closed: closing.is_some(),
            });
        } else if b[i..].starts_with(b"//") && (i == 0 || (b[i - 1] != b':' && b[i - 1] != b'/')) {
            i += 2;
            while i < b.len() && b[i] != b'\r' && b[i] != b'\n' {
                i += 1;
            }
            out.push(Token {
                start,
                end: i,
                comment: true,
                closed: true,
            });
        } else {
            i += 1;
        }
    }
    out
}

/// `withoutSourceComments`: comments blanked, line breaks and offsets kept.
fn without_comments(text: &str) -> String {
    let mut out = text.as_bytes().to_vec();
    for token in tokens(text).iter().filter(|t| t.comment) {
        for byte in &mut out[token.start..token.end] {
            if *byte != b'\r' && *byte != b'\n' {
                *byte = b' ';
            }
        }
    }
    String::from_utf8(out).expect("whole characters were blanked")
}

/// `sourceLiteralPrefix`.
fn literal_prefix(text: &str, quote_index: usize) -> &str {
    let raw = &text[..quote_index];
    let marker = (text.as_bytes()[quote_index] != b'`')
        .then(|| MARKER.find(raw))
        .flatten();
    marker
        .map_or(raw, |m| &raw[..m.start()])
        .trim_end_matches(crate::theme_import::color::js_space)
}

/// `looksLikeQuotedSourceLiteral`.
fn quoted_source_literal(text: &str, index: usize, end: usize) -> bool {
    let b = text.as_bytes();
    let Some(&before) = index.checked_sub(1).and_then(|i| b.get(i)) else {
        return false;
    };
    if !QUOTES.contains(&before) {
        return false;
    }
    let prefix = literal_prefix(text, index - 1);
    OPERATOR.is_match(prefix)
        || (prefix.ends_with(':') && CONDITIONAL.is_match(prefix))
        || KEY.is_match(prefix)
        || (b.get(end) == Some(&before) && SEMICOLON.is_match(&text[end + 1..]))
}

/// `quotedSourceRanges`.
fn quoted_source_ranges(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut previous: Option<(usize, bool)> = None;
    for token in tokens(text).iter().filter(|t| !t.comment) {
        let start = token.start;
        let end = token.end - usize::from(token.closed);
        let continues = previous.is_some_and(|(previous_end, _)| {
            let prefix = literal_prefix(text, start);
            let separator = js_trim(prefix.get(previous_end + 1..).unwrap_or(""));
            separator == "," || separator == "+"
        });
        let source = match previous {
            Some((_, source)) if continues => source,
            _ => quoted_source_literal(text, start + 1, end),
        };
        if source {
            out.push((start, end));
        }
        previous = Some((end, source));
    }
    out
}

/// The end of a reference wrapped in `quote`, where the same quote closes it.
fn closing_quote(url: &str, quote: u8) -> Option<usize> {
    let b = url.as_bytes();
    (0..b.len()).filter(|&i| b[i] == quote).find(|&i| {
        let mut j = i + 1;
        while j < b.len() && b",.;:!?]}".contains(&b[j]) {
            j += 1;
        }
        j == b.len() || (QUOTES.contains(&b[j]) && b[j + 1..].starts_with(b"xdt-file://"))
    })
}

struct Files(Vec<File>);

impl Files {
    fn add(&mut self, url: Option<&Js>, title: Option<&Js>) {
        let Some(url) = url.and_then(Js::str).filter(|u| DECLARED.is_match(u)) else {
            return;
        };
        let mut name = url.rsplit('/').next().unwrap_or(url).to_owned();
        if url.starts_with("xdt-file://") {
            let Some(path) = absolute_path(url) else {
                return;
            };
            let base = path.rsplit(['\\', '/']).next().unwrap_or("");
            if !base.is_empty() {
                name = base.to_owned();
            }
        }
        if self.0.iter().any(|f| f.url == url) {
            return;
        }
        let title = title
            .and_then(Js::str)
            .filter(|t| !js_trim(t).is_empty())
            .map_or(name, str::to_owned);
        self.0.push(File {
            url: url.to_owned(),
            title,
        });
    }

    fn scan(&mut self, value: &str) {
        if !value.contains("xdt-file://") {
            return;
        }
        let context = without_comments(value);
        let ranges = quoted_source_ranges(&context);
        let mut range = ranges.iter().peekable();
        let mut previous: Option<(usize, bool)> = None;
        let mut at = 0;
        while let Some(m) = URL.find_at(value, at) {
            at = m.end();
            let start = m.start();
            if context.as_bytes()[start] != value.as_bytes()[start] {
                continue;
            }
            while range.next_if(|(_, end)| *end < start).is_some() {}
            if range.peek().is_some_and(|(s, e)| *s < start && start < *e) {
                continue;
            }
            let quote = start.checked_sub(1).map(|i| value.as_bytes()[i]);
            let cut = quote
                .filter(|q| *q == b'\'' || *q == b'`')
                .and_then(|q| closing_quote(m.as_str(), q));
            let url = &m.as_str()[..cut.unwrap_or(m.len())];
            at = start + url.len();
            let continues = previous.is_some_and(|(end, _)| {
                value
                    .as_bytes()
                    .get(end)
                    .is_some_and(|c| QUOTES.contains(c))
                    && quote.is_some_and(|q| QUOTES.contains(&q))
                    && js_trim(&without_comments(
                        value.get(end + 1..start - 1).unwrap_or(""),
                    )) == ","
            });
            let source = match previous {
                Some((_, source)) if continues => source,
                _ => quoted_source_literal(&context, start, start + url.len()),
            };
            if !source {
                self.add(Some(&Js::Str(url.to_owned())), None);
            }
            previous = Some((start + url.len(), source));
        }
    }
}

/// `extractPayloadToolResultFiles`.
pub fn tool_result_files(text: &str) -> Vec<File> {
    let parsed = parse_tool_result(text);
    let field = |key: &str| parsed.as_ref().and_then(|p| p.get(key));
    if field("_xdt_render_image") == Some(&Js::Bool(false)) {
        return Vec::new();
    }
    let mut files = Files(Vec::new());
    for raw in field("_xdt_model_files")
        .and_then(Js::arr)
        .unwrap_or_default()
    {
        let file = raw.obj();
        files.add(
            file.and_then(|f| f.get("url")),
            file.and_then(|f| f.get("name")),
        );
    }
    for url in field("xdt_media_produced")
        .and_then(Js::arr)
        .unwrap_or_default()
    {
        files.add(Some(url), None);
    }
    let content = match parsed {
        Some(fields) => Js::Obj(fields),
        None => js::parse(text).unwrap_or_else(|_| Js::Str(text.to_owned())),
    };
    let mut pending: Vec<&Js> = vec![&content];
    while let Some(value) = pending.pop() {
        match value {
            Js::Str(s) => files.scan(s),
            Js::Arr(items) => pending.extend(items.iter().rev()),
            Js::Obj(fields) => pending.extend(
                js::entries(fields)
                    .filter(|(k, _)| *k != "_xdt_model_files" && *k != "xdt_media_produced")
                    .map(|(_, v)| v)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev(),
            ),
            _ => {}
        }
    }
    files.0
}
