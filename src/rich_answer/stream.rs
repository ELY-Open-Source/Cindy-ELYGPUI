//! `RichAnswerStream.update(source, true)` on a fresh stream: each line one operation, a
//! rejected line skipped as Cindy's `catch` skips it.

use std::{collections::HashMap, sync::LazyLock};

use indexmap::IndexMap;
use regex::Regex;

use super::js::{self, Js, Obj, stringify_len, truthy, utf16_len};

const MAX_SOURCE: usize = 2_000_000;
const MAX_NODES: usize = 2048;
pub const MAX_DEPTH: usize = 32;
const MAX_PROPS: usize = 65_536;

pub const KINDS: [&str; 26] = [
    "text", "markdown", "heading", "icon", "stack", "row", "grid", "group", "divider", "image",
    "video", "audio", "code", "math", "diagram", "stat", "chart", "svg", "range", "input", "check",
    "select", "button", "form", "details", "tabs",
];
const CONTAINERS: [&str; 7] = ["stack", "row", "grid", "group", "form", "details", "tabs"];
const TEXT_PROP_KINDS: [&str; 6] = ["text", "markdown", "heading", "code", "math", "diagram"];

static ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zA-Z0-9_-]{1,80}$").expect("valid pattern"));

#[derive(Clone, Debug)]
pub struct Node {
    pub kind: &'static str,
    pub props: Obj,
    pub children: Vec<String>,
}

/// `RichDocument`: nodes in the order a JavaScript `Map` keeps them.
#[derive(Default)]
pub struct Doc {
    pub nodes: IndexMap<String, Node>,
    pub roots: Vec<String>,
    implicit_text: HashMap<String, String>,
}

/// A rejected operation; Cindy counts it and goes on.
struct Rejected;

type Step = Result<(), Rejected>;

fn kind(value: Option<&Js>) -> Option<&'static str> {
    let name = value?.str()?;
    KINDS.into_iter().find(|k| *k == name)
}

fn container(kind: &str) -> bool {
    CONTAINERS.contains(&kind)
}

fn valid_id(value: Option<&Js>) -> Option<&str> {
    value?.str().filter(|id| ID.is_match(id))
}

/// `splice(index, 0, id)` with Cindy's index: a whole number at least 0, else the end.
fn insert(list: &mut Vec<String>, index: Option<&Js>, id: String) {
    let at = match index.and_then(Js::integer) {
        Some(n) if n < list.len() as f64 => n.max(0.0) as usize,
        _ => list.len(),
    };
    list.insert(at, id);
}

/// The text a node holds, as `richString(node, "text")`.
pub fn text_prop(props: &Obj) -> &str {
    props.get("text").and_then(Js::str).unwrap_or("")
}

/// `props`: an object without prototype keys, or a bare string for the kinds that show text.
fn props(value: Option<&Js>, kind: Option<&Js>) -> Result<Obj, Rejected> {
    let Some(value) = value else {
        return Ok(Obj::new());
    };
    if let Js::Str(text) = value {
        let text_kind = kind
            .and_then(Js::str)
            .is_some_and(|k| TEXT_PROP_KINDS.contains(&k));
        if truthy(kind) && !text_kind {
            return Err(Rejected);
        }
        let props = Obj::from([("text".to_owned(), Js::Str(text.clone()))]);
        if stringify_len(&Js::Obj(props.clone())) > MAX_PROPS {
            return Err(Rejected);
        }
        return Ok(props);
    }
    let Js::Obj(fields) = value else {
        return Err(Rejected);
    };
    if stringify_len(value) > MAX_PROPS {
        return Err(Rejected);
    }
    Ok(fields
        .iter()
        .filter(|(k, _)| !matches!(k.as_str(), "__proto__" | "constructor" | "prototype"))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect())
}

/// `op ?? fallback`.
fn nullish(value: Option<&Js>) -> Option<&Js> {
    value.filter(|v| **v != Js::Null)
}

impl Doc {
    pub fn replay(source: &str) -> Doc {
        let mut doc = Doc::default();
        if utf16_len(source) > MAX_SOURCE {
            log::debug!(target: "rich-answer", "a cindy-ui block over {MAX_SOURCE} characters shows nothing");
            return doc;
        }
        let mut seq = 0.0;
        let rejected = source
            .split('\n')
            .filter(|line| doc.line(line, &mut seq).is_err())
            .count();
        if rejected > 0 {
            log::debug!(target: "rich-answer", "{rejected} cindy-ui operations rejected");
        }
        doc
    }

