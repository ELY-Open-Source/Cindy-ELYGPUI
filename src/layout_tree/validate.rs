use std::collections::HashSet;

use serde_json::Value;

use super::{CHAT_MAIN, MAX_TREE_DEPTH, SESSION_LIST};
use crate::cindy_json::{js_number, js_string};

type Checked = Result<(), String>;

/// A content pane's kind and collapsed flag.
type Seen<'a> = Vec<(&'a str, Option<&'a Value>)>;

/// JS `.length`, in UTF-16 units.
fn bounded(value: Option<&Value>) -> Option<&str> {
    let s = value?.as_str()?;
    (1..=128).contains(&s.encode_utf16().count()).then_some(s)
}

fn finite(value: Option<&Value>) -> Option<f64> {
    value?.as_f64()
}

fn node<'a>(
    node: Option<&'a Value>,
    depth: usize,
    ids: &mut HashSet<&'a str>,
    panes: &mut Seen<'a>,
) -> Checked {
    if depth > MAX_TREE_DEPTH {
        return Err(format!("tree deeper than {MAX_TREE_DEPTH}"));
    }
    let Some(node) = node.and_then(Value::as_object) else {
        return Err("node is not an object".into());
    };
    let Some(id) = bounded(node.get("id")) else {
        return Err("node id invalid".into());
    };
    if !ids.insert(id) {
        return Err(format!("duplicate node id: {id}"));
    }
    match node.get("type").and_then(Value::as_str) {
        Some("pane") => {
            let Some(kind) = bounded(node.get("panelKind")) else {
                return Err("panelKind invalid".into());
            };
            let collapsed = node.get("collapsed");
            if collapsed.is_some_and(|c| !c.is_boolean()) {
                return Err("collapsed must be boolean".into());
            }
            if node.contains_key("minWidth")
                && !finite(node.get("minWidth")).is_some_and(|w| w > 0.0)
            {
                return Err("minWidth must be a positive finite number".into());
            }
            if node.contains_key("preferredWidth")
                && !finite(node.get("preferredWidth"))
                    .is_some_and(|w| (120.0..=16000.0).contains(&w))
            {
                return Err("preferredWidth must be between 120 and 16000".into());
            }
            panes.push((kind, collapsed));
            Ok(())
        }
        Some("split") => {
            let direction = node.get("direction").and_then(Value::as_str);
            if !matches!(direction, Some("row" | "column")) {
                return Err("split direction invalid".into());
            }
            let Some(children) = node
                .get("children")
                .and_then(Value::as_array)
                .filter(|c| c.len() >= 2)
            else {
                return Err("split needs at least 2 children".into());
            };
            let mut sum = 0.0;
            for child in children {
                let Some(child) = child.as_object() else {
                    return Err("split child is not an object".into());
                };
                let Some(fraction) = finite(child.get("fraction")).filter(|f| *f > 0.0) else {
                    return Err("child fraction must be a positive finite number".into());
                };
                sum += fraction;
                self::node(child.get("node"), depth + 1, ids, panes)?;
            }
            if (sum - 1.0).abs() > 0.01 {
                return Err(format!(
                    "split fractions must sum to ~1, got {}",
                    js_number(sum)
                ));
            }
            Ok(())
        }
        _ => Err(format!(
            "unknown node type: {}",
            js_string(node.get("type"))
        )),
    }
}

/// `validateLayout` over the tree as JSON.
pub fn layout(input: &Value) -> Checked {
    let Some(input) = input.as_object() else {
        return Err("layout is not an object".into());
    };
    if finite(input.get("schemaVersion")) != Some(1.0) {
        return Err("schemaVersion must be 1".into());
    }
    if !input
        .get("float")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty)
    {
        return Err("float must be an empty array in v1".into());
    }
    let sidebar = input.get("sidebar").and_then(Value::as_object);
    let Some(sidebar) = sidebar.filter(|s| s.get("type").and_then(Value::as_str) == Some("pane"))
    else {
        return Err("sidebar must be a pane".into());
    };
    if sidebar.get("panelKind").and_then(Value::as_str) != Some(SESSION_LIST) {
        return Err("sidebar must be session-list".into());
    }
    if sidebar.get("edge").and_then(Value::as_str) != Some("left") {
        return Err("sidebar edge must be 'left' in v1".into());
    }
    let Some(sidebar_id) = bounded(sidebar.get("id")) else {
        return Err("sidebar id invalid".into());
    };
    let mut ids = HashSet::from([sidebar_id]);
    let mut panes = Vec::new();
    node(input.get("content"), 1, &mut ids, &mut panes)?;
    let chats: Vec<_> = panes
        .iter()
        .filter(|(kind, _)| *kind == CHAT_MAIN)
        .collect();
    if chats.len() != 1 {
        return Err(format!(
            "content must contain exactly one chat-main (got {})",
            chats.len()
        ));
    }
    if chats[0].1 == Some(&Value::Bool(true)) {
        return Err("chat-main must not be collapsed".into());
    }
    if panes.iter().any(|(kind, _)| *kind == SESSION_LIST) {
        return Err("session-list must stay outside the content tree".into());
    }
    Ok(())
}
