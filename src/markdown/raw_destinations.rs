//! `remarkPreserveRawLocalDestinations`: a local destination is kept as it was written.

use markdown::mdast::Node;

use crate::{local_path::drive, markdown_target};

/// `isLocalDestination`.
fn local(url: &str) -> bool {
    drive(url) || url.starts_with("file://") || !markdown_target::scheme(url)
}

/// Keeps the address of every image and link that names a local place.
pub fn transform(node: &mut Node) {
    match node {
        Node::Image(image) if local(&image.url) => image.raw = Some(image.url.clone()),
        Node::Link(link) if local(&link.url) => link.raw = Some(link.url.clone()),
        _ => {}
    }
    for child in node.children_mut().into_iter().flatten() {
        transform(child);
    }
}