    fn line(&mut self, line: &str, last_seq: &mut f64) -> Step {
        if crate::theme_import::color::js_trim(line).is_empty() {
            return Ok(());
        }
        let op = js::parse(line).map_err(|_| Rejected)?;
        let op = op.obj().ok_or(Rejected)?;
        let seq = match op.get("seq") {
            Some(Js::Num(n)) if n.fract() == 0.0 && n.abs() <= 9_007_199_254_740_991.0 => *n,
            _ => return Err(Rejected),
        };
        if seq < 1.0 {
            return Err(Rejected);
        }
        if seq <= *last_seq {
            return Ok(());
        }
        *last_seq = seq;
        if op.get("version").is_some_and(|v| *v != Js::Num(1.0)) {
            return Err(Rejected);
        }
        let id = valid_id(op.get("id")).ok_or(Rejected)?.to_owned();
        let mut props = props(op.get("props"), op.get("kind"))?;
        if let Some(style @ Js::Obj(_)) = op.get("style")
            && !props.contains_key("style")
        {
            props.insert("style".into(), Js::Obj(self::props(Some(style), None)?));
        }
        match op.get("op").and_then(Js::str) {
            Some("create") => self.create(op, id, props),
            Some("append") => self.append(op, &id),
            Some("patch") => {
                let node = self.nodes.get_mut(&id).ok_or(Rejected)?;
                node.props.extend(props);
                Ok(())
            }
            Some("remove") => self.remove(&id),
            Some("move") => self.relocate(op, &id),
            _ => Err(Rejected),
        }
    }

    fn create(&mut self, op: &Obj, id: String, props: Obj) -> Step {
        let kind = kind(op.get("kind")).filter(|_| self.nodes.len() < MAX_NODES);
        let (Some(kind), false) = (kind, self.nodes.contains_key(&id)) else {
            return Err(Rejected);
        };
        let named = op.get("parent");
        let parent = match named {
            None => None,
            Some(Js::Str(root)) if root == "root" => None,
            Some(name) => self
                .nodes
                .get(&js::string(Some(name)))
                .map(|_| js::string(Some(name))),
        };
        if truthy(named) && named.and_then(Js::str) != Some("root") && parent.is_none() {
            return Err(Rejected);
        }
        if let Some(parent) = &parent
            && (!container(self.nodes[parent].kind) || self.depth(parent) >= MAX_DEPTH)
        {
            return Err(Rejected);
        }
        let mut props = props;
        let inline = match props.get("children") {
            Some(Js::Arr(items)) if container(kind) => Some(items.clone()),
            _ => None,
        };
        if inline.is_some() {
            props.shift_remove("children");
        }
        self.nodes.insert(
            id.clone(),
            Node {
                kind,
                props,
                children: Vec::new(),
            },
        );
        let siblings = match &parent {
            Some(parent) => &mut self.nodes[parent].children,
            None => &mut self.roots,
        };
        insert(siblings, op.get("index"), id.clone());
        if let Some(inline) = inline {
            let depth = self.depth(&id) + 1;
            self.attach_inline(&id, &inline, depth);
        }
        Ok(())
    }

    /// `attachInlineChildren`: children written in `props` become nodes; a bad draft is skipped.
    fn attach_inline(&mut self, parent: &str, drafts: &[Js], depth: usize) {
        if !self.nodes.contains_key(parent) {
            return;
        }
        for (offset, draft) in drafts.iter().enumerate() {
            let Some(fields) = draft.obj() else { continue };
            let Some(kind) = kind(fields.get("kind")) else {
                continue;
            };
            if depth > MAX_DEPTH || self.nodes.len() >= MAX_NODES {
                continue;
            }
            let fallback = match fields.get("text") {
                Some(Js::Str(text)) => Obj::from([("text".to_owned(), Js::Str(text.clone()))]),
                _ => Obj::new(),
            };
            let fallback = Js::Obj(fallback);
            let given = nullish(fields.get("props")).unwrap_or(&fallback);
            let Ok(mut props) = props(Some(given), fields.get("kind")) else {
                continue;
            };
            let id = match valid_id(fields.get("id")) {
                Some(id) if !self.nodes.contains_key(id) => id.to_owned(),
                _ => self.inline_id(parent, offset + 1),
            };
            let nested = match (fields.get("children"), props.get("children")) {
                (Some(Js::Arr(items)), _) | (_, Some(Js::Arr(items))) => Some(items.clone()),
                _ => None,
            }
            .filter(|_| container(kind));
            if nested.is_some() {
                props.shift_remove("children");
            }
            self.nodes.insert(
                id.clone(),
                Node {
                    kind,
                    props,
                    children: Vec::new(),
                },
            );
            self.nodes[parent].children.push(id.clone());
            if let Some(nested) = nested {
                self.attach_inline(&id, &nested, depth + 1);
            }
        }
    }

