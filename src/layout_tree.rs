//! Cindy's `shared/layoutTree.ts`: the main window's layout tree, its checks and operations.

mod ops;
mod store;
#[cfg(test)]
mod tests;
mod validate;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::cindy_json::js_value;
pub use store::LayoutStore;

pub const MIN_SPLIT_CHILD_FRACTION: f64 = 0.05;
const FRACTION_TOLERANCE: f64 = 1e-9;
const MAX_TREE_DEPTH: usize = 8;
const CHAT_MAIN: &str = "chat-main";
const SESSION_LIST: &str = "session-list";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    schema_version: u8,
    pub sidebar: Sidebar,
    pub content: Node,
    float: Vec<Value>,
    #[serde(flatten)]
    extra: Map<String, Value>,
}

/// The session list column. Only its id is checked; other fields stay as written.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sidebar {
    pub id: String,
    #[serde(flatten)]
    rest: Map<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Node {
    Pane(Pane),
    Split(Split),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pane {
    pub id: String,
    pub panel_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collapsed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_width: Option<f64>,
    #[serde(flatten)]
    extra: Map<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Split {
    pub id: String,
    pub direction: Direction,
    pub children: Vec<Child>,
    #[serde(flatten)]
    extra: Map<String, Value>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Row,
    Column,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Child {
    pub fraction: f64,
    pub node: Node,
    #[serde(flatten)]
    extra: Map<String, Value>,
}

impl Pane {
    fn new(id: &str, panel_kind: &str) -> Self {
        Pane {
            id: id.into(),
            panel_kind: panel_kind.into(),
            collapsed: None,
            min_width: None,
            preferred_width: None,
            extra: Map::new(),
        }
    }
}

impl Child {
    fn new(fraction: f64, node: Node) -> Self {
        Child {
            fraction,
            node,
            extra: Map::new(),
        }
    }
}

impl Layout {
    /// `createDefaultLayout`.
    pub fn default_tree() -> Self {
        let rest = json!({ "type": "pane", "panelKind": SESSION_LIST, "edge": "left" });
        let Value::Object(rest) = rest else {
            unreachable!()
        };
        let chat = Pane {
            min_width: Some(400.0),
            ..Pane::new("chat", CHAT_MAIN)
        };
        Layout {
            schema_version: 1,
            sidebar: Sidebar {
                id: "sessions".into(),
                rest,
            },
            content: Node::Split(Split {
                id: "root".into(),
                direction: Direction::Row,
                children: vec![
                    Child::new(0.5, Node::Pane(chat)),
                    Child::new(0.5, Node::Pane(Pane::new("right", "right-tabs"))),
                ],
                extra: Map::new(),
            }),
            float: Vec::new(),
            extra: Map::new(),
        }
    }

    /// `coerceLayout`: the tree, or the default and why it fell back. None is unreadable.
    pub fn coerce(raw: Option<&Value>) -> (Self, Option<String>) {
        let Some(raw) = raw.filter(|v| v.is_object()) else {
            return (Self::default_tree(), Some("not an object".into()));
        };
        match validate::layout(raw) {
            Ok(()) => (Self::from_valid(raw), None),
            Err(reason) => (Self::default_tree(), Some(reason)),
        }
    }

    fn from_valid(raw: &Value) -> Self {
        serde_json::from_value(raw.clone()).expect("a valid layout reads as one")
    }

    /// The tree as `JSON.stringify` writes it.
    pub fn to_value(&self) -> Value {
        let mut value = serde_json::to_value(self).expect("a layout serializes");
        js_numbers(&mut value);
        value
    }

    /// `validateLayout`.
    pub fn validate(&self) -> Result<(), String> {
        validate::layout(&self.to_value())
    }
}

impl Node {
    pub fn id(&self) -> &str {
        match self {
            Node::Pane(pane) => &pane.id,
            Node::Split(split) => &split.id,
        }
    }

    pub fn contains_kind(&self, kind: &str) -> bool {
        match self {
            Node::Pane(pane) => pane.panel_kind == kind,
            Node::Split(split) => split.children.iter().any(|c| c.node.contains_kind(kind)),
        }
    }

    pub fn is_pane_of(&self, kind: &str) -> bool {
        matches!(self, Node::Pane(pane) if pane.panel_kind == kind)
    }
}

fn js_numbers(value: &mut Value) {
    match value {
        Value::Number(n) => *value = js_value(n.as_f64().expect("JSON numbers are f64")),
        Value::Array(items) => items.iter_mut().for_each(js_numbers),
        Value::Object(map) => map.values_mut().for_each(js_numbers),
        _ => {}
    }
}
