//! `JSON.stringify(value, null, indent)`.

use super::{Js, Obj, RED_ZONE, STACK, quote};
use crate::cindy_json::js_number;

/// `JSON.stringify` with an indent string; empty writes the compact form.
pub fn stringify(value: &Js, indent: &str) -> String {
    let mut out = String::new();
    write(value, indent, 0, &mut out);
    out
}

/// Own properties in `Object.keys` order: array indices ascending, then insertion order.
pub fn entries(fields: &Obj) -> impl Iterator<Item = (&str, &Js)> {
    let index = |key: &str| {
        let n = key.parse::<u32>().ok()?;
        (n != u32::MAX && n.to_string() == key).then_some(n)
    };
    let mut indexed: Vec<(u32, &str, &Js)> = fields
        .iter()
        .filter_map(|(k, v)| index(k).map(|n| (n, k.as_str(), v)))
        .collect();
    indexed.sort_by_key(|(n, _, _)| *n);
    let named = fields.iter().filter(move |(k, _)| index(k).is_none());
    indexed
        .into_iter()
        .map(|(_, k, v)| (k, v))
        .chain(named.map(|(k, v)| (k.as_str(), v)))
}

fn write(value: &Js, indent: &str, depth: usize, out: &mut String) {
    stacker::maybe_grow(RED_ZONE, STACK, || match value {
        Js::Null => out.push_str("null"),
        Js::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Js::Num(n) if n.is_finite() => out.push_str(&js_number(*n)),
        Js::Num(_) => out.push_str("null"),
        Js::Str(s) => out.push_str(&quote(s)),
        Js::Arr(items) if items.is_empty() => out.push_str("[]"),
        Js::Arr(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                separate(i, indent, depth + 1, out);
                write(item, indent, depth + 1, out);
            }
            close(indent, depth, out);
            out.push(']');
        }
        Js::Obj(fields) if fields.is_empty() => out.push_str("{}"),
        Js::Obj(fields) => {
            out.push('{');
            for (i, (key, item)) in entries(fields).enumerate() {
                separate(i, indent, depth + 1, out);
                out.push_str(&quote(key));
                out.push_str(if indent.is_empty() { ":" } else { ": " });
                write(item, indent, depth + 1, out);
            }
            close(indent, depth, out);
            out.push('}');
        }
    })
}

fn separate(i: usize, indent: &str, depth: usize, out: &mut String) {
    if i > 0 {
        out.push(',');
    }
    if !indent.is_empty() {
        out.push('\n');
        out.push_str(&indent.repeat(depth));
    }
}

fn close(indent: &str, depth: usize, out: &mut String) {
    if !indent.is_empty() {
        out.push('\n');
        out.push_str(&indent.repeat(depth));
    }
}
