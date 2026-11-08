//! `richDefaults`, `richResolvedValues` and `richEvaluate`: whitelisted arithmetic over the
//! document's inputs and computed nodes.

use std::collections::{HashMap, HashSet};

use super::{
    js::{self, Js, utf16_len},
    stream::{Doc, MAX_DEPTH, Node},
};
use crate::{
    cindy_json::{js_number, js_to_number},
    theme_import::color::js_trim,
};

const MAX_CONCAT: usize = 65_536;

/// `RichValue`.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Str(String),
    Num(f64),
    Bool(bool),
}

impl Value {
    pub fn truthy(&self) -> bool {
        match self {
            Value::Str(s) => !s.is_empty(),
            Value::Num(n) => *n != 0.0,
            Value::Bool(b) => *b,
        }
    }

    fn number(&self) -> f64 {
        match self {
            Value::Str(s) => js_to_number(s),
            Value::Num(n) => *n,
            Value::Bool(b) => f64::from(u8::from(*b)),
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Str(s) => f.write_str(s),
            Value::Num(n) => f.write_str(&js_number(*n)),
            Value::Bool(b) => write!(f, "{b}"),
        }
    }
}

/// `value(v)`: a string, a boolean or a finite number.
fn scalar(expr: Option<&Js>) -> Option<Value> {
    match expr? {
        Js::Str(s) => Some(Value::Str(s.clone())),
        Js::Bool(b) => Some(Value::Bool(*b)),
        Js::Num(n) if n.is_finite() => Some(Value::Num(*n)),
        _ => None,
    }
}

pub fn string<'a>(node: &'a Node, key: &str, fallback: &'a str) -> &'a str {
    node.props.get(key).and_then(Js::str).unwrap_or(fallback)
}

fn number(node: &Node, key: &str, fallback: f64) -> f64 {
    match node.props.get(key) {
        Some(Js::Num(n)) if n.is_finite() => *n,
        _ => fallback,
    }
}

/// `node.props.value ?? node.props.text`.
fn value_or_text(node: &Node) -> Option<&Js> {
    match node.props.get("value") {
        None | Some(Js::Null) => node.props.get("text"),
        value => value,
    }
}

/// The resolved values; a key present with none is a value that stays unavailable.
pub struct Values<'a> {
    doc: &'a Doc,
    own: HashMap<String, Option<Value>>,
    visiting: HashSet<String>,
    /// Off once resolved: `richLabel` and `richChart` evaluate without resolving.
    resolving: bool,
}

impl<'a> Values<'a> {
    /// `richResolvedValues(document)` with nothing edited.
    pub fn resolve(doc: &'a Doc) -> Self {
        let mut values = Values {
            doc,
            own: defaults(doc),
            visiting: HashSet::new(),
            resolving: true,
        };
        for id in doc.nodes.keys() {
            values.lookup(id);
        }
        values.resolving = false;
        values
    }

    pub fn get(&self, key: &str) -> Option<&Option<Value>> {
        self.own.get(key)
    }

    fn lookup(&mut self, id: &str) -> Option<Value> {
        if let Some(value) = self.own.get(id) {
            return value.clone();
        }
        if self.visiting.contains(id) || self.visiting.len() >= MAX_DEPTH {
            return None;
        }
        let doc = self.doc;
        let node = doc.nodes.get(id)?;
        if !matches!(node.kind, "stat" | "text" | "heading") {
            return None;
        }
        self.visiting.insert(id.to_owned());
        let result = self.evaluate(value_or_text(node), 0);
        self.visiting.remove(id);
        self.own.insert(id.to_owned(), result.clone());
        result
    }

