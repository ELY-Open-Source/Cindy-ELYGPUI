//! `extractDocumentArtifactMetadata`: what a document tool says of the file it made.

use crate::rich_answer::js::{self, Js, Obj, non_empty};

/// `DocumentArtifactPreview`.
#[derive(Clone, Debug, PartialEq)]
pub enum Preview {
    Sheet {
        rows: Vec<Vec<String>>,
        has_header: bool,
    },
    Slide {
        title: Option<String>,
        subtitle: Option<String>,
    },
}

/// `DocumentArtifactMetadata`.
#[derive(Clone, Debug, PartialEq)]
pub struct Artifact {
    pub format: &'static str,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub theme: Option<&'static str>,
    pub cover: Option<bool>,
    pub summary: Option<(&'static str, f64)>,
    pub preview: Option<Preview>,
}

const SUMMARY_KINDS: [&str; 5] = ["pages", "slides", "sheets", "rows", "bytes"];
const THEMES: [&str; 3] = ["light", "dark", "navy"];
const TOOLS: [&str; 4] = ["make_docx", "make_pptx", "make_xlsx", "render_pdf"];

/// `documentToolName`.
fn document_tool(tool_name: &str) -> Option<&'static str> {
    let normalized = match tool_name.strip_prefix("mcp__") {
        Some(rest) => format!("mcp:{rest}"),
        None => tool_name.to_owned(),
    }
    .replace("__", ":");
    let name = normalized.rsplit(':').next()?;
    TOOLS.into_iter().find(|tool| *tool == name)
}

/// `parseToolResult`: the result's JSON when it is an object.
pub fn parse_result(content: Option<&str>) -> Option<Obj> {
    match &mut js::parse(content?).ok()? {
        Js::Obj(fields) => Some(std::mem::take(fields)),
        _ => None,
    }
}

/// `isExplicitFailedToolResult`.
pub fn failed(content: Option<&str>) -> bool {
    if content.is_some_and(|c| c.contains("<tool_use_error>")) {
        return true;
    }
    let Some(parsed) = parse_result(content) else {
        return false;
    };
    let no = Some(&Js::Bool(false));
    let status = parsed.get("status").and_then(Js::str).unwrap_or_default();
    parsed.get("ok") == no
        || parsed.get("success") == no
        || ["error", "failed", "failure"].contains(&status.to_lowercase().as_str())
}

fn record<'a>(fields: Option<&'a Obj>, key: &str) -> Option<&'a Obj> {
    fields?.get(key)?.obj()
}

/// `stringField`.
fn field<'a>(fields: Option<&'a Obj>, key: &str) -> Option<&'a str> {
    non_empty(fields?.get(key))
}

fn list<'a>(fields: Option<&'a Obj>, key: &str) -> Option<&'a [Js]> {
    fields?.get(key)?.arr()
}

fn finite(value: Option<&Js>) -> Option<f64> {
    match value {
        Some(Js::Num(n)) if n.is_finite() => Some(*n),
        _ => None,
    }
}

/// `String(value).slice(0, 48)`; a pair cut in half shows as U+FFFD.
fn cut(text: &str) -> String {
    let (mut out, mut units) = (String::new(), 0);
    for c in text.chars() {
        if units + c.len_utf16() > 48 {
            if units < 48 {
                out.push('\u{FFFD}');
            }
            break;
        }
        units += c.len_utf16();
        out.push(c);
    }
    out
}

/// `previewCellText`.
fn cell(mut value: &Js) -> String {
    loop {
        match value {
            Js::Str(_) | Js::Num(_) | Js::Bool(_) => return cut(&js::string(Some(value))),
            Js::Obj(fields) => match fields.get("result").or_else(|| fields.get("text")) {
                Some(inner) => value = inner,
                None => return String::new(),
            },
            Js::Null | Js::Arr(_) => return String::new(),
        }
    }
}

/// `sheetPreview`.
fn sheet(input: Option<&Obj>) -> Option<Preview> {
    let first = list(input, "sheets")?.first()?.obj();
    first?;
    let cells = |row: &[Js]| row.iter().take(3).map(cell).collect::<Vec<_>>();
    let header = list(first, "header").map(cells).unwrap_or_default();
    let has_header = !header.is_empty();
    let body = list(first, "rows").unwrap_or_default();
    let body = body.iter().take(if has_header { 3 } else { 4 });
    let mut rows: Vec<Vec<String>> = has_header.then_some(header).into_iter().collect();
    rows.extend(body.filter_map(Js::arr).map(cells));
    (!rows.is_empty()).then_some(Preview::Sheet { rows, has_header })
}

/// `extractDocumentArtifactMetadata`; a result that is there must say `ok: true`.
pub fn extract(tool_name: &str, input: Option<&Js>, result: Option<&str>) -> Option<Artifact> {
    let name = document_tool(tool_name)?;
    let input = input.and_then(Js::obj);
    let parsed = parse_result(result);
    let parsed = parsed.as_ref();
    if result.is_some() && parsed.and_then(|r| r.get("ok")) != Some(&Js::Bool(true)) {
        return None;
    }
    let artifact = record(parsed, "artifact");
    let first = |key: &str| list(input, key).and_then(|all| all.first()?.obj());
    let said = |key: &str| {
        field(artifact, key)
            .or_else(|| field(parsed, key))
            .or_else(|| field(input, key))
    };
    let title = said("title").or_else(|| match name {
        "make_pptx" => field(first("slides"), "title"),
        "make_xlsx" => field(first("sheets"), "name"),
        _ => None,
    });
    let theme = said("theme").and_then(|theme| THEMES.into_iter().find(|t| *t == theme));
    let stated = record(artifact, "summary").or_else(|| record(parsed, "summary"));
    let kind = stated.and_then(|s| s.get("kind")?.str());
    let kind = kind.and_then(|kind| SUMMARY_KINDS.into_iter().find(|k| *k == kind));
    let value = stated.and_then(|s| finite(s.get("value")));
    let counted = |kind, key: &str| Some((kind, list(input, key)?.len() as f64));
    let summary = kind.zip(value).or_else(|| match name {
        "make_pptx" => counted("slides", "slides"),
        "make_xlsx" => counted("sheets", "sheets"),
        _ => Some(("bytes", finite(parsed?.get("bytes"))?)),
    });
    let preview = match name {
        "make_xlsx" => sheet(input),
        "make_pptx" => Some(Preview::Slide {
            title: field(first("slides"), "title").map(str::to_owned),
            subtitle: field(first("slides"), "subtitle").map(str::to_owned),
        }),
        _ => None,
    };
    let flag = |fields: Option<&Obj>| match fields?.get("cover")? {
        Js::Bool(cover) => Some(*cover),
        _ => None,
    };
    Some(Artifact {
        format: match name {
            "make_docx" => "docx",
            "make_pptx" => "pptx",
            "make_xlsx" => "xlsx",
            _ => "pdf",
        },
        title: title.map(str::to_owned),
        subtitle: said("subtitle").map(str::to_owned),
        theme,
        cover: flag(artifact).or_else(|| flag(input)),
        summary,
        preview,
    })
}
