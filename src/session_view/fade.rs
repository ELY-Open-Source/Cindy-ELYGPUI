//! `StreamFadeSpan` and `StreamFadeListItem`: a streamed word and a list marker on their way in.

use std::collections::HashMap;

use gpui::Window;

use crate::{
    markdown::hast::{Element, Hast, Prop},
    motion::{FAST, motion_ease_out},
    streamed_chunks::BEGIN,
    word_fade::Fade,
};

#[cfg(test)]
mod tests;

/// How far each fading node has come, by its address; a node not held is whole.
#[derive(Default)]
pub struct Fading(HashMap<*const Element, f32>);

impl Fading {
    pub fn of(&self, node: *const Element) -> Option<f32> {
        self.0.get(&node).copied()
    }
}

/// When the node's animation starts: a word's on the timeline, a marker's as its item keeps it.
fn begin(element: &Element, key: Option<&str>, fade: &Fade) -> Option<f64> {
    match (element.tag, element.prop(BEGIN)) {
        ("span", _) if element.classed("stream-word") => fade.start(key?),
        ("li", Some(Prop::Text(begin))) => Some(begin.parse().expect("a marker's start")),
        _ => None,
    }
}

fn walk(nodes: &[Hast], fade: &Fade, now: f64, out: &mut Fading) {
    for node in nodes {
        let Hast::Element(element) = node else {
            continue;
        };
        let key = match element.prop("dataWfKey") {
            Some(Prop::Text(key)) => Some(key.as_str()),
            _ => None,
        };
        if let Some(begin) = begin(element, key, fade) {
            let part = (now - begin) / (FAST.as_secs_f64() * 1000.0);
            match (part >= 1.0, key.filter(|_| element.tag == "span")) {
                // A word's `animationend`; a marker's comes with the frame, before the render.
                (true, Some(key)) => fade.settle(key),
                (true, None) => {}
                (false, _) => {
                    out.0.insert(element, motion_ease_out(part.max(0.0) as f32));
                }
            }
        }
        walk(&element.children, fade, now, out);
    }
}

/// `stream-word-in` and `stream-marker-in` over `tree` at `now`; a frame follows while one runs.
pub fn read(tree: &[Hast], fade: &Fade, now: f64, window: &Window) -> Fading {
    let mut fading = Fading::default();
    walk(tree, fade, now, &mut fading);
    if !fading.0.is_empty() {
        window.request_animation_frame();
    }
    fading
}
