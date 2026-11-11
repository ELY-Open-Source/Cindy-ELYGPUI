//! `remarkBareSpaceLocalImages`: an image whose local destination holds spaces, read off the source.

use markdown::mdast::{Image, Node, Text};

use super::{REPLACEMENT, plain};
use crate::{bare_space_images, theme_import::color::js_space};

/// Rebuilds every text outside a link that holds such an image; `source` is what `parse` read.
pub fn transform(node: &mut Node, source: &str) {
    // The tree's offsets count each NUL as the U+FFFD it reads as.
    let nuls: Vec<usize> = (source.match_indices('\0').enumerate())
        .map(|(count, (at, _))| at + count * (REPLACEMENT.len_utf8() - 1))
        .collect();
    let original =
        |at: usize| at - nuls.partition_point(|nul| *nul < at) * (REPLACEMENT.len_utf8() - 1);
    walk(node, &|start, end| &source[original(start)..original(end)]);
}

fn walk<'a>(node: &mut Node, span: &impl Fn(usize, usize) -> &'a str) {
    let inside = matches!(node, Node::Link(_) | Node::LinkReference(_));
    let Some(children) = node.children_mut() else {
        return;
    };
    let mut index = 0;
    while index < children.len() {
        let rebuilt = match &children[index] {
            Node::Text(text) if !inside => (text.position.as_ref())
                .and_then(|at| rebuild(span(at.start.offset, at.end.offset), &text.value)),
            _ => None,
        };
        match rebuilt {
            Some(nodes) => {
                let count = nodes.len();
                children.splice(index..=index, nodes);
                index += count;
            }
            None => {
                walk(&mut children[index], span);
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

/// `decodeFragment`: a piece of source read as inline Markdown, its outer white space kept.
fn decode(fragment: &str) -> Vec<Node> {
    let core = fragment.trim_matches(js_space);
    let lead = &fragment[..fragment.len() - fragment.trim_start_matches(js_space).len()];
    let trail = &fragment[lead.len() + core.len()..];
    let mut inner = Vec::new();
    if !core.is_empty() {
        let mut blocks = plain(core)
            .children_mut()
            .map(std::mem::take)
            .unwrap_or_default();
        inner = match blocks.as_mut_slice() {
            [Node::Paragraph(paragraph)] => std::mem::take(&mut paragraph.children),
            _ => vec![text(core)],
        };
    }
    let edge = |edge: &str| (!edge.is_empty()).then(|| text(edge));
    edge(lead)
        .into_iter()
        .chain(inner)
        .chain(edge(trail))
        .collect()
}

/// The nodes a text becomes, when its source `span` holds an image.
fn rebuild(span: &str, value: &str) -> Option<Vec<Node>> {
    let images = bare_space_images::find(span);
    if images.is_empty() {
        return None;
    }
    let piece = |from: usize, to: usize| match span == value {
        true if from < to => vec![text(&span[from..to])],
        true => Vec::new(),
        false => decode(&span[from..to]),
    };
    let mut nodes = Vec::new();
    let mut cursor = 0;
    for image in images {
        nodes.extend(piece(cursor, image.range.start));
        nodes.push(Node::Image(Image {
            position: None,
            alt: image.alt,
            url: image.url,
            title: image.title,
            width: None,
            height: None,
            raw: None,
        }));
        cursor = image.range.end;
    }
    nodes.extend(piece(cursor, span.len()));
    Some(nodes)
}
