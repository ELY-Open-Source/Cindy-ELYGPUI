use super::{label_start, parse_project, parse_session, unescape_label};
use crate::rich_answer::js::{self, Js, Obj, utf16_len};

/// Runs `run` on each recorded argument list of `name` in the linkify golden.
pub fn golden(name: &str, run: impl Fn(&[Js]) -> Js) {
    let all = js::parse(include_str!("../../tests/fixtures/linkify/expected.json")).unwrap();
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

#[test]
fn session_links_parse_as_cindys() {
    golden("parseSessionDeepLinkHref", |args| {
        let Some(target) = parse_session(args[0].str().unwrap()) else {
            return Js::Null;
        };
        let optional = |value: Option<String>| value.map_or(Js::Null, Js::Str);
        object(vec![
            ("sessionId", Js::Str(target.session_id)),
            ("messageClientId", optional(target.message_client_id)),
            ("deviceId", optional(target.device_id)),
        ])
    });
}

#[test]
fn project_links_parse_as_cindys() {
    golden("parseProjectDeepLinkHref", |args| {
        let dir = parse_project(args[0].str().unwrap());
        dir.map_or(Js::Null, |dir| object(vec![("workingDir", Js::Str(dir))]))
    });
}

#[test]
fn a_label_starts_where_cindys_does() {
    golden("findMarkdownLabelStart", |args| {
        let source = args[0].str().unwrap();
        let units = args[1].integer().unwrap() as usize;
        let close = source
            .char_indices()
            .map(|(at, _)| at)
            .find(|at| utf16_len(&source[..*at]) == units)
            .unwrap();
        let start = label_start(source, close);
        Js::Num(start.map_or(-1.0, |at| utf16_len(&source[..at]) as f64))
    });
}

#[test]
fn label_brackets_unescape_as_cindys() {
    golden("unescapeMarkdownLabelBrackets", |args| {
        text(&unescape_label(args[0].str().unwrap()))
    });
}
