//! Golden `merge-messages`: Cindy's `mergeMessages` and `confirmRemoteUsers`, identity included.

use std::{collections::HashSet, rc::Rc};

use indexmap::IndexMap;

use super::{Field, Message, Messages, Options, RowsOrder, handoff::confirm_remote_users, merge};
use crate::rich_answer::js::{self, Js, Obj};

type Lists = IndexMap<String, Vec<Message>>;

/// The script's `resolve`: a `$ref` is that row or that row's field, shared.
fn resolve(value: &Js, lists: &Lists) -> Field {
    if let Some(at) = value.obj().and_then(|o| o.get("$ref")).and_then(Js::str) {
        let (place, key) = at.split_once('/').map_or((at, None), |(p, k)| (p, Some(k)));
        let (list, index) = place.split_once(':').unwrap();
        let row = &lists[list][index.parse::<usize>().unwrap()];
        return match key {
            Some(key) => row.get(key).cloned().flatten(),
            None => panic!("a row reference where a field goes"),
        };
    }
    Some(Rc::new(value.clone()))
}

fn rows(list: &Js, lists: &mut Lists, name: &str) -> Messages {
    lists.insert(name.into(), Vec::new());
    for row in list.arr().unwrap() {
        let row = row.obj().unwrap();
        let message = match row.get("$ref").and_then(Js::str) {
            Some(at) => {
                let (list, index) = at.split_once(':').unwrap();
                lists[list][index.parse::<usize>().unwrap()].clone()
            }
            None => Rc::new(
                row.iter()
                    .map(|(k, v)| (k.clone(), resolve(v, lists)))
                    .collect(),
            ),
        };
        lists[name].push(message);
    }
    Rc::new(lists[name].clone())
}

fn strings(value: Option<&Js>) -> Option<HashSet<String>> {
    let items = value?.arr().unwrap();
    Some(items.iter().map(|s| s.str().unwrap().to_owned()).collect())
}

fn options(value: Option<&Js>) -> Options {
    let Some(o) = value.and_then(Js::obj) else {
        return Options::default();
    };
    let flag = |key: &str| matches!(o.get(key), Some(Js::Bool(true)));
    Options {
        preserve_tool_result_content: flag("preserveExistingToolResultContent"),
        preserve_codex_plan_content: flag("preserveExistingCodexPlanContent"),
        add_only: flag("addOnly"),
        add_only_except: strings(o.get("addOnlyExcept")),
    }
}

/// The script's `describe`, as JSON.
fn describe(result: &Messages, lists: &Lists, same: bool) -> String {
    let mut out = Obj::new();
    let plain = |m: &Message| {
        let fields = m
            .iter()
            .filter_map(|(k, v)| Some((k.clone(), (**v.as_ref()?).clone())));
        Js::Obj(fields.collect())
    };
    out.insert("result".into(), Js::Arr(result.iter().map(plain).collect()));
    let origin = result.iter().map(|row| {
        lists
            .iter()
            .find_map(|(name, list)| {
                let index = list.iter().position(|m| Rc::ptr_eq(m, row))?;
                Some(Js::Str(format!("{name}:{index}")))
            })
            .unwrap_or(Js::Null)
    });
    out.insert("origin".into(), Js::Arr(origin.collect()));
    let undefined = result.iter().map(|row| {
        let keys = row.iter().filter(|(_, v)| v.is_none());
        Js::Arr(keys.map(|(k, _)| Js::Str(k.clone())).collect())
    });
    out.insert("undefinedKeys".into(), Js::Arr(undefined.collect()));
    out.insert("same".into(), Js::Bool(same));
    js::stringify(&Js::Obj(out), " ")
}

fn run(case: &Obj) -> String {
    let mut lists = Lists::new();
    if case["fn"].str() == Some("confirm") {
        let messages = rows(&case["messages"], &mut lists, "messages");
        let result = confirm_remote_users(&messages, &strings(case.get("confirmed")).unwrap());
        return describe(&result, &lists, Rc::ptr_eq(&result, &messages));
    }
    let server = rows(&case["server"], &mut lists, "server");
    let existing = rows(&case["existing"], &mut lists, "existing");
    let order = match case.get("rowsOrder").and_then(Js::str) {
        Some("newest-first") => RowsOrder::NewestFirst,
        _ => RowsOrder::OldestFirst,
    };
    let result = merge(&server, &existing, &options(case.get("options")), order);
    describe(&result, &lists, Rc::ptr_eq(&result, &existing))
}

#[test]
fn every_case_matches_cindy() {
    let cases = js::parse(include_str!(
        "../../../tests/fixtures/merge-messages/cases.json"
    ))
    .unwrap();
    let expected = js::parse(include_str!(
        "../../../tests/fixtures/merge-messages/expected.json"
    ))
    .unwrap();
    let (cases, expected) = (cases.arr().unwrap(), expected.arr().unwrap());
    assert_eq!(cases.len(), expected.len(), "merge-messages is out of sync");
    let mut failures = Vec::new();
    for (case, want) in cases.iter().zip(expected) {
        let (case, mut want) = (case.obj().unwrap(), want.obj().unwrap().clone());
        let name = want.shift_remove("name").unwrap();
        assert_eq!(case.get("name"), Some(&name));
        let (got, want) = (run(case), js::stringify(&Js::Obj(want), " "));
        if got != want {
            failures.push(format!(
                "{}\n got: {got}\nwant: {want}",
                name.str().unwrap()
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// The reviewer's input: 90M NULs encode past `String::kMaxLength`; Cindy's Electron takes `fresh`.
#[test]
fn an_unencodable_field_counts_zero() {
    let row = |content: String| {
        let mut fields = super::Fields::new();
        for (key, value) in [
            ("clientId", Js::Str("a".into())),
            ("role", Js::Str("tool_result".into())),
            ("content", Js::Str(content)),
            ("remoteContentTruncated", Js::Bool(true)),
        ] {
            fields.insert(key.into(), Some(Rc::new(value)));
        }
        Rc::new(fields)
    };
    let existing = Rc::new(vec![row("\0".repeat(90_000_000))]);
    let server = [row("fresh".into())];
    let merged = merge(
        &server,
        &existing,
        &Options::default(),
        RowsOrder::OldestFirst,
    );
    assert_eq!(
        super::get(&merged[0], "content").and_then(Js::str),
        Some("fresh")
    );
}
