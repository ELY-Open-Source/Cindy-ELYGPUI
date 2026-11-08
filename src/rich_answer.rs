//! `richAnswer.ts`'s `richAnswerText`: an answer's `cindy-ui` blocks written out as Markdown
//! text, for readers without the component renderer.

pub(crate) mod js;
mod stream;
mod values;

use std::sync::LazyLock;

use regex::Regex;

use crate::theme_import::color::{JS_SPACE, js_trim};
use js::Js;
use stream::{Doc, Node, text_prop};
use values::{Value, Values, string};

const LANGUAGE: &str = "cindy-ui";

static OPEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^ {0,3}(`{3,}|~{3,})([^\r\n]*)$").expect("valid pattern"));
static NOT_WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[^A-Za-z0-9_+-]").expect("valid pattern"));
static BACKTICKS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`+").expect("valid pattern"));

/// `RichPart`: Markdown as written, or a `cindy-ui` block's operations.
#[derive(Debug, PartialEq)]
enum Part<'a> {
    Markdown(&'a str),
    Rich(&'a str),
}

struct Fence {
    close: Regex,
    rich: bool,
}

/// `splitRichAnswer`: only top-level `cindy-ui` fences split; one inside another fence stays.
fn split(source: &str) -> Vec<Part<'_>> {
    let mut parts = Vec::new();
    let (mut cursor, mut start, mut rich_start) = (0, 0, None);
    let mut fence: Option<Fence> = None;
    for line in source.split_inclusive('\n') {
        let body = line
            .strip_suffix('\n')
            .map_or(line, |l| l.strip_suffix('\r').unwrap_or(l));
        match &fence {
            None => {
                if let Some(open) = OPEN.captures(body) {
                    let marks = &open[1];
                    let rich = js_trim(&open[2]) == LANGUAGE;
                    let close = format!(
                        "^ {{0,3}}{}{{{},}}{JS_SPACE}*$",
                        regex::escape(&marks[..1]),
                        marks.len()
                    );
                    fence = Some(Fence {
                        close: Regex::new(&close).expect("valid pattern"),
                        rich,
                    });
                    if rich {
                        if cursor > start {
                            parts.push(Part::Markdown(&source[start..cursor]));
                        }
                        rich_start = Some(cursor);
                        start = cursor + line.len();
                    }
                }
            }
            Some(open) if open.close.is_match(body) => {
                if open.rich && rich_start.is_some() {
                    parts.push(Part::Rich(&source[start..cursor]));
                    start = cursor + line.len();
                    rich_start = None;
                }
                fence = None;
            }
            Some(_) => {}
        }
        cursor += line.len();
    }
    if rich_start.is_some() {
        parts.push(Part::Rich(&source[start..]));
    } else if start < source.len() {
        parts.push(Part::Markdown(&source[start..]));
    }
    parts
}

/// `richAnswerText`.
pub fn text(source: &str) -> String {
    split(source)
        .into_iter()
        .map(|part| match part {
            Part::Markdown(markdown) => markdown.to_owned(),
            Part::Rich(ops) => {
                let doc = Doc::replay(ops);
                let mut values = Values::resolve(&doc);
                let roots: Vec<String> = doc
                    .roots
                    .iter()
                    .map(|id| visit(&doc, &mut values, id))
                    .filter(|t| !t.is_empty())
                    .collect();
                roots.join("\n\n")
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn visit(doc: &Doc, values: &mut Values, id: &str) -> String {
    let Some(node) = doc.nodes.get(id) else {
        return String::new();
    };
    if let Some(markdown) = markdown(node) {
        let caption = string(node, "caption", "");
        return match caption {
            "" => markdown,
            caption => format!("{markdown}\n{caption}"),
        };
    }
    let label = string(node, "label", "");
    if !node.children.is_empty() {
        let children = node.children.iter().map(|c| visit(doc, values, c));
        let all: Vec<String> = std::iter::once(label.to_owned())
            .chain(children)
            .filter(|t| !t.is_empty())
            .collect();
        return all.join("\n\n");
    }
    match node.kind {
        "svg" => string(node, "label", string(node, "alt", "")).to_owned(),
        "heading" => format!("### {}", values.label(node)),
        "icon" => [label, string(node, "text", "")]
            .into_iter()
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        "text" => values.label(node),
        "divider" => "---".into(),
        "stat" => format!("{label}: {}", values.label(node)),
        "chart" => std::iter::once(label.to_owned())
            .chain(chart(node, values).map(|(l, v)| format!("{l}: {v}")))
            .collect::<Vec<_>>()
            .join("\n"),
        kind => {
            let bound = string(node, "bind", id);
            let current = current(values, bound);
            if kind == "check" {
                let checked = current.as_ref().is_some_and(Current::truthy);
                let mark = if checked { "☑" } else { "☐" };
                return format!("{mark} {}", string(node, "label", string(node, "text", "")));
            }
            match current {
                None => label.to_owned(),
                Some(current) => format!("{label}: {current}"),
            }
        }
    }
}

/// `values[key]`: an own value, or what a plain object inherits from `Object.prototype`.
enum Current {
    Own(Value),
    Inherited(String),
}

impl Current {
    fn truthy(&self) -> bool {
        match self {
            Current::Own(value) => value.truthy(),
            Current::Inherited(_) => true,
        }
    }
}

impl std::fmt::Display for Current {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Current::Own(value) => write!(f, "{value}"),
            Current::Inherited(text) => f.write_str(text),
        }
    }
}

/// None is `undefined`, as a key present without a value is.
fn current(values: &Values, key: &str) -> Option<Current> {
    if let Some(value) = values.get(key) {
        return value.clone().map(Current::Own);
    }
    const METHODS: [&str; 10] = [
        "__defineGetter__",
        "__defineSetter__",
        "hasOwnProperty",
        "__lookupGetter__",
        "__lookupSetter__",
        "isPrototypeOf",
        "propertyIsEnumerable",
        "toString",
        "valueOf",
        "toLocaleString",
    ];
    match key {
        "__proto__" => Some(Current::Inherited("[object Object]".into())),
        "constructor" => Some(Current::Inherited(
            "function Object() { [native code] }".into(),
        )),
        key if METHODS.contains(&key) => Some(Current::Inherited(format!(
            "function {key}() {{ [native code] }}"
        ))),
        _ => None,
    }
}

/// `richChart`: up to 100 rows whose value is a finite number.
fn chart<'a>(
    node: &'a Node,
    values: &'a mut Values,
) -> impl Iterator<Item = (String, String)> + 'a {
    let rows = node.props.get("data").and_then(Js::arr).unwrap_or(&[]);
    rows.iter()
        .take(100)
        .filter_map(Js::obj)
        .filter_map(|item| {
            let label = item.get("label").and_then(Js::str).unwrap_or("").to_owned();
            let value = match values.evaluate(item.get("value"), 0) {
                Some(Value::Str(s)) => crate::cindy_json::js_to_number(&s),
                Some(Value::Num(n)) => n,
                Some(Value::Bool(b)) => f64::from(u8::from(b)),
                None => f64::NAN,
            };
            value
                .is_finite()
                .then(|| (label, crate::cindy_json::js_number(value)))
        })
}

/// `richMarkdown`: the kinds that are Markdown already.
fn markdown(node: &Node) -> Option<String> {
    let text = text_prop(&node.props);
    let fenced = |language: &str| {
        let run = BACKTICKS
            .find_iter(text)
            .map(|m| m.len() + 1)
            .max()
            .unwrap_or(0);
        let fence = "`".repeat(run.max(3));
        format!("{fence}{language}\n{text}\n{fence}")
    };
    Some(match node.kind {
        "markdown" => text.to_owned(),
        "code" => fenced(&NOT_WORD.replace_all(string(node, "language", ""), "")),
        "diagram" => fenced("mermaid"),
        "math" => format!("$$\n{text}\n$$"),
        "image" | "video" | "audio" => {
            let src: String = string(node, "src", "")
                .chars()
                .filter(|c| !matches!(c, '\n' | '\r' | '<' | '>'))
                .collect();
            let alt: String = string(node, "alt", "")
                .chars()
                .filter(|c| !matches!(c, '[' | ']' | '\\' | '\n' | '\r'))
                .collect();
            if src.is_empty() {
                String::new()
            } else {
                format!("![{alt}](<{src}>)")
            }
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests;
