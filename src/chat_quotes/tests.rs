use super::{Quote, Segment, format, join_text, segments, strip_marker_lines};
use crate::rich_answer::js::{self, Js, Obj};

/// Runs `run` on each recorded argument list of `name` in the sent-text golden.
pub fn golden(name: &str, run: impl Fn(&[Js]) -> Js) {
    let all = js::parse(include_str!("../../tests/fixtures/sent-text/expected.json")).unwrap();
    let cases = all.obj().unwrap()[name].arr().unwrap();
    assert!(!cases.is_empty(), "{name}");
    for case in cases {
        let [args, expected] = case.arr().unwrap() else {
            panic!("case shape")
        };
        assert_eq!(
            js::stringify(&run(args.arr().unwrap()), ""),
            js::stringify(expected, ""),
            "{name} {}",
            js::stringify(args, "")
        );
    }
}

pub fn text(text: &str) -> Js {
    Js::Str(text.to_owned())
}

pub fn object(fields: Vec<(&str, Js)>) -> Js {
    let fields = fields
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value));
    Js::Obj(Obj::from_iter(fields))
}

pub fn string(value: &Js, key: &str) -> Option<String> {
    value.obj()?.get(key)?.str().map(str::to_owned)
}

pub fn number(value: &Js, key: &str) -> Option<f64> {
    match value.obj()?.get(key)? {
        Js::Num(n) => Some(*n),
        _ => None,
    }
}

fn quote(value: &Js) -> Quote {
    Quote {
        text: string(value, "text").unwrap(),
        source_path: string(value, "sourcePath"),
        start_line: number(value, "startLine"),
        end_line: number(value, "endLine"),
    }
}

pub fn segment(value: &Js) -> Segment {
    match string(value, "kind").unwrap().as_str() {
        "text" => Segment::Text(string(value, "text").unwrap()),
        _ => Segment::Quote(quote(&value.obj().unwrap()["quote"])),
    }
}

fn shown(segment: &Segment) -> Js {
    let quote = match segment {
        Segment::Text(body) => return object(vec![("kind", text("text")), ("text", text(body))]),
        Segment::Quote(quote) => quote,
    };
    let mut fields = vec![("text", text(&quote.text))];
    fields.extend(
        quote
            .source_path
            .as_deref()
            .map(|path| ("sourcePath", text(path))),
    );
    fields.extend(quote.start_line.map(|line| ("startLine", Js::Num(line))));
    fields.extend(quote.end_line.map(|line| ("endLine", Js::Num(line))));
    object(vec![("kind", text("quote")), ("quote", object(fields))])
}

#[test]
fn quotes_are_written_and_read_as_cindys() {
    golden("formatQuoteForSend", |args| text(&format(&quote(&args[0]))));
    golden("parseChatQuoteSegments", |args| {
        Js::Arr(segments(args[0].str().unwrap()).iter().map(shown).collect())
    });
}

#[test]
fn the_text_outside_quotes_joins_as_cindys() {
    golden("joinChatQuoteTextSegments", |args| {
        text(&join_text(&segments(args[0].str().unwrap())))
    });
    golden("stripChatQuoteMarkerLines", |args| {
        text(&strip_marker_lines(args[0].str().unwrap()))
    });
}

#[test]
fn a_source_shows_its_file_name_and_lines() {
    golden("quoteSourceDisplayLabel", |args| {
        super::source_label(&quote(&args[0])).map_or(Js::Null, |label| text(&label))
    });
}
