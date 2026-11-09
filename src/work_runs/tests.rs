use super::*;
use crate::rich_answer::js::{self, Js, Obj, truthy};

/// `scripts/golden/workRuns.mjs`'s toy adapter.
struct Toy {
    fold: Option<bool>,
}

fn flag(item: &Js, key: &str) -> bool {
    truthy(item.obj().and_then(|o| o.get(key)))
}

fn ms(item: &Js, key: &str) -> Option<f64> {
    match item.obj().and_then(|o| o.get(key)) {
        Some(Js::Num(n)) => Some(*n),
        _ => None,
    }
}

fn id(item: &Js) -> Js {
    item.obj()
        .and_then(|o| o.get("id"))
        .cloned()
        .unwrap_or(Js::Null)
}

fn made(run: Vec<Js>, next: Option<&Js>, last: (&str, Js), boundary: Option<f64>) -> Js {
    let mut out = Obj::new();
    out.insert("group".into(), Js::Arr(run.iter().map(id).collect()));
    out.insert("next".into(), next.map_or(Js::Null, id));
    out.insert(last.0.into(), last.1);
    out.insert("boundary".into(), boundary.map_or(Js::Null, Js::Num));
    Js::Obj(out)
}

impl Adapter for Toy {
    type Item = Js;
    fn is_user_boundary(&self, i: &Js) -> bool {
        flag(i, "user")
    }
    fn is_continuation_boundary(&self, i: &Js) -> bool {
        flag(i, "cont")
    }
    fn is_answer(&self, i: &Js) -> bool {
        flag(i, "answer")
    }
    fn can_prepend_to_answer(&self, i: &Js) -> bool {
        !flag(i, "commentary")
    }
    fn is_sealed_answer(&self, i: &Js) -> bool {
        flag(i, "sealed")
    }
    fn is_compact_boundary(&self, i: &Js) -> bool {
        flag(i, "compact")
    }
    fn is_activity(&self, i: &Js) -> bool {
        flag(i, "act")
    }
    fn is_archivable(&self, i: &Js) -> bool {
        flag(i, "act") || flag(i, "arch")
    }
    fn start_timestamp(&self, i: &Js) -> Option<f64> {
        ms(i, "ms")
    }
    fn end_timestamp(&self, i: &Js) -> Option<f64> {
        ms(i, "end").or_else(|| ms(i, "ms"))
    }
    fn boundary_timestamp(&self, i: Option<&Js>) -> Option<f64> {
        i.and_then(|i| ms(i, "ms"))
    }
    fn user_boundary_end(&self, i: &Js, previous: Option<f64>) -> Option<f64> {
        ms(i, "ms").or(previous)
    }
    fn create_group(
        &self,
        run: Vec<Js>,
        next: Option<&Js>,
        streaming: bool,
        boundary: Option<f64>,
    ) -> Js {
        made(run, next, ("streaming", Js::Bool(streaming)), boundary)
    }
    fn create_completed_group(&self, run: Vec<Js>, next: Option<&Js>, boundary: Option<f64>) -> Js {
        made(run, next, ("completed", Js::Bool(true)), boundary)
    }
    fn fold_answer_tail(&self) -> bool {
        self.fold != Some(false)
    }
}

#[test]
fn groups_match_cindys() {
    let golden = js::parse(include_str!("../../tests/fixtures/work-runs/expected.json")).unwrap();
    let cases = golden.obj().unwrap()["groupWorkRuns"].arr().unwrap();
    assert!(!cases.is_empty());
    for case in cases {
        let [args, expected] = case.arr().unwrap() else {
            panic!("case shape")
        };
        let args = args.arr().unwrap();
        let fold = match &args[2] {
            Js::Bool(b) => Some(*b),
            _ => None,
        };
        let items = args[0].arr().unwrap().to_vec();
        let got = Js::Arr(group(items, args[1] == Js::Bool(true), &Toy { fold }));
        assert_eq!(
            js::stringify(&got, ""),
            js::stringify(expected, ""),
            "{}",
            js::stringify(&args[0], "")
        );
    }
}
