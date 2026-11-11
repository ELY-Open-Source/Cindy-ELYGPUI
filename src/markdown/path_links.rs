//! `remarkLocalPathLinks`: a bare file path in text becomes a link marked as one.

use markdown::mdast::{Link, Node, Text};

use crate::path_tokens;

/// Splits every text outside a link at its file paths.
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
        // A piece read again alone would lose the text before it, which a match is judged by.
        match split {
            Some(nodes) => {
                let count = nodes.len();
                children.splice(index..=index, nodes);
                index += count;
            }
            None => {
                transform(&mut children[index]);
                index += 1;
            }
        }
    }
}

fn text(value: &str) -> Node {
    Node::Text(Text {
        value: value.to_owned(),
        position: None,
    })
}

/// `splitTextNode`: the text's pieces, if any; every match passes `looksLikeFilePath`.
fn split(value: &str) -> Option<Vec<Node>> {
    let mut nodes = Vec::new();
    let mut cursor = 0;
    for found in path_tokens::find(value) {
        let path = &value[found.clone()];
        if found.start > cursor {
            nodes.push(text(&value[cursor..found.start]));
        }
        nodes.push(Node::Link(Link {
            children: vec![text(path)],
            position: None,
            url: path.to_owned(),
            title: None,
            bare_path: true,
            raw: None,
        }));
        cursor = found.end;
    }
    if nodes.is_empty() {
        return None;
    }
    if cursor < value.len() {
        nodes.push(text(&value[cursor..]));
    }
    Some(nodes)
}
