use serde_json::{Value, json};

use super::{Intent, from_actions, from_command, intents_from_actions};
use crate::rich_answer::js;

fn golden() -> Value {
    serde_json::from_str(include_str!(
        "../../tests/fixtures/command-intent/expected.json"
    ))
    .unwrap()
}

/// The intent as `JSON.stringify` writes it, undefined fields left out.
fn shown(intent: Option<&Intent>) -> Value {
    let Some(intent) = intent else {
        return Value::Null;
    };
    let mut out = json!({ "action": intent.action });
    if let Some(target) = &intent.target {
        out["target"] = json!(target);
    }
    if let Some(path) = &intent.path {
        out["path"] = json!(path);
    }
    out
}

fn as_js(value: &Value) -> js::Js {
    js::parse(&value.to_string()).unwrap()
}

#[test]
fn commands_match_cindys_classifier() {
    let golden = golden();
    let mut wrong = Vec::new();
    for case in golden["command"].as_array().unwrap() {
        let command = case[0].as_str().unwrap();
        let got = shown(from_command(command).as_ref());
        if got != case[1] {
            wrong.push(format!("{command:?}: {got} != {}", case[1]));
        }
    }
    assert!(
        wrong.is_empty(),
        "{} wrong:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}

#[test]
fn codex_actions_match_cindys_classifier() {
    let golden = golden();
    for case in golden["actions"].as_array().unwrap() {
        let raw = (!case[0].is_null()).then(|| as_js(&case[0]));
        let got = shown(from_actions(raw.as_ref(), None).as_ref());
        assert_eq!(got, case[1], "{}", case[0]);
    }
    for case in golden["intents"].as_array().unwrap() {
        let [actions, full] = [&case[0][0], &case[0][1]];
        let got: Vec<Value> = intents_from_actions(Some(&as_js(actions)), full.as_str())
            .iter()
            .map(|i| shown(Some(i)))
            .collect();
        assert_eq!(Value::from(got), case[1], "{}", case[0]);
    }
}