    /// `inlineChildId`: ids stay within 80 characters, the suffix first.
    fn inline_id(&self, parent: &str, ordinal: usize) -> String {
        (1..)
            .map(|attempt| {
                let suffix = match attempt {
                    1 => format!("-{ordinal}"),
                    n => format!("-{ordinal}-{n}"),
                };
                let keep = parent.len().min(80 - suffix.len());
                format!("{}{suffix}", &parent[..keep])
            })
            .find(|id| !self.nodes.contains_key(id))
            .expect("some suffix is free")
    }

    fn append(&mut self, op: &Obj, id: &str) -> Step {
        let (Some(node), Some(Js::Str(added))) = (self.nodes.get(id), op.get("text")) else {
            return Err(Rejected);
        };
        if TEXT_PROP_KINDS.contains(&node.kind) {
            let text = format!("{}{added}", text_prop(&node.props));
            self.nodes[id].props.insert("text".into(), Js::Str(text));
            return Ok(());
        }
        if !container(node.kind) {
            return Err(Rejected);
        }
        let reused = self.implicit_text.get(id).filter(|child| {
            node.children.last() == Some(*child)
                && self.nodes.get(*child).is_some_and(|c| c.kind == "markdown")
        });
        if let Some(child) = reused.cloned() {
            let text = format!("{}{added}", text_prop(&self.nodes[&child].props));
            self.nodes[&child]
                .props
                .insert("text".into(), Js::Str(text));
            return Ok(());
        }
        if self.nodes.len() >= MAX_NODES || self.depth(id) >= MAX_DEPTH {
            return Err(Rejected);
        }
        let child = self.inline_id(id, 1);
        let given = Js::Obj(Obj::from([("text".to_owned(), Js::Str(added.clone()))]));
        let props = props(Some(&given), Some(&Js::Str("markdown".into())))?;
        self.nodes.insert(
            child.clone(),
            Node {
                kind: "markdown",
                props,
                children: Vec::new(),
            },
        );
        self.nodes[id].children.push(child.clone());
        self.implicit_text.insert(id.to_owned(), child);
        Ok(())
    }

    fn remove(&mut self, id: &str) -> Step {
        if !self.nodes.contains_key(id) {
            return Err(Rejected);
        }
        self.drop_subtree(id);
        let nodes = &self.nodes;
        self.implicit_text
            .retain(|_, child| nodes.contains_key(child));
        self.detach(id);
        Ok(())
    }

    fn drop_subtree(&mut self, id: &str) {
        let children = self
            .nodes
            .get(id)
            .map(|n| n.children.clone())
            .unwrap_or_default();
        for child in &children {
            self.drop_subtree(child);
        }
        self.nodes.shift_remove(id);
        self.implicit_text.remove(id);
    }

    /// Takes `id` out of the roots and out of every node's children.
    fn detach(&mut self, id: &str) {
        self.roots.retain(|r| r != id);
        for node in self.nodes.values_mut() {
            node.children.retain(|c| c != id);
        }
    }

    fn relocate(&mut self, op: &Obj, id: &str) -> Step {
        if !self.nodes.contains_key(id) {
            return Err(Rejected);
        }
        let named = op.get("parent");
        let parent = match named {
            Some(Js::Str(root)) if root == "root" => None,
            named => Some(js::string(named)),
        };
        if let Some(parent) = &parent {
            let node = self.nodes.get(parent).ok_or(Rejected)?;
            if !container(node.kind) || self.descendant(id, parent) {
                return Err(Rejected);
            }
        }
        let base = parent.as_deref().map_or(0, |p| self.depth(p));
        if base + self.height(id) > MAX_DEPTH {
            return Err(Rejected);
        }
        self.detach(id);
        let siblings = match &parent {
            Some(parent) => &mut self.nodes[parent].children,
            None => &mut self.roots,
        };
        insert(siblings, op.get("index"), id.to_owned());
        Ok(())
    }

    fn descendant(&self, root: &str, id: &str) -> bool {
        root == id
            || self
                .nodes
                .get(root)
                .is_some_and(|n| n.children.iter().any(|c| self.descendant(c, id)))
    }

    fn height(&self, id: &str) -> usize {
        1 + self.nodes.get(id).map_or(0, |n| {
            n.children.iter().map(|c| self.height(c)).max().unwrap_or(0)
        })
    }

    /// 1 at the top; the first node holding `id` is its parent.
    pub fn depth(&self, id: &str) -> usize {
        match self
            .nodes
            .iter()
            .find(|(_, n)| n.children.iter().any(|c| c == id))
        {
            Some((parent, _)) => 1 + self.depth(parent),
            None => 1,
        }
    }
}
