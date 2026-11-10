use super::{dangerous, downgraded, extension};
use crate::rich_answer::js::{self, Js};

/// Runs `found` on every case of `function` and returns how many there were.
fn golden(function: &str, found: fn(&Js) -> Js) -> usize {
    let all = include_str!("../../tests/fixtures/attachment-safety/expected.json");
    let all = js::parse(all).unwrap();
    let cases = all.obj().unwrap()[function].arr().unwrap();
    for case in cases {
        let [args, expected] = case.arr().unwrap() else {
            panic!("case shape")
        };
        assert_eq!(
            &found(&args.arr().unwrap()[0]),
            expected,
            "{function} {args:?}"
        );
    }
    cases.len()
}

#[test]
fn extensions_are_cindys() {
    let found = |name: &Js| Js::Str(extension(name.str().unwrap()));
    assert!(golden("attachmentExtension", found) > 80);
}

#[test]
fn dangerous_names_are_cindys() {
    let found = |name: &Js| Js::Bool(dangerous(name.str().unwrap()));
    assert!(golden("isDangerousAttachmentName", found) > 80);
}

#[test]
fn downgraded_files_are_cindys() {
    let found = |file: &Js| {
        let field = |key: &str| file.obj().unwrap()[key].str().unwrap();
        Js::Bool(downgraded(field("name"), field("path")))
    };
    assert!(golden("isSafetyDowngradedAttachment", found) > 15);
}
