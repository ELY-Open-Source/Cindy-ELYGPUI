use super::{Kind, glyph, kind};
use crate::rich_answer::js::{self, Js};

fn name(kind: Kind) -> &'static str {
    match kind {
        Kind::Code => "code",
        Kind::Text => "text",
        Kind::Pdf => "pdf",
        Kind::Document => "document",
        Kind::Sheet => "sheet",
        Kind::Slide => "slide",
        Kind::Image => "image",
        Kind::Audio => "audio",
        Kind::Video => "video",
        Kind::Archive => "archive",
        Kind::Database => "database",
        Kind::File => "file",
    }
}

/// Runs `shown` on every case of `function` and returns how many there were.
fn golden(function: &str, shown: fn(Kind) -> &'static str) -> usize {
    let all = include_str!("../../tests/fixtures/file-presentation/expected.json");
    let all = js::parse(all).unwrap();
    let cases = all.obj().unwrap()[function].arr().unwrap();
    for case in cases {
        let [args, expected] = case.arr().unwrap() else {
            panic!("case shape")
        };
        let input = args.arr().unwrap()[0].obj().unwrap();
        let field = |key| input.get(key).and_then(Js::str).unwrap_or_default();
        let found = shown(kind(field("name"), field("mimeType")));
        assert_eq!(&Js::Str(found.into()), expected, "{function} {args:?}");
    }
    cases.len()
}

#[test]
fn kinds_and_glyphs_are_cindys() {
    assert!(golden("getFileVisualKind", name) > 250);
    assert!(golden("glyph", glyph) > 250);
}
