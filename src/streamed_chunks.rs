//! `MarkdownRendererBase`'s streamed chunks: `StreamingMarkdownChunk` per piece of a growing text.
use std::{cell::RefCell, rc::Rc};

use crate::{
    deep_link,
    markdown::hast::{self, Element, Hast, Prop, Urls},
    markdown_target::{self, Target},
    streaming_markdown::{chunks, repair},
    word_fade::{self, Fade},
};

#[cfg(test)]
mod tests;

/// A mounted chunk: what its memo compares, and the tree its last parse left.
struct Held {
    start: usize,
    content: String,
    whole: bool,
    fade: Option<Rc<RefCell<Fade>>>,
    tree: Vec<Hast>,
}

/// The chunks a streamed message has mounted, and their trees end to end.
#[derive(Default)]
pub struct Chunks {
    held: Vec<Held>,
    tree: Rc<Vec<Hast>>,
}

/// Whether the `a` renderer draws something in its children's place: a card, a chip, the player.
fn replaced(a: &Element, files: &[(String, String)]) -> bool {
    let text = |name| match a.prop(name) {
        Some(Prop::Text(value)) => Some(value.as_str()),
        _ => None,
    };
    let href = text("href");
    let chip = |href| ["session/", "project/"].map(|path| deep_link::strip(href, path));
    let deep =
        |href| deep_link::parse_card(href).is_some() || chip(href).iter().any(Option::is_some);
    let handed = text("data-cindy-raw-local-link-href").or(href);
    let audio = || matches!(markdown_target::link(handed, files), Target::Audio { .. });
    href.is_some_and(deep) || audio()
}

/// The fade keys whose refs attach, in their order: a span's, then its list item's.
fn mounted<'a>(nodes: &'a [Hast], files: &[(String, String)], out: &mut Vec<&'a str>) {
    for node in nodes {
        let Hast::Element(element) = node else {
            continue;
        };
        if element.tag == "a" && replaced(element, files) {
            continue;
        }
        let key = match element.prop("dataWfKey") {
            Some(Prop::Text(key)) => Some(key.as_str()),
            _ => None,
        };
        out.extend(key.filter(|_| element.tag == "span"));
        mounted(&element.children, files, out);
        out.extend(key.filter(|_| element.tag == "li"));
    }
}

impl Chunks {
    /// A render of the throttled `text` at `now`: its chunks' tree; `files` are the task's attached.
    pub fn render(
        &mut self,
        text: &str,
        fade: Option<&Rc<RefCell<Fade>>>,
        files: &[(String, String)],
        now: f64,
    ) -> Rc<Vec<Hast>> {
        // The repair comes with the fade: with it off the text is parsed as it stands.
        let repaired = match fade {
            Some(_) => repair(text),
            None => text.to_owned(),
        };
        let split = chunks::split(&repaired);
        let whole = split.len() == 1;
        let mut before = std::mem::take(&mut self.held);
        let mut changed = before.len() != split.len();
        for chunk in split {
            // Cindy keys a chunk by its UTF-16 offset.
            let start = repaired[..chunk.start].encode_utf16().count();
            let kept = before.iter().position(|held| {
                let same = match (&held.fade, fade) {
                    (Some(held), Some(fade)) => Rc::ptr_eq(held, fade),
                    (None, None) => true,
                    _ => false,
                };
                held.start == start && held.content == chunk.content && held.whole == whole && same
            });
            if let Some(kept) = kept {
                self.held.push(before.swap_remove(kept));
                continue;
            }
            changed = true;
            let mut tree = hast::render(chunk.content, true, Urls::Trusted);
            if let Some(fade) = fade {
                let mut owner = fade.borrow_mut();
                let mut candidate = match (owner.source(start).candidate(), whole) {
                    (_, true) => owner.whole_candidate(start),
                    (candidate, false) => candidate,
                };
                word_fade::fade(&mut tree, &mut candidate, now);
                let mut refs = Vec::new();
                mounted(&tree, files, &mut refs);
                let source = owner.source(start);
                for key in refs {
                    source.schedule(key, now);
                }
                source.commit(&candidate);
                if whole {
                    owner.retain_only(start);
                }
            }
            self.held.push(Held {
                start,
                content: chunk.content.to_owned(),
                whole,
                fade: fade.cloned(),
                tree,
            });
        }
        if changed {
            let all = self.held.iter().flat_map(|held| held.tree.iter().cloned());
            self.tree = Rc::new(all.collect());
        }
        self.tree.clone()
    }
}
