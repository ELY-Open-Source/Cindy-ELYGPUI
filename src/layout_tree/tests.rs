use std::fs;

use serde_json::{Value, json};

use super::{Layout, LayoutStore, ops::TabGroup, store::FILE};
use crate::cindy_json::roundtrip;

fn fixture(name: &str) -> Value {
    let path = format!(
        "{}/tests/fixtures/layout/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    roundtrip(&fs::read_to_string(path).unwrap()).unwrap()
}

fn num(arg: &Value) -> f64 {
    match arg.as_str() {
        Some("NaN") => f64::NAN,
        Some("Infinity") => f64::INFINITY,
        _ => arg.as_f64().unwrap(),
    }
}

fn index(arg: &Value) -> usize {
    arg.as_u64().unwrap() as usize
}

fn tree(raw: &Value) -> Layout {
    let (layout, fallback) = Layout::coerce(Some(raw));
    assert_eq!(fallback, None);
    layout
}

/// The JSON Cindy's function returns for the same call.
fn run(base: &Value, op: &str, a: &[Value]) -> Value {
    let layout = tree(base);
    let s = |i: usize| a[i].as_str().unwrap();
    let edited = match op {
        "setPaneCollapsed" => layout.set_pane_collapsed(s(0), a[1].as_bool().unwrap()),
        "setSplitChildFraction" => layout.set_split_child_fraction(s(0), index(&a[1]), num(&a[2])),
        "swapRootSplitChildrenByKind" => layout.swap_root_children_by_kind(s(0), s(1)),
        "ensureTabGroupPane" => {
            let group = TabGroup::ALL.into_iter().find(|g| g.id() == s(0)).unwrap();
            layout.ensure_tab_group_pane(group)
        }
        "transferSplitFraction" => {
            layout.transfer_split_fraction(s(0), index(&a[1]), index(&a[2]), num(&a[3]))
        }
        "transferSplitFractionRelay" => {
            let sources: Vec<_> = a[1].as_array().unwrap().iter().map(index).collect();
            layout.transfer_split_fraction_relay(s(0), &sources, index(&a[2]), num(&a[3]))
        }
        "paneEdgeRelativeToChat" => {
            return json!(layout.pane_edge_relative_to_chat(s(0)).map(TabGroup::id));
        }
        "walkPanes" => {
            let panes = layout.walk_panes().into_iter();
            return panes
                .map(|p| json!({ "id": p.id, "panelKind": p.panel_kind, "collapsed": p.collapsed }))
                .collect();
        }
        "findSplitChildByPanelKind" => {
            return layout.split_child_by_kind(s(0)).map_or(Value::Null, |r| {
                json!({ "splitId": r.split_id, "childIndex": r.child_index, "fraction": r.fraction })
            });
        }
        other => panic!("unknown op {other}"),
    };
    match edited {
        Ok(next) => json!({ "layout": next.to_value(), "applied": true }),
        Err(reason) => json!({ "layout": layout.to_value(), "applied": false, "reason": reason }),
    }
}

#[test]
fn matches_cindy_on_every_case() {
    let (input, expected) = (fixture("input.json"), fixture("expected.json"));
    for (case, want) in input["coerce"]
        .as_array()
        .unwrap()
        .iter()
        .zip(expected["coerce"].as_array().unwrap())
    {
        let raw = match case.get("text") {
            Some(text) => roundtrip(text.as_str().unwrap()).ok(),
            None => Some(case["raw"].clone()),
        };
        let (layout, reason) = Layout::coerce(raw.as_ref());
        let mut got = json!({ "name": case["name"], "layout": layout.to_value(), "fallback": reason.is_some() });
        if let Some(reason) = reason {
            got["reason"] = reason.into();
        }
        assert_eq!(&got, want);
    }
    for (case, want) in input["ops"]
        .as_array()
        .unwrap()
        .iter()
        .zip(expected["ops"].as_array().unwrap())
    {
        let base = &input["bases"][case["base"].as_str().unwrap()];
        let got = run(
            base,
            case["op"].as_str().unwrap(),
            case["args"].as_array().unwrap(),
        );
        assert_eq!(&got, &want["result"], "{}", case["name"]);
    }
    let kinds = input["kinds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| json!(TabGroup::from_panel_kind(k.as_str().unwrap()).map(TabGroup::id)));
    assert_eq!(kinds.collect::<Value>(), expected["kinds"]);
    let groups: Vec<_> = TabGroup::ALL
        .iter()
        .map(|g| json!([g.id(), g.panel_kind()]))
        .collect();
    assert_eq!(Value::from(groups), expected["groups"]);
}

#[test]
fn heals_a_missing_or_broken_file_and_keeps_a_good_one() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(FILE);
    let saved = |file: &std::path::Path| roundtrip(&fs::read_to_string(file).unwrap()).unwrap();
    assert_eq!(
        LayoutStore::open(dir.path()).layout(),
        &Layout::default_tree()
    );
    assert_eq!(saved(&file), Layout::default_tree().to_value());

    fs::write(&file, "{").unwrap();
    LayoutStore::open(dir.path());
    assert_eq!(saved(&file), Layout::default_tree().to_value());

    let mine = Layout::default_tree()
        .swap_root_children_by_kind("chat-main", "right-tabs")
        .unwrap();
    let text = serde_json::to_string(&mine.to_value()).unwrap();
    fs::write(&file, &text).unwrap();
    assert_eq!(LayoutStore::open(dir.path()).layout(), &mine);
    assert_eq!(fs::read_to_string(&file).unwrap(), text);
    assert!(!dir.path().join(format!("{FILE}.tmp")).exists());
}

#[test]
fn rejects_a_bad_tree_and_reports_a_failed_write() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = LayoutStore::open(dir.path());
    let swapped = store
        .layout()
        .swap_root_children_by_kind("chat-main", "right-tabs")
        .unwrap();
    assert_eq!(store.set(swapped.clone()), Ok(true));
    let file = dir.path().join(FILE);
    let pretty = serde_json::to_string_pretty(&swapped.to_value()).unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), pretty);

    let mut bad = swapped.clone();
    bad.sidebar.id = String::new();
    assert_eq!(store.set(bad), Err("sidebar id invalid".into()));
    assert_eq!(store.layout(), &swapped);
    assert_eq!(fs::read_to_string(&file).unwrap(), pretty);

    fs::remove_file(&file).unwrap();
    fs::create_dir(&file).unwrap();
    assert!(!store.reset());
    assert_eq!(store.layout(), &Layout::default_tree());
    assert!(!dir.path().join(format!("{FILE}.tmp")).exists());
}
