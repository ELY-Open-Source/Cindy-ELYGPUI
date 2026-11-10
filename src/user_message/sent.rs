//! `UserMessage.tsx`'s sent text: the tokens its saved ranges cut, and the chips its words name.

use std::{ops::Range, sync::LazyLock};

use regex::Regex;

use crate::{
    chat_quotes::{self, Segment},
    i18n::Translator,
    js_re, js_sort,
    rich_answer::js::{Js, utf16_len},
    theme_import::color::js_space,
};

/// A saved range of the text, in UTF-16 units as JavaScript wrote it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub start: f64,
    pub end: f64,
}

/// `PastedTextRange`.
pub struct Pasted {
    pub span: Span,
    pub display: String,
}

/// What an `AgentInputReference` shows.
pub enum Kind {
    Message {
        text: Option<String>,
    },
    Session {
        title: Option<String>,
    },
    Project {
        name: String,
        working_dir: String,
    },
    BrowserTab {
        title: Option<String>,
        url: String,
    },
    DesktopWindow {
        title: Option<String>,
        app_name: String,
    },
    PluginResource {
        label: String,
        resource_id: String,
    },
    Bot {
        name: String,
        bot_id: String,
    },
    Todo {
        title: String,
        todo_id: String,
    },
}

/// `AgentInputReference`.
pub struct Reference {
    pub span: Span,
    pub kind: Kind,
}

fn field<'a>(value: &'a Js, key: &str) -> Option<&'a Js> {
    value.obj()?.get(key)
}

fn word(value: &Js, key: &str) -> Option<String> {
    field(value, key)?.str().map(str::to_owned)
}

impl Span {
    /// A range as `mapServerMessages` kept it.
    pub fn read(value: &Js) -> Span {
        let number = |key| match field(value, key) {
            Some(Js::Num(n)) => *n,
            other => panic!("a stored range's {key} is {other:?}"),
        };
        Span {
            start: number("start"),
            end: number("end"),
        }
    }
}

impl Pasted {
    pub fn read(value: &Js) -> Pasted {
        Pasted {
            span: Span::read(value),
            display: word(value, "display").expect("a pasted range has its words"),
        }
    }
}

impl Reference {
    /// A reference as `readAgentInputReferences` kept it.
    pub fn read(value: &Js) -> Reference {
        let own = |key: &str| {
            word(value, key).unwrap_or_else(|| panic!("a stored reference lacks {key}"))
        };
        let kind = match own("kind").as_str() {
            "message" => Kind::Message {
                text: word(value, "text"),
            },
            "session" => Kind::Session {
                title: word(value, "title"),
            },
            "project" => Kind::Project {
                name: own("name"),
                working_dir: own("workingDir"),
            },
            "browser-tab" => Kind::BrowserTab {
                title: word(value, "title"),
                url: own("url"),
            },
            "desktop-window" => Kind::DesktopWindow {
                title: word(value, "title"),
                app_name: own("appName"),
            },
            "plugin-resource" => Kind::PluginResource {
                label: own("label"),
                resource_id: own("resourceId"),
            },
            "bot" => Kind::Bot {
                name: own("name"),
                bot_id: own("botId"),
            },
            "todo" => Kind::Todo {
                title: own("title"),
                todo_id: own("todoId"),
            },
            other => panic!("a stored reference of kind {other}"),
        };
        Reference {
            span: Span::read(value),
            kind,
        }
    }
}

/// `SentInlineToken`; a reference is its place in the list given.
#[derive(Debug, PartialEq)]
pub enum Token<'a> {
    Text(String),
    Slash(String),
    Pasted { text: String, display: &'a str },
    Reference { text: String, index: usize },
}

#[derive(Clone, Copy)]
enum Cut {
    Pasted(usize),
    Slash,
    Reference(usize),
}

/// `String.prototype.slice`'s bounds; a cast takes `NaN` to 0 and stops at the largest index.
fn bounds(length: usize, start: f64, end: f64) -> Range<usize> {
    let at = |n: f64| (n.trunc() as usize).min(length);
    at(start)..at(end)
}

/// The ranges' sort order: by start, a reference first, then by end.
fn order(a: (Span, Cut), b: (Span, Cut)) -> f64 {
    let reference = |cut: Cut| matches!(cut, Cut::Reference(_));
    match (reference(a.1), reference(b.1)) {
        _ if a.0.start != b.0.start => a.0.start - b.0.start,
        (true, false) => -1.0,
        (false, true) => 1.0,
        _ => a.0.end - b.0.end,
    }
}

