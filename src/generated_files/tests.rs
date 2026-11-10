use super::{
    Artifact, File,
    artifact::{self, Preview},
    collect, failed,
};
use crate::rich_answer::js::{self, Js, Obj};

pub fn text(value: &str) -> Js {
    Js::Str(value.to_owned())
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
        let rows: Vec<&Js> = args[0].arr().unwrap().iter().collect();
        let files = collect(&rows, args[1].str().unwrap());
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
