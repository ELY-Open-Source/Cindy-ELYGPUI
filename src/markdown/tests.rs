use markdown::mdast::{AlignKind, Node, ReferenceKind};
use serde_json::{Value, json};

use super::{cjk_urls, html_images, parse, session_links};

fn reference(kind: ReferenceKind) -> &'static str {
    match kind {
        ReferenceKind::Shortcut => "shortcut",
        ReferenceKind::Collapsed => "collapsed",
        ReferenceKind::Full => "full",
    }
}

/// A node as remark holds it, less its place in the source.
fn bare(node: &Node) -> Value {
    let mut out = match node {
        Node::Root(_) => json!({"type": "root"}),
        Node::Paragraph(_) => json!({"type": "paragraph"}),
        Node::Blockquote(_) => json!({"type": "blockquote"}),
        Node::Emphasis(_) => json!({"type": "emphasis"}),
        Node::Strong(_) => json!({"type": "strong"}),
        Node::Delete(_) => json!({"type": "delete"}),
        Node::TableRow(_) => json!({"type": "tableRow"}),
        Node::TableCell(_) => json!({"type": "tableCell"}),
        Node::ThematicBreak(_) => json!({"type": "thematicBreak"}),
        Node::Break(_) => json!({"type": "break"}),
        Node::Text(n) => json!({"type": "text", "value": n.value}),
        Node::InlineCode(n) => json!({"type": "inlineCode", "value": n.value}),
        Node::InlineMath(n) => json!({"type": "inlineMath", "value": n.value}),
        Node::Html(n) => json!({"type": "html", "value": n.value}),
        Node::Heading(n) => json!({"type": "heading", "depth": n.depth}),
        Node::Code(n) => json!({"type": "code", "lang": n.lang, "meta": n.meta, "value": n.value}),
        Node::Math(n) => json!({"type": "math", "meta": n.meta, "value": n.value}),
        Node::List(n) => {
            json!({"type": "list", "ordered": n.ordered, "start": n.start, "spread": n.spread})
        }
        Node::ListItem(n) => json!({"type": "listItem", "spread": n.spread, "checked": n.checked}),
        Node::Link(n) => json!({"type": "link", "title": n.title, "url": n.url}),
        Node::Image(n) => {
            let mut image = json!({"type": "image", "alt": n.alt, "title": n.title, "url": n.url});
            for (key, size) in [("width", &n.width), ("height", &n.height)] {
                if let Some(size) = size {
                    image[key] = json!(size);
                }
            }
            image
        }
        Node::Definition(n) => json!({
            "type": "definition", "identifier": n.identifier, "label": n.label,
            "title": n.title, "url": n.url,
        }),
        Node::LinkReference(n) => json!({
            "type": "linkReference", "identifier": n.identifier, "label": n.label,
            "referenceType": reference(n.reference_kind),
        }),
        Node::ImageReference(n) => json!({
            "type": "imageReference", "alt": n.alt, "identifier": n.identifier, "label": n.label,
            "referenceType": reference(n.reference_kind),
        }),
        Node::FootnoteReference(n) => {
            json!({"type": "footnoteReference", "identifier": n.identifier, "label": n.label})
        }
        Node::FootnoteDefinition(n) => {
            json!({"type": "footnoteDefinition", "identifier": n.identifier, "label": n.label})
        }
        Node::Table(n) => {
            let align: Vec<Value> = n
                .align
                .iter()
                .map(|kind| match kind {
                    AlignKind::Left => json!("left"),
                    AlignKind::Right => json!("right"),
                    AlignKind::Center => json!("center"),
                    AlignKind::None => Value::Null,
                })
                .collect();
            json!({"type": "table", "align": align})
        }
        other => panic!("a node this parse does not build: {other:?}"),
    };
    if let Some(children) = node.children() {
        out["children"] = children.iter().map(bare).collect();
    }
    out
}

/// Cindy's own parse (`REMARK_PLUGINS`' first three) of each case, by `sync-golden.mjs`.
#[test]
fn the_tree_matches_cindys() {
    let golden = include_str!("../../tests/fixtures/markdown-tree/expected.json");
    let cases: Vec<(String, Value)> = serde_json::from_str(golden).unwrap();
    assert!(cases.len() > 51000);
    let mut wrong = 0;
    for (text, tree) in &cases {
        let port = std::panic::catch_unwind(|| bare(&parse(text)));
        if port.as_ref().ok() != Some(tree) {
            wrong += 1;
            eprintln!("{text:?}\n  cindy {tree}\n  port  {port:?}");
        }
    }
    assert_eq!(wrong, 0, "of {} trees", cases.len());
}

/// Each transformer alone on Cindy's parse, as Cindy's own plugin leaves the tree.
#[test]
fn the_transformers_match_cindys() {
    let golden = include_str!("../../tests/fixtures/markdown-transforms/expected.json");
    let cases: Vec<(String, Value)> = serde_json::from_str(golden).unwrap();
    assert!(cases.len() > 5800);
    type Transform = fn(&mut Node);
    let transforms: [(&str, Transform); 3] = [
        ("truncate", cjk_urls::transform),
        ("images", html_images::transform),
        ("sessions", session_links::transform),
    ];
    let mut wrong = 0;
    for (text, trees) in &cases {
        for (key, transform) in transforms {
            let mut tree = parse(text);
            transform(&mut tree);
            let port = bare(&tree);
            if port != trees[key] {
                wrong += 1;
                eprintln!("{key} {text:?}\n  cindy {}\n  port  {port}", trees[key]);
            }
        }
    }
    assert_eq!(wrong, 0, "of {} cases", cases.len());
}

#[test]
fn an_html_image_keeps_its_place() {
    let mut tree = parse("a <img src=\"https://a.test/b.png\"> b");
    html_images::transform(&mut tree);
    let image = &tree.children().unwrap()[0].children().unwrap()[1];
    let place = image.position().unwrap();
    assert!(matches!(image, Node::Image(_)));
    assert_eq!((place.start.offset, place.end.offset), (2, 34));
}
