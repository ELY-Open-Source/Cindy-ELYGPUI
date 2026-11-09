use serde_json::{Value, json};

use super::{Change, Kind, Parsed, describe, humanize, parse_name, truncate};
use crate::rich_answer::js;

fn golden() -> Value {
    serde_json::from_str(include_str!("../../tests/fixtures/tool-use/expected.json")).unwrap()
}

/// Sets `key` when there is a value, as the descriptor's spreads add it.
fn put(out: &mut Value, key: &str, value: Option<Value>) {
    if let Some(value) = value {
        out[key] = value;
    }
}

fn intent(i: &crate::command_intent::Intent) -> Value {
    let mut out = json!({ "action": i.action });
    put(&mut out, "target", i.target.as_ref().map(|t| json!(t)));
    put(&mut out, "path", i.path.as_ref().map(|p| json!(p)));
    out
}

fn change(c: &Change) -> Value {
    let mut out = json!({ "action": c.action, "fileName": c.file_name });
    put(
        &mut out,
        "moveFileName",
        c.move_file_name.as_ref().map(|m| json!(m)),
    );
    out
}

/// The descriptor as the golden's projection writes it.
fn shown(name: &str, input: &Value) -> Value {
    let raw = (!input.is_null()).then(|| js::parse(&input.to_string()).unwrap());
    let d = describe(name, raw.as_ref());
    let mut out = json!({ "toolName": d.tool_name });
    let opt = |v: &Option<String>| v.as_ref().map(|s| json!(s));
    let kind = match &d.kind {
        Kind::Command {
            description,
            command,
            intent: i,
        } => {
            put(&mut out, "description", opt(description));
            out["command"] = json!(command);
            put(&mut out, "intent", i.as_ref().map(intent));
            "command"
        }
        Kind::File { action, file_name } => {
            (out["action"], out["fileName"]) = (json!(action), json!(file_name));
            "file"
        }
        Kind::FileChange(changes) => {
            out["changes"] = changes.iter().map(change).collect();
            "fileChange"
        }
        Kind::Search { pattern } => {
            out["pattern"] = json!(pattern);
            "search"
        }
        Kind::Web { fetch, target } => {
            out["mode"] = json!(if *fetch { "fetch" } else { "search" });
            out["target"] = json!(target);
            "web"
        }
        Kind::Todo => "todo",
        Kind::Task { description } => {
            put(&mut out, "description", opt(description));
            "task"
        }
        Kind::Mcp {
            server,
            tool_label,
            created_path,
        } => {
            (out["serverLabel"], out["toolLabel"]) = (json!(server), json!(tool_label));
            put(&mut out, "createdPath", opt(created_path));
            "mcp"
        }
        Kind::Dynamic {
            namespace,
            tool_label,
        } => {
            put(&mut out, "namespace", opt(namespace));
            out["toolLabel"] = json!(tool_label);
            "dynamic"
        }
        Kind::Collab { tool_label } => {
            out["toolLabel"] = json!(tool_label);
            "collab"
        }
        Kind::Generic => "generic",
    };
    out["kind"] = json!(kind);
    out
}

#[test]
fn descriptors_match_cindys() {
    let golden = golden();
    for case in golden["describe"].as_array().unwrap() {
        let name = case[0].as_str().unwrap();
        assert_eq!(shown(name, &case[1]), case[2], "{name} {}", case[1]);
    }
}

#[test]
fn names_split_as_cindys_do() {
    for case in golden()["parse"].as_array().unwrap() {
        let name = case[0].as_str().unwrap();
        let got = match parse_name(name) {
            Parsed::Plain => json!({ "kind": "plain", "name": name }),
            Parsed::Mcp { server, tool } => {
                json!({ "kind": "mcp", "server": server, "tool": tool })
            }
            Parsed::Dynamic { namespace, tool } => {
                let mut out = json!({ "kind": "dynamic", "tool": tool });
                put(&mut out, "namespace", namespace.map(|n| json!(n)));
                out
            }
            Parsed::Collab { tool } => json!({ "kind": "collab", "tool": tool }),
        };
        assert_eq!(got, case[1], "{name}");
    }
}

#[test]
fn text_trims_and_cuts_as_cindys_does() {
    let golden = golden();
    for case in golden["truncate"].as_array().unwrap() {
        let (text, max) = (
            case[0].as_str().unwrap(),
            case[1].as_u64().unwrap() as usize,
        );
        assert_eq!(
            truncate(text, max),
            case[2].as_str().unwrap(),
            "{text:?} {max}"
        );
    }
    for case in golden["humanize"].as_array().unwrap() {
        let token = case[0].as_str().unwrap();
        assert_eq!(humanize(token), case[1].as_str().unwrap(), "{token:?}");
    }
}
