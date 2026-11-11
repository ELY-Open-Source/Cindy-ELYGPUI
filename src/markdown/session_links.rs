//! `remarkSessionLinks`: a bare Cindy deep link in text becomes a link node.

use std::sync::LazyLock;

use markdown::mdast::{Link, Node, Text};
use regex::Regex;

use crate::{
    deep_link::{self, PROJECT, SESSION},
    js_re,
};

/// Splits every text outside a link at its deep links.
pub fn transform(node: &mut Node) {
    let inside = matches!(node, Node::Link(_));
    let Some(children) = node.children_mut() else {
        return;
    };
    let mut index = 0;
    while index < children.len() {
        let split = match &children[index] {
            Node::Text(text) if !inside => split(&text.value),
            _ => None,
        };
        // The pieces of a split text hold no link left to find.
        if let Some(nodes) = split {
            children.splice(index..=index, nodes);
        }
        transform(&mut children[index]);
        index += 1;
    }
}

fn text(value: &str) -> Node {
    Node::Text(Text {
        value: value.to_owned(),
        position: None,
    })
}

/// `findSessionLinkMatches` and `splitTextNode`: the text's pieces, if it holds a link.
fn split(value: &str) -> Option<Vec<Node>> {
    static LINK: LazyLock<Regex> =
        LazyLock::new(|| js_re::regex(&format!("({SESSION})|({PROJECT})"), false));
    if !deep_link::contains(value) {
        return None;
    }
    let mut nodes = Vec::new();
    let mut cursor = 0;
    let mut from = 0;
    while let Some(found) = LINK.captures_at(value, from) {
        let whole = found.get(0).expect("a match");
        let session = found.get(1).is_some();
        from = whole.end();
        // The project source's lookahead; no link starts inside a refused match.
        if !session && !deep_link::project_ends(value, whole.end()) {
            continue;
        }
        let href = whole
            .as_str()
            .trim_end_matches(['.', ',', ';', ':', '!', '?']);
        let valid = if session {
            deep_link::parse_session(href).is_some()
        } else {
            deep_link::parse_project(href).is_some()
        };
        if !valid {
            continue;
        }
        if whole.start() > cursor {
            nodes.push(text(&value[cursor..whole.start()]));
        }
        nodes.push(Node::Link(Link {
            children: vec![text(href)],
            position: None,
            url: href.to_owned(),
            title: None,
            bare_path: false,
            raw: None,
        }));
        cursor = whole.start() + href.len();
    }
    if nodes.is_empty() {
        return None;
    }
    if cursor < value.len() {
        nodes.push(text(&value[cursor..]));
    }
    Some(nodes)
}
