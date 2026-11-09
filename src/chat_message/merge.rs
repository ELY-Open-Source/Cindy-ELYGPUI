//! The store's `mergeMessages`, its sort and auto-resume collapse, on rows shared as JS shares objects.

use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::LazyLock,
};

use indexmap::IndexMap;
use regex::Regex;

use crate::rich_answer::js::{self, Js};

pub mod handoff;
mod hydrate;

use hydrate::hydrate;

/// A field's value; `None` is a key set to `undefined`.
pub type Field = Option<Rc<Js>>;
pub type Fields = IndexMap<String, Field>;
/// A `ChatMessage` object: clones of the `Rc` are the same object.
pub type Message = Rc<Fields>;
pub type Messages = Rc<Vec<Message>>;

/// `message[key]`; `None` is `undefined`.
pub fn get<'a>(m: &'a Fields, key: &str) -> Option<&'a Js> {
    m.get(key)?.as_deref()
}

fn field(m: &Fields, key: &str) -> Field {
    m.get(key).cloned().flatten()
}

fn is_str(m: &Fields, key: &str, want: &str) -> bool {
    get(m, key).and_then(Js::str) == Some(want)
}

fn is_true(m: &Fields, key: &str) -> bool {
    matches!(get(m, key), Some(Js::Bool(true)))
}

/// `Object.is`, or `===` when `strict` (NaN apart, zeros equal).
fn same(a: Option<&Rc<Js>>, b: Option<&Rc<Js>>, strict: bool) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) if Rc::ptr_eq(a, b) => {
            !strict || !matches!(**a, Js::Num(n) if n.is_nan())
        }
        (Some(a), Some(b)) => match (&**a, &**b) {
            (Js::Null, Js::Null) => true,
            (Js::Bool(x), Js::Bool(y)) => x == y,
            (Js::Str(x), Js::Str(y)) => x == y,
            (Js::Num(x), Js::Num(y)) if strict => x == y,
            (Js::Num(x), Js::Num(y)) => x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan()),
            _ => false,
        },
        _ => false,
    }
}

fn strict_field(a: &Fields, b: &Fields, key: &str) -> bool {
    same(
        a.get(key).and_then(Option::as_ref),
        b.get(key).and_then(Option::as_ref),
        true,
    )
}

/// A `Map` key under SameValueZero.
#[derive(PartialEq, Eq, Hash)]
pub enum Key {
    Undefined,
    Null,
    Bool(bool),
    Num(u64),
    Str(String),
    Object(*const Js),
}

pub fn client_id(m: &Fields) -> Key {
    let Some(value) = m.get("clientId").and_then(Option::as_ref) else {
        return Key::Undefined;
    };
    match &**value {
        Js::Null => Key::Null,
        Js::Bool(b) => Key::Bool(*b),
        Js::Num(n) if n.is_nan() => Key::Num(f64::NAN.to_bits()),
        Js::Num(n) => Key::Num((n + 0.0).to_bits()),
        Js::Str(s) => Key::Str(s.clone()),
        Js::Arr(_) | Js::Obj(_) => Key::Object(Rc::as_ptr(value)),
    }
}