    /// `richEvaluate`.
    pub fn evaluate(&mut self, expr: Option<&Js>, depth: usize) -> Option<Value> {
        if let Some(value) = scalar(expr) {
            return Some(value);
        }
        let expr = expr?.obj().filter(|_| depth <= 16)?;
        if let Some(Js::Str(id)) = expr.get("ref") {
            return match self.own.get(id) {
                Some(value) => value.clone(),
                None if self.resolving => self.lookup(id),
                None => None,
            };
        }
        let args = expr.get("args")?.arr().filter(|a| a.len() <= 32)?;
        let op = expr.get("op");
        let named = op.and_then(Js::str);
        if named == Some("if") {
            let [condition, yes, no] = args else {
                return None;
            };
            let condition = self.evaluate(Some(condition), depth + 1)?;
            let branch = if condition.truthy() { yes } else { no };
            return self.evaluate(Some(branch), depth + 1);
        }
        let args: Vec<Option<Value>> = args
            .iter()
            .map(|a| self.evaluate(Some(a), depth + 1))
            .collect();
        let args: Vec<Value> = args.into_iter().collect::<Option<_>>()?;
        if named == Some("concat") {
            let size: usize = args.iter().map(|a| utf16_len(&a.to_string())).sum();
            return (size <= MAX_CONCAT)
                .then(|| Value::Str(args.iter().map(Value::to_string).collect()));
        }
        let binary = matches!(js::string(op).as_str(), "eq" | "lt" | "gt" | "sub" | "div");
        if binary && args.len() != 2 {
            return None;
        }
        if named == Some("eq") {
            return Some(Value::Bool(args[0] == args[1]));
        }
        let blank = |a: &Value| matches!(a, Value::Str(s) if js_trim(s).is_empty());
        if args.is_empty() || args.iter().any(blank) {
            return None;
        }
        let nums: Vec<f64> = args.iter().map(Value::number).collect();
        if !nums.iter().all(|n| n.is_finite()) {
            return None;
        }
        let result = match named? {
            "add" => nums.iter().fold(0.0, |a, b| a + b),
            "mul" => nums.iter().fold(1.0, |a, b| a * b),
            "sub" => nums[0] - nums[1],
            "div" => nums[0] / nums[1],
            "min" => nums.iter().copied().fold(f64::INFINITY, js_min),
            "max" => nums.iter().copied().fold(f64::NEG_INFINITY, js_max),
            "lt" => return Some(Value::Bool(nums[0] < nums[1])),
            "gt" => return Some(Value::Bool(nums[0] > nums[1])),
            _ => return None,
        };
        result.is_finite().then_some(Value::Num(result))
    }

    /// `richValue`: `value ?? text ?? ""`.
    pub fn value(&mut self, node: &Node) -> Option<Value> {
        match value_or_text(node) {
            None | Some(Js::Null) => Some(Value::Str(String::new())),
            expr => self.evaluate(expr, 0),
        }
    }

    /// `richLabel`: prefix, value and suffix, or a dash while unavailable.
    pub fn label(&mut self, node: &Node) -> String {
        match self.value(node) {
            None => "—".into(),
            Some(value) => {
                format!(
                    "{}{value}{}",
                    string(node, "prefix", ""),
                    string(node, "suffix", "")
                )
            }
        }
    }
}

/// `Math.min`: -0 is below 0.
fn js_min(a: f64, b: f64) -> f64 {
    if b < a || (b == 0.0 && a == 0.0 && b.is_sign_negative()) {
        b
    } else {
        a
    }
}

fn js_max(a: f64, b: f64) -> f64 {
    if b > a || (b == 0.0 && a == 0.0 && b.is_sign_positive()) {
        b
    } else {
        a
    }
}

/// `richDefaults`: each input's default, a later bind winning.
fn defaults(doc: &Doc) -> HashMap<String, Option<Value>> {
    doc.nodes
        .iter()
        .filter(|(_, n)| {
            matches!(
                n.kind,
                "input" | "range" | "select" | "check" | "tabs" | "details"
            )
        })
        .map(|(id, n)| {
            let value = scalar(n.props.get("default")).unwrap_or(match n.kind {
                "check" | "details" => Value::Bool(false),
                "range" => Value::Num(number(n, "min", 0.0)),
                _ => Value::Str(String::new()),
            });
            (string(n, "bind", id).to_owned(), Some(value))
        })
        .collect()
}
