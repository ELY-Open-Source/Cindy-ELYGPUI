use super::*;

fn golden() -> Js {
    js::parse(include_str!(
        "../../tests/fixtures/history-view/expected.json"
    ))
    .unwrap()
}

/// Each case of `name`: its arguments and the expected result.
fn cases<'a>(golden: &'a Js, name: &str) -> Vec<(&'a [Js], &'a Js)> {
    let list = golden.obj().unwrap()[name].arr().unwrap();
    assert!(!list.is_empty(), "{name} has cases");
    list.iter()
        .map(|c| (c.arr().unwrap()[0].arr().unwrap(), &c.arr().unwrap()[1]))
        .collect()
}

#[test]
fn projections_match_cindys() {
    let golden = golden();
    for (args, expected) in cases(&golden, "projectHistoryView") {
        let rows = args[0].arr().unwrap();
        let flag = |i: usize| args.get(i).is_some_and(|v| truthy(Some(v)));
        let got = Js::Arr(
            project(rows, flag(1), flag(2))
                .iter()
                .map(json::item)
                .collect(),
        );
        assert_eq!(
            js::stringify(&got, " "),
            js::stringify(expected, " "),
            "{}",
            js::stringify(&args[0], "")
        );
    }
}

#[test]
fn subagent_scopes_match_cindys() {
    let golden = golden();
    for (args, expected) in cases(&golden, "historySubagentScopes") {
        let root = args.get(1).and_then(Js::str);
        let got = subagent_scopes(args[0].arr().unwrap(), root);
        let want: HashMap<String, String> = expected
            .obj()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.str().unwrap().to_owned()))
            .collect();
        assert_eq!(got, want, "{}", js::stringify(&args[0], ""));
    }
}

#[test]
fn turn_completion_matches_cindys() {
    let golden = golden();
    for (args, expected) in cases(&golden, "completesHistoryTurn") {
        assert_eq!(
            &Js::Bool(completes_turn(&args[0])),
            expected,
            "{}",
            js::stringify(&args[0], "")
        );
    }
}