#[derive(Default)]
pub struct Options {
    pub preserve_tool_result_content: bool,
    pub preserve_codex_plan_content: bool,
    pub add_only: bool,
    pub add_only_except: Option<HashSet<String>>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum RowsOrder {
    OldestFirst,
    NewestFirst,
}

/// `mergeMessages`: `existing` itself when nothing changed.
pub fn merge(
    server: &[Message],
    existing: &Messages,
    options: &Options,
    order: RowsOrder,
) -> Messages {
    let server_order: HashMap<Key, usize> = server
        .iter()
        .enumerate()
        .map(|(i, m)| (client_id(m), i))
        .collect();
    let mut by_id: IndexMap<Key, Message> = IndexMap::new();
    for m in server {
        by_id.insert(client_id(m), m.clone());
    }
    if existing.is_empty() {
        return Rc::new(collapse(sort(
            by_id.into_values().collect(),
            &server_order,
            order,
        )));
    }
    let mut existing_by_id: IndexMap<Key, Message> = IndexMap::new();
    for m in existing.iter() {
        let next = match existing_by_id.get(&client_id(m)) {
            Some(previous) => hydrate(previous, m, options),
            None => m.clone(),
        };
        existing_by_id.insert(client_id(m), next);
    }
    let mut rows: Vec<Message> = existing_by_id
        .iter()
        .map(|(key, m)| {
            let Some(persisted) = by_id.get(key) else {
                return m.clone();
            };
            let except = get(m, "clientId").and_then(Js::str).is_some_and(|id| {
                options
                    .add_only_except
                    .as_ref()
                    .is_some_and(|s| s.contains(id))
            });
            if options.add_only && !except {
                return m.clone();
            }
            hydrate(m, persisted, options)
        })
        .collect();
    let added = by_id
        .iter()
        .filter(|(key, _)| !existing_by_id.contains_key(*key));
    rows.extend(added.map(|(_, m)| m.clone()));
    let sorted = sort(rows, &server_order, order);
    // Each change Cindy flags (a fold, a hydrate, an addition) also breaks this check.
    let same_order = sorted.len() == existing.len()
        && sorted
            .iter()
            .zip(existing.iter())
            .all(|(a, b)| Rc::ptr_eq(a, b));
    match same_order {
        true => existing.clone(),
        false => Rc::new(collapse(sorted)),
    }
}

/// `messageTime`: an unreadable time sorts last.
fn message_time(m: &Fields) -> f64 {
    match get(m, "createdAt").filter(|v| js::truthy(Some(v))) {
        None => f64::INFINITY,
        v => Some(super::time(v))
            .filter(|t| t.is_finite())
            .unwrap_or(f64::INFINITY),
    }
}

fn message_rowid(m: &Fields) -> f64 {
    match get(m, "rowid") {
        Some(Js::Num(n)) if n.is_finite() => *n,
        _ => f64::NAN,
    }
}

/// `sortMessagesChronologically`, through V8's sort since its order is not total.
fn sort(
    messages: Vec<Message>,
    server_order: &HashMap<Key, usize>,
    order: RowsOrder,
) -> Vec<Message> {
    let mut items: Vec<usize> = (0..messages.len()).collect();
    crate::js_sort::sort(&mut items, |a, b| {
        let (x, y) = (&messages[a], &messages[b]);
        for diff in [
            message_time(x) - message_time(y),
            message_rowid(x) - message_rowid(y),
        ] {
            if !diff.is_nan() && diff != 0.0 {
                return diff < 0.0;
            }
        }
        if let (Some(&p), Some(&q)) = (
            server_order.get(&client_id(x)),
            server_order.get(&client_id(y)),
        ) && p != q
        {
            return (order == RowsOrder::OldestFirst) == (p < q);
        }
        a < b
    });
    items.into_iter().map(|i| messages[i].clone()).collect()
}

fn auto_resume(m: &Fields) -> Option<bool> {
    let card = get(m, "systemCardType").filter(|v| js::truthy(Some(v)))?;
    Some(matches!(
        card.str(),
        Some("auto-resume" | "auto-resume-pending")
    ))
}

/// `hasUserVisibleText`: something left past whitespace, format and control characters.
fn has_user_visible_text(text: &str) -> bool {
    static VISIBLE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"[^\s\p{Cf}\p{Cc}]").expect("a valid pattern"));
    VISIBLE.is_match(text)
}

/// `isSubstantiveChatRow`: other system cards and visible content end an interruption.
fn substantive(m: &Fields) -> bool {
    if auto_resume(m) == Some(false) {
        return true;
    }
    if is_true(m, "isSyntheticTrigger") || is_str(m, "role", "thinking") {
        return false;
    }
    match get(m, "content") {
        Some(Js::Str(text)) => has_user_visible_text(text),
        content => !matches!(content, None | Some(Js::Null)),
    }
}

/// `collapseConsecutiveAutoResumeRows`: one interruption keeps only its last card.
fn collapse(mut messages: Vec<Message>) -> Vec<Message> {
    let mut saw_card = false;
    for m in messages.iter_mut().rev() {
        if auto_resume(m) == Some(true) {
            if saw_card {
                let mut stripped = (**m).clone();
                stripped.shift_remove("systemCardType");
                stripped.shift_remove("systemCardData");
                *m = Rc::new(stripped);
            }
            saw_card = true;
            continue;
        }
        if saw_card && substantive(m) {
            saw_card = false;
        }
    }
    messages
}

#[cfg(test)]
mod tests;