/// The text's pieces in UTF-16 units: what lies between the ranges, and each range kept.
fn cuts(
    length: usize,
    pasted: &[Pasted],
    slash: &[Span],
    references: &[Reference],
) -> Vec<(Range<usize>, Option<Cut>)> {
    let mut ranges: Vec<(Span, Cut)> = Vec::new();
    let paste = |(at, p): (usize, &Pasted)| (p.span, Cut::Pasted(at));
    ranges.extend(pasted.iter().enumerate().map(paste));
    ranges.extend(slash.iter().map(|span| (*span, Cut::Slash)));
    let refer = |(at, r): (usize, &Reference)| (r.span, Cut::Reference(at));
    ranges.extend(references.iter().enumerate().map(refer));
    js_sort::sort(&mut ranges, |a, b| order(a, b) < 0.0);
    let mut out = Vec::new();
    let mut cursor = 0.0;
    for (Span { start, end }, cut) in ranges {
        if start < cursor || end > length as f64 || end <= start {
            continue;
        }
        if start > cursor {
            out.push((bounds(length, cursor, start), None));
        }
        out.push((bounds(length, start, end), Some(cut)));
        cursor = end;
    }
    if cursor < length as f64 {
        out.push((bounds(length, cursor, f64::INFINITY), None));
    }
    out
}

/// `buildSentInlineTokens`: an overlapping range is dropped; half a cut pair is U+FFFD.
pub fn tokens<'a>(
    content: &str,
    pasted: &'a [Pasted],
    slash: &[Span],
    references: &[Reference],
) -> Vec<Token<'a>> {
    let units: Vec<u16> = content.encode_utf16().collect();
    let token = |(range, cut): (Range<usize>, Option<Cut>)| {
        let text = String::from_utf16_lossy(&units[range]);
        match cut {
            None => Token::Text(text),
            Some(Cut::Slash) => Token::Slash(text),
            Some(Cut::Reference(index)) => Token::Reference { text, index },
            Some(Cut::Pasted(at)) => Token::Pasted {
                text,
                display: &pasted[at].display,
            },
        }
    };
    let cuts = cuts(units.len(), pasted, slash, references);
    cuts.into_iter().map(token).collect()
}

/// `oneLine`.
fn one_line(value: &str) -> String {
    let words: Vec<&str> = value.split(js_space).filter(|w| !w.is_empty()).collect();
    words.join(" ")
}

/// `sentAgentReferenceDisplayLabel`.
pub fn label(reference: &Reference, t: &Translator) -> String {
    let or = |first: &str, second: &str| match one_line(first) {
        first if first.is_empty() => second.to_owned(),
        first => first,
    };
    let optional = |text: &Option<String>| one_line(text.as_deref().unwrap_or_default());
    match &reference.kind {
        Kind::Message { text } => {
            let text = match optional(text) {
                text if text.is_empty() => t.t("chat.userMessage.messageLinkFallback", &[]),
                text => text,
            };
            let units: Vec<u16> = text.encode_utf16().collect();
            match units.len() <= 240 {
                true => text,
                false => format!("{}…", String::from_utf16_lossy(&units[..239])),
            }
        }
        Kind::Session { title } => match optional(title) {
            title if title.is_empty() => t.t("chat.userMessage.sessionLinkFallback", &[]),
            title => title,
        },
        Kind::Project { name, working_dir } => or(name, working_dir),
        Kind::BrowserTab { title, url } => or(title.as_deref().unwrap_or_default(), url),
        Kind::DesktopWindow { title, app_name } => {
            or(title.as_deref().unwrap_or_default(), &one_line(app_name))
        }
        Kind::PluginResource { label, resource_id } => or(label, resource_id),
        Kind::Bot { name, bot_id } => or(name, bot_id),
        Kind::Todo { title, todo_id } => or(title, todo_id),
    }
}

/// `slashCommandDisplayLabel`: a stored command without its runtime prefix.
pub fn slash_label(command: &str) -> String {
    static PREFIX: LazyLock<Regex> =
        LazyLock::new(|| js_re::regex(r"^(?:skill|cindy|cindy-plugin-[^\s:/]+):", true));
    let name = command.strip_prefix('/');
    let rest = name.and_then(|name| Some(&name[PREFIX.find(name)?.end()..]));
    // The pattern's `(?=\S+$)`.
    match rest.filter(|rest| !rest.is_empty() && !rest.contains(js_space)) {
        Some(rest) => format!("/{rest}"),
        None => command.to_owned(),
    }
}

/// `projectSentInlinePlainText`: pasted runs and references as their chips' words.
pub fn inline_plain(
    content: &str,
    pasted: &[Pasted],
    references: &[Reference],
    label: impl Fn(&Reference) -> String,
) -> String {
    // Joined in units, so two halves that meet make their character again.
    let units: Vec<u16> = content.encode_utf16().collect();
    let mut out: Vec<u16> = Vec::new();
    for (range, cut) in cuts(units.len(), pasted, &[], references) {
        match cut {
            Some(Cut::Pasted(at)) => out.extend(pasted[at].display.encode_utf16()),
            Some(Cut::Reference(at)) => out.extend(label(&references[at]).encode_utf16()),
            Some(Cut::Slash) | None => out.extend(&units[range]),
        }
    }
    String::from_utf16_lossy(&out)
}

