use super::{
    Artifact, File,
    artifact::{self, Preview},
    collect, collect_cached, failed,
};
use std::rc::Rc;

use crate::{
    chat_message::merge::{Fields, Message},
    rich_answer::js::{self, Js, Obj},
};

pub fn text(value: &str) -> Js {
    Js::Str(value.to_owned())
}

/// A fixture's rows as the store holds them.
pub fn rows(list: &Js) -> Vec<Message> {
    let row = |row: &Js| {
        let fields = row.obj().into_iter().flatten();
        Rc::new(Fields::from_iter(
            fields.map(|(k, v)| (k.clone(), Some(Rc::new(v.clone())))),
        ))
    };
    list.arr().unwrap().iter().map(row).collect()
}

/// Rows by reference, as the turn readers take them.
pub fn refs(rows: &[Message]) -> Vec<&Fields> {
    rows.iter().map(|row| &**row).collect()
}

/// An object of the fields that hold a value, as `JSON.stringify` drops `undefined`.
pub fn object(fields: Vec<(&str, Option<Js>)>) -> Js {
    let kept = fields
        .into_iter()
        .filter_map(|(k, v)| Some((k.to_owned(), v?)));
    Js::Obj(Obj::from_iter(kept))
}

/// Runs `run` on each recorded argument list of `name` in the turn-yields golden.
pub fn yields(name: &str, run: impl Fn(&[Js]) -> Js) {
    let all = js::parse(include_str!(
        "../../tests/fixtures/turn-yields/expected.json"
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

fn shown(a: &Artifact) -> Js {
    let rows = |rows: &Vec<Vec<String>>| {
        let row = |row: &Vec<String>| Js::Arr(row.iter().map(|c| text(c)).collect());
        Js::Arr(rows.iter().map(row).collect())
    };
    let preview = a.preview.as_ref().map(|preview| match preview {
        Preview::Sheet {
            rows: all,
            has_header,
        } => object(vec![
            ("kind", Some(text("sheet"))),
            ("rows", Some(rows(all))),
            ("hasHeader", Some(Js::Bool(*has_header))),
        ]),
        Preview::Slide { title, subtitle } => object(vec![
            ("kind", Some(text("slide"))),
            ("title", title.as_deref().map(text)),
            ("subtitle", subtitle.as_deref().map(text)),
        ]),
    });
    let summary = a.summary.map(|(kind, value)| {
        object(vec![
            ("kind", Some(text(kind))),
            ("value", Some(Js::Num(value))),
        ])
    });
    object(vec![
        ("format", Some(text(a.format))),
        ("title", a.title.as_deref().map(text)),
        ("subtitle", a.subtitle.as_deref().map(text)),
        ("theme", a.theme.map(text)),
        ("cover", a.cover.map(Js::Bool)),
        ("summary", summary),
        ("preview", preview),
    ])
}

pub fn file(f: &File) -> Js {
    let source = if f.from_command { "command" } else { "tool" };
    object(vec![
        ("path", Some(text(&f.path))),
        ("name", Some(text(&f.name))),
        ("source", Some(text(source))),
        ("ready", (!f.ready).then_some(Js::Bool(false))),
        ("artifact", f.artifact.as_ref().map(shown)),
        (
            "artifactConfirmed",
            f.artifact_confirmed.then_some(Js::Bool(true)),
        ),
    ])
}

#[test]
fn generated_files_match_cindys() {
    yields("collectGeneratedFiles", |args| {
        let rows = rows(&args[0]);
        let files = collect(&refs(&rows), args[1].str().unwrap());
        Js::Arr(files.iter().map(file).collect())
    });
}

#[test]
fn document_artifacts_match_cindys() {
    yields("extractDocumentArtifactMetadata", |args| {
        let result = args.get(2).and_then(Js::str);
        let found = artifact::extract(args[0].str().unwrap(), args.get(1), result);
        found.as_ref().map_or(Js::Null, shown)
    });
}

#[test]
fn failed_results_match_cindys() {
    yields("isExplicitFailedToolResult", |args| {
        Js::Bool(failed(args[0].str()))
    });
}

#[test]
fn a_turn_is_read_once_while_its_rows_live() {
    let write = |path: &str| {
        let input = object(vec![("file_path", Some(text(path)))]);
        object(vec![
            ("role", Some(text("tool_use"))),
            ("toolName", Some(text("Write"))),
            ("toolInput", Some(input)),
        ])
    };
    let turn = rows(&Js::Arr(vec![write("/w/a.md"), write("/w/b.md")]));
    let all: Vec<&Message> = turn.iter().collect();
    let held = super::cache::held();
    let first = collect_cached(&all, "/w");
    assert_eq!(first.len(), 2);
    assert_eq!(super::cache::held(), held + 1);
    assert_eq!(collect_cached(&all, "/w"), first);
    assert_eq!(super::cache::held(), held + 1);
    assert_eq!(collect_cached(&all[..1], "/w").len(), 1);
    assert_eq!(collect_cached(&all, "/x"), collect(&refs(&turn), "/x"));
    assert_eq!(super::cache::held(), held + 1);
    drop(turn);
    let other = rows(&Js::Arr(vec![write("/w/c.md")]));
    collect_cached(&other.iter().collect::<Vec<_>>(), "/w");
    assert_eq!(super::cache::held(), held + 1);
}
