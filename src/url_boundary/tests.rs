use super::{Options, Strip, bare_http, clip, wrap_marker};
use crate::rich_answer::js::{self, Js};

/// Runs `run` on each recorded argument list of `name` in the golden.
fn golden(name: &str, run: impl Fn(&[Js]) -> Js) {
    let all = js::parse(include_str!(
        "../../tests/fixtures/url-boundary/expected.json"
    ))
    .unwrap();
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

fn text(text: &str) -> Js {
    Js::Str(text.to_owned())
}

#[test]
fn a_bare_url_ends_where_cindys_does() {
    golden("clipBareHttpAutolinkText", |args| {
        let raw = args[0].str().unwrap();
        let given = args.get(1).and_then(Js::obj);
        let field = |key: &str| given.and_then(|given| given.get(key));
        let strip = match field("stripMarkdownFormattingPunct") {
            Some(Js::Bool(true)) => Strip::Always,
            Some(Js::Str(_)) => Strip::Auto,
            _ => Strip::Never,
        };
        let options = Options {
            prefix: field("prefix").and_then(Js::str).unwrap_or_default(),
            wrap_marker: field("markdownWrapMarker").map(Js::str),
            strip_formatting: strip,
            cut_path_brackets: field("cutPathBrackets") != Some(&Js::Bool(false)),
        };
        text(&raw[..clip(raw, &options)])
    });
}

#[test]
fn the_scanner_matches_what_cindys_does() {
    golden("matchBareHttp", |args| {
        let found = bare_http().find_iter(args[0].str().unwrap());
        Js::Arr(found.map(|m| text(m.as_str())).collect())
    });
}

#[test]
fn a_prefix_opens_a_wrap_as_cindys_does() {
    golden("markdownWrapMarkerFromPrefix", |args| {
        wrap_marker(args[0].str().unwrap()).map_or(Js::Null, text)
    });
}
