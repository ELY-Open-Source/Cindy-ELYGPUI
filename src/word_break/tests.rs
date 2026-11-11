//! Golden `word-break`: V8's segments in Cindy's Electron (`scripts/v8-words.js`).

use super::{class, split, words};
use crate::rich_answer::js::{self, Js};

/// The UTF-16 offsets where `text`'s words end, those inside a complex-script run left out.
fn ends(text: &str, cuts: impl Iterator<Item = usize>) -> Vec<usize> {
    let complex = |c: Option<char>| c.is_some_and(|c| class(c).complex);
    let units: Vec<usize> = text
        .char_indices()
        .map(|(at, _)| text[..at].encode_utf16().count())
        .chain([text.encode_utf16().count()])
        .collect();
    let bytes: Vec<usize> = text.char_indices().map(|(at, _)| at).collect();
    let inside = |unit: &usize| {
        let nth = units
            .iter()
            .position(|at| at == unit)
            .expect("a cut is a character's edge");
        let at = bytes.get(nth).copied().unwrap_or(text.len());
        complex(text[..at].chars().next_back()) && complex(text[at..].chars().next())
    };
    cuts.filter(|unit| !inside(unit)).collect()
}

#[test]
fn words_are_cut_where_v8_cuts_them() {
    let golden = js::parse(include_str!("../../tests/fixtures/word-break/v8.json")).unwrap();
    let rows = golden.obj().unwrap()["rows"].arr().unwrap();
    for row in rows {
        let [text, wanted] = row.arr().unwrap() else {
            panic!("row shape")
        };
        let text = text.str().unwrap();
        let wanted = wanted.arr().unwrap().iter().map(|end| match end {
            Js::Num(end) => *end as usize,
            other => panic!("no offset: {other:?}"),
        });
        let mut at = 0;
        let got = words(text).into_iter().map(|word| {
            at += word.encode_utf16().count();
            at
        });
        let got: Vec<usize> = got.collect();
        assert_eq!(ends(text, got.into_iter()), ends(text, wanted), "{text:?}");
    }
    assert!(rows.len() > 25_000);
}

#[test]
fn a_character_v8_holds_unassigned_stands_alone() {
    assert_eq!(words("a\u{10ef4}b"), ["a", "\u{10ef4}", "b"]);
    assert_eq!(words("\u{1b168}_"), ["\u{1b168}", "_"]);
    assert_eq!(words("\u{3031}\u{1b168}"), ["\u{3031}", "\u{1b168}"]);
}

#[test]
fn white_space_stays_with_the_word_before_it() {
    assert_eq!(split("hello world foo"), ["hello ", "world ", "foo"]);
    assert_eq!(split("  lead"), ["  ", "lead"]);
    let mixed = "**bold** 中文混排 text，标点。\n\nnew para";
    assert_eq!(split(mixed).concat(), mixed);
    assert!(split("今天天气不错我们去公园散步").len() > 1);
    assert!(split("").is_empty());
}
