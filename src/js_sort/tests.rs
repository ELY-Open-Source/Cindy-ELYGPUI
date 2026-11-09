//! Golden `js-sort`: V8's sort with `compareMessageTimeline`'s order on seeded arrays.

use super::sort;
use crate::rich_answer::js::{self, Js};

fn field(value: &Js) -> f64 {
    match value {
        Js::Num(n) => *n,
        _ => f64::NAN,
    }
}

/// `compareMessageTimeline` with a missing time last and a missing rowid never deciding.
fn less(a: (f64, f64), b: (f64, f64)) -> bool {
    let time = |t: f64| if t.is_nan() { f64::INFINITY } else { t };
    for diff in [time(a.0) - time(b.0), a.1 - b.1] {
        if !diff.is_nan() && diff != 0.0 {
            return diff < 0.0;
        }
    }
    false
}

#[test]
fn every_case_sorts_as_v8_does() {
    let cases = js::parse(include_str!("../../tests/fixtures/js-sort/cases.json")).unwrap();
    let mut failures = Vec::new();
    for case in cases.arr().unwrap() {
        let case = case.obj().unwrap();
        let items: Vec<(f64, f64)> = case["items"]
            .arr()
            .unwrap()
            .iter()
            .map(|p| (field(&p.arr().unwrap()[0]), field(&p.arr().unwrap()[1])))
            .collect();
        let mut order: Vec<usize> = (0..items.len()).collect();
        sort(&mut order, |a, b| less(items[a], items[b]));
        let want: Vec<usize> = case["order"]
            .arr()
            .unwrap()
            .iter()
            .map(|i| field(i) as usize)
            .collect();
        if order != want {
            failures.push(format!(
                "{} of {}",
                js::string(case.get("shape")),
                items.len()
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join(", "));
}