/// `locateChatQuoteTextSegmentStarts`: where each text segment starts, in UTF-16 units.
pub fn text_starts(content: &str, segments: &[Segment]) -> Vec<Option<usize>> {
    let mut cursor = 0;
    let mut find = |needle: &str| {
        let at = cursor + content[cursor..].find(needle)?;
        cursor = at + needle.len();
        Some(utf16_len(&content[..at]))
    };
    let starts = segments.iter().map(|segment| match segment {
        Segment::Text(text) => find(text),
        Segment::Quote(quote) => find(&chat_quotes::format(quote)).and(None),
    });
    starts.collect()
}

/// `projectSentRanges` for one range: it moved into a segment's own units, if it lies inside.
pub fn within(span: Span, source_start: Option<usize>, text_length: usize) -> Option<Span> {
    let start = source_start? as f64;
    let inside = span.start >= start && span.end <= start + text_length as f64;
    inside.then_some(Span {
        start: span.start - start,
        end: span.end - start,
    })
}

/// `looksLikePath`.
fn looks_like_path(reference: &str) -> bool {
    static EXTENSION: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\.(?-u:\w){1,10}\z").expect("a valid pattern"));
    reference.contains('/') || EXTENSION.is_match(reference) || reference.starts_with('.')
}

/// `looksLikeCommand`.
fn looks_like_command(word: &str) -> bool {
    let mut chars = word.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && word.len() <= 30
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// A piece of a text token: plain words, or the chip its `/command` or `@mention` draws.
#[derive(Debug, PartialEq)]
pub enum Part {
    Text(String),
    Command(String),
    Agent(String),
    Dir { label: String, path: String },
    File { name: String, path: String },
}

/// `parseMentionToken`'s `ref`.
fn mention(token: &str) -> String {
    match token
        .strip_prefix("@\"")
        .and_then(|rest| rest.strip_suffix('"'))
    {
        Some(quoted) => quoted.replace("\\\"", "\""),
        None => token[1..].to_owned(),
    }
}

/// The chip a mention draws, if it reads as a path.
fn chip(reference: String) -> Option<Part> {
    let last = |path: &str| {
        path.rsplit(['\\', '/'])
            .next()
            .unwrap_or_default()
            .to_owned()
    };
    if !looks_like_path(&reference) {
        return None;
    }
    if let Some(agent) = reference.strip_prefix(".claude/agents/") {
        return Some(Part::Agent(
            agent.strip_suffix(".md").unwrap_or(agent).to_owned(),
        ));
    }
    Some(match reference.strip_suffix('/') {
        Some(dir) => {
            let name = dir.rsplit(['\\', '/']).find(|part| !part.is_empty());
            Part::Dir {
                label: format!("{}/", name.unwrap_or(&reference)),
                path: reference,
            }
        }
        None => Part::File {
            name: Some(last(&reference))
                .filter(|name| !name.is_empty())
                .unwrap_or(reference.clone()),
            path: reference,
        },
    })
}

/// `renderContentWithoutPastedText`'s split; `legacy` reads a `/command` at a line's start.
pub fn parts(content: &str, legacy: bool) -> Vec<Part> {
    static TOKEN: LazyLock<Regex> = LazyLock::new(|| {
        let line = r"[^\n\r\x{2028}\x{2029}]";
        js_re::regex(&format!(r#"@"(?:\\{line}|[^"\\])*"|@\S+"#), false)
    });
    let mut out: Vec<Part> = Vec::new();
    // Each piece stays apart, as each is scanned for links alone.
    let text = |out: &mut Vec<Part>, piece: &str| {
        if !piece.is_empty() {
            out.push(Part::Text(piece.to_owned()));
        }
    };
    for (at, mut line) in content.split('\n').enumerate() {
        if at > 0 {
            text(&mut out, "\n");
        }
        let word = line
            .strip_prefix('/')
            .map(|rest| rest.split(js_space).next().unwrap_or(rest));
        if let Some(word) = word.filter(|word| legacy && looks_like_command(word)) {
            out.push(Part::Command(format!("/{word}")));
            line = &line[1 + word.len()..];
        }
        // `split` with a capture: the text before each token, the token, then the rest.
        let mut pieces: Vec<&str> = Vec::new();
        let mut from = 0;
        for found in TOKEN.find_iter(line) {
            pieces.extend([&line[from..found.start()], found.as_str()]);
            from = found.end();
        }
        pieces.push(&line[from..]);
        for (at, piece) in pieces.iter().enumerate() {
            let before = at.checked_sub(1).map(|at| pieces[at]).unwrap_or_default();
            let starts_word = before.is_empty() || before.ends_with(js_space);
            let drawn = (piece.starts_with('@') && starts_word).then(|| chip(mention(piece)));
            match drawn.flatten() {
                Some(part) => out.push(part),
                None => text(&mut out, piece),
            }
        }
    }
    out
}

#[cfg(test)]
mod tests;
