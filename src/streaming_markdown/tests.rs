//! The golden `streaming-markdown`: Cindy's functions on each text as it grows.

use super::{chunks, repair, throttle};
use crate::rich_answer::js::{self, Js};

fn golden() -> Js {
    js::parse(include_str!(
        "../../tests/fixtures/streaming-markdown/expected.json"
    ))
    .unwrap()
}

fn num(value: &Js) -> usize {
    match value {
        Js::Num(n) => *n as usize,
        other => panic!("no number: {other:?}"),
    }
}

/// The byte a prefix of `length` UTF-16 units ends at.
fn byte(text: &str, length: usize) -> usize {
    let mut units = 0;
    let ends = text.char_indices().find(|(_, c)| {
        units += c.len_utf16();
        units > length
    });
    ends.map_or(text.len(), |(at, _)| at)
}

/// Each prefix of `text` against the repairs and splits Cindy made of it.
fn grown(text: &str, made: &Js) -> usize {
    let listed = |key: &str| {
        let rows = made.obj().unwrap()[key].arr().unwrap().iter();
        let rows = rows.map(|row| row.arr().unwrap());
        rows.map(|row| (num(&row[0]), &row[1]))
            .collect::<std::collections::HashMap<_, _>>()
    };
    let (repairs, starts) = (listed("repairs"), listed("starts"));
    let units = text.encode_utf16().count();
    let mut checked = 0;
    for length in 0..=units {
        let end = byte(text, length);
        if text[..end].encode_utf16().count() != length {
            continue;
        }
        let prefix = &text[..end];
        let wanted = repairs
            .get(&length)
            .map_or(prefix, |made| made.str().unwrap());
        assert_eq!(repair(prefix), wanted, "{prefix:?}");
        let wanted: Vec<usize> = match starts.get(&length) {
            Some(made) => made.arr().unwrap().iter().map(num).collect(),
            None => vec![0],
        };
        let parts = chunks::split(prefix);
        let got: Vec<usize> = (parts.iter())
            .map(|part| prefix[..part.start].encode_utf16().count())
            .collect();
        assert_eq!(got, wanted, "{prefix:?}");
        let joined: String = parts.iter().map(|part| part.content).collect();
        assert_eq!(joined, prefix);
        checked += 1;
    }
    checked
}

#[test]
fn a_growing_text_is_repaired_and_split_as_cindy_does() {
    let all = golden();
    let all = all.obj().unwrap();
    let mut checked = 0;
    for case in all["stream"].arr().unwrap() {
        let [args, made] = case.arr().unwrap() else {
            panic!("case shape")
        };
        checked += grown(args.arr().unwrap()[0].str().unwrap(), made);
    }
    for case in all["mix"].arr().unwrap() {
        let made = &case.arr().unwrap()[1];
        checked += grown(made.obj().unwrap()["text"].str().unwrap(), made);
    }
    assert!(checked > 8_000, "{checked}");
}

#[test]
fn cindys_own_inputs_are_repaired_alike() {
    let all = golden();
    let cases = all.obj().unwrap()["repairStreamingMarkdown"].arr().unwrap();
    for case in cases {
        let [args, expected] = case.arr().unwrap() else {
            panic!("case shape")
        };
        let text = args.arr().unwrap()[0].str().unwrap();
        assert_eq!(repair(text), expected.str().unwrap(), "{text:?}");
    }
    assert!(cases.len() > 20);
}

#[test]
fn the_interval_grows_with_the_text() {
    let all = golden();
    for case in all.obj().unwrap()["interval"].arr().unwrap() {
        let [args, expected] = case.arr().unwrap() else {
            panic!("case shape")
        };
        let length = num(&args.arr().unwrap()[0]);
        assert_eq!(throttle(&"a".repeat(length)) as usize, num(expected));
    }
}
