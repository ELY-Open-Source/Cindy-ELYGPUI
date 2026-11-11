use super::{format, rel};
use crate::{
    markdown_target::Place,
    rich_answer::js::{self, Js},
};

fn golden(name: &str) -> Vec<Js> {
    let all = js::parse(include_str!(
        "../../tests/fixtures/file-location/expected.json"
    ))
    .unwrap();
    all.obj().unwrap()[name].arr().unwrap().to_vec()
}

fn text(value: &Js) -> Option<String> {
    value.str().map(str::to_owned)
}

#[test]
fn a_relative_path_is_cindys() {
    let cases = golden("toWorkdirRel");
    assert!(cases.len() > 300);
    for case in &cases {
        let [args, expected] = case.arr().unwrap() else {
            panic!("case shape")
        };
        let args = args.arr().unwrap();
        let (workdir, path) = (args[0].str().unwrap(), args[1].str().unwrap());
        assert_eq!(rel(workdir, path), text(expected), "{workdir:?} {path:?}");
    }
}

/// A number a `Place` can hold: `Ok(None)` when absent, `Err` for one `splitLocalLineSuffix` never gives.
fn number(location: &Js, key: &str) -> Result<Option<u32>, ()> {
    match location.obj().unwrap().get(key) {
        None => Ok(None),
        Some(Js::Num(n)) if n.fract() == 0.0 && (1.0..=9_999_999.0).contains(n) => {
            Ok(Some(*n as u32))
        }
        Some(_) => Err(()),
    }
}

#[test]
fn a_location_is_cindys() {
    let mut ran = 0;
    for case in &golden("formatFileLocation") {
        let [args, expected] = case.arr().unwrap() else {
            panic!("case shape")
        };
        let args = args.arr().unwrap();
        let (workdir, location) = (args[0].str().unwrap(), &args[1]);
        let path = location.obj().unwrap()["absPath"].str().unwrap();
        let (Ok(line), Ok(column)) = (number(location, "line"), number(location, "column")) else {
            continue;
        };
        ran += 1;
        let got = format(workdir, path, Place { line, column });
        assert_eq!(got, text(expected), "{workdir:?} {path:?}");
    }
    assert!(ran > 20, "{ran}");
}
