//! `remarkTruncateCjkUrls`: what a GFM literal autolink swallowed goes back to text.

use markdown::mdast::{Link, Node, Text};

use crate::{
    linkify,
    url_boundary::{self, Options, Strip},
};

/// Cuts every literal autolink of the tree.
pub fn transform(node: &mut Node) {
    let Some(children) = node.children_mut() else {
        return;
    };
    let mut index = 0;
    while index < children.len() {
        cut(children, index);
        transform(&mut children[index]);
        index += 1;
    }
}

/// `hasSamePosition`: the tokenizer's literal, whose text spans the whole link.
fn literal(link: &Link) -> bool {
    let [Node::Text(text)] = link.children.as_slice() else {
        return false;
    };
    let span = |node: Option<&markdown::unist::Position>| {
        node.map(|position| (position.start.offset, position.end.offset))
    };
    text.value == link.url
        && span(link.position.as_ref()).is_some()
        && span(link.position.as_ref()) == span(text.position.as_ref())
}

/// `textFromPhrasingContent`.
fn plain(node: &Node, out: &mut String) {
    match node {
        Node::Text(text) => out.push_str(&text.value),
        Node::InlineCode(code) => out.push_str(&code.value),
        Node::Image(image) => out.push_str(&image.alt),
        Node::ImageReference(image) => out.push_str(&image.alt),
        _ => {
            for child in node.children().into_iter().flatten() {
                plain(child, out);
            }
        }
    }
}

/// The visitor: cuts the literal autolink at `index`; its tail holds no literal.
fn cut(siblings: &mut Vec<Node>, index: usize) {
    let Node::Link(link) = &siblings[index] else {
        return;
    };
    if !literal(link) {
        return;
    }
    merge(siblings, index);
    let before = index.checked_sub(1).and_then(|at| match &siblings[at] {
        Node::Text(text) => Some(text.value.as_str()),
        _ => None,
    });
    let mut prefix = String::new();
    for node in &siblings[..index] {
        plain(node, &mut prefix);
    }
    let Node::Link(link) = &siblings[index] else {
        unreachable!("the merge keeps the link")
    };
    let options = Options {
        prefix: &prefix,
        wrap_marker: Some(before.and_then(url_boundary::wrap_marker)),
        strip_formatting: Strip::Auto,
        ..Options::default()
    };
    let at = url_boundary::clip(&link.url, &options);
    if at == link.url.len() || at == 0 {
        return;
    }
    let tail = tail(&link.url[at..]);
    set(&mut siblings[index], |url| url.truncate(at));
    siblings.splice(index + 1..index + 1, tail);
}

/// Writes the link's address and its one text alike.
fn set(node: &mut Node, change: impl Fn(&mut String)) {
    let Node::Link(link) = node else {
        unreachable!("a literal autolink")
    };
    change(&mut link.url);
    let [Node::Text(text)] = link.children.as_mut_slice() else {
        unreachable!("a literal autolink holds one text")
    };
    change(&mut text.value);
}

fn text(value: &str) -> Node {
    Node::Text(Text {
        value: value.to_owned(),
        position: None,
    })
}

/// `linkifyTailUrls`: the addresses the cut tail still holds.
fn tail(tail: &str) -> Vec<Node> {
    let mut nodes = Vec::new();
    let mut cursor = 0;
    for found in linkify::find(tail) {
        if found.link != linkify::Link::Url {
            continue;
        }
        if found.range.start > cursor {
            nodes.push(text(&tail[cursor..found.range.start]));
        }
        let url = &tail[found.range.clone()];
        nodes.push(Node::Link(Link {
            children: vec![text(url)],
            position: None,
            url: url.to_owned(),
            title: None,
        }));
        cursor = found.range.end;
    }
    if cursor < tail.len() {
        nodes.push(text(&tail[cursor..]));
    }
    nodes
}

/// `expectedQueryBracketCloser`: the closer the query's open brackets wait for.
fn closer(query: &str) -> Option<char> {
    let mut stack = Vec::new();
    for ch in query.chars() {
        match ch {
            '[' => stack.push(']'),
            '{' => stack.push('}'),
            ']' | '}' if stack.last() == Some(&ch) => {
                stack.pop();
            }
            _ => {}
        }
    }
    stack.last().copied()
}

/// `mergeBalancedQueryBracketTail`: closers the tokenizer left after a query go back in.
fn merge(siblings: &mut Vec<Node>, index: usize) {
    let Node::Link(link) = &siblings[index] else {
        unreachable!("a literal autolink")
    };
    let Some(Node::Text(next)) = siblings.get(index + 1) else {
        return;
    };
    let Some(query) = link.url.find(['?', '#']) else {
        return;
    };
    let mut taken = String::new();
    let mut rest = next.value.as_str();
    while let Some(ch) = closer(&format!("{}{taken}", &link.url[query..])) {
        let Some(after) = rest.strip_prefix(ch) else {
            break;
        };
        taken.push(ch);
        rest = after;
    }
    if taken.is_empty() {
        return;
    }
    let rest = rest.to_owned();
    set(&mut siblings[index], |url| url.push_str(&taken));
    if rest.is_empty() {
        siblings.remove(index + 1);
    } else if let Node::Text(next) = &mut siblings[index + 1] {
        next.value = rest;
    }
}
