//! `StreamFadeListItem`'s `::marker` animations, as React and Blink keep them per list item.

use std::collections::HashMap;

use crate::{
    markdown::hast::{Element, Hast, Prop},
    motion,
    word_fade::Fade,
};

/// The property a running marker's animation start is kept in, as the DOM keeps `--wf-delay`.
pub const BEGIN: &str = "wfBegin";

/// A mounted list item: what React last rendered on it, and what its DOM holds.
#[derive(Debug)]
pub struct Marker {
    key: Option<String>,
    /// React's `data-stream-marker`; `animationend` takes the attribute off behind React's back.
    prop: bool,
    shown: bool,
    /// Whether the item's list draws markers: Blink animates no `::marker` it does not make.
    boxed: bool,
    /// `--wf-delay` on the item.
    delay: f64,
    /// When the animation was made.
    since: f64,
    ended: bool,
}

/// The attribute, the animation's state and its delay, as the golden reads them.
#[cfg(test)]
pub type State = (bool, &'static str, Option<f64>);

/// A row's list items, by their place among its elements.
pub type Markers = HashMap<String, Marker>;

impl Marker {
    fn live(&self) -> bool {
        self.shown && self.boxed
    }

    /// `animationend` once `now` is past the end: it settles the key the item rendered with.
    fn advance(&mut self, fade: &Fade, now: f64) {
        let length = motion::FAST.as_secs_f64() * 1000.0;
        if !self.live() || self.ended || now - self.since - self.delay < length {
            return;
        }
        self.ended = true;
        if let Some(key) = &self.key {
            fade.settle(key);
            self.shown = false;
        }
    }

    /// A render of the item with `key` at `now`; `marked` is whether the transform marked it.
    fn render(&mut self, key: Option<String>, marked: bool, boxed: bool, source: &Fade, now: f64) {
        let settled = key.as_ref().is_some_and(|key| source.settled(key));
        let prop = marked && !settled;
        let was = self.live();
        // The style React sets holds a delay for an item with no key alone.
        if self.key.is_some() != key.is_some() {
            self.delay = 0.0;
        }
        // The ref attaches again when the key changed, and queues a key not yet settled.
        if let Some(key) = key.as_ref().filter(|_| self.key != key && !settled) {
            self.delay = source.schedule(key, now);
        }
        // React writes the attribute only when its own prop changed.
        if self.prop != prop {
            self.shown = prop;
        }
        (self.key, self.prop, self.boxed) = (key, prop, boxed);
        if self.live() && !was {
            (self.since, self.ended) = (now, false);
        }
    }

    #[cfg(test)]
    pub fn state(&self) -> State {
        let animation = match (self.live(), self.ended) {
            (false, _) => "none",
            (true, false) => "running",
            (true, true) => "finished",
        };
        (self.shown, animation, self.live().then_some(self.delay))
    }
}

/// The animations of `markers` that ended by `now`.
pub fn advance(markers: &mut Markers, fade: &Fade, now: f64) {
    for marker in markers.values_mut() {
        marker.advance(fade, now);
    }
}

/// A render of `element` at `path`: an item React keeps keeps its record, another mounts.
pub fn render(
    element: &mut Element,
    (path, boxed): (&str, bool),
    source: &Fade,
    now: f64,
    (before, after): (&mut Markers, &mut Markers),
) {
    if element.tag == "li" {
        let key = match element.prop("dataWfKey") {
            Some(Prop::Text(key)) => Some(key.clone()),
            _ => None,
        };
        let marked = element.prop("dataStreamMarker").is_some();
        let mut marker = before.remove(path).unwrap_or(Marker {
            key: None,
            prop: false,
            shown: false,
            boxed,
            delay: 0.0,
            since: now,
            ended: false,
        });
        marker.render(key, marked, boxed, source, now);
        if marker.live() && !marker.ended {
            let begin = marker.since + marker.delay;
            element.props.push((BEGIN, Prop::Text(begin.to_string())));
        }
        after.insert(path.to_owned(), marker);
    }
    // A list whose own class replaced Cindy's has `list-style: none`.
    let boxed = match element.tag {
        "ul" | "ol" => element.prop("className").is_none(),
        _ => boxed,
    };
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for child in &mut element.children {
        let Hast::Element(child) = child else {
            continue;
        };
        // React keys a child by its tag's count.
        let nth = counts.entry(child.tag).or_default();
        let path = format!("{path}/{}-{nth}", child.tag);
        *nth += 1;
        render(child, (&path, boxed), source, now, (before, after));
    }
}
