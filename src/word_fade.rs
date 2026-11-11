//! `rehypeStreamWordFade.ts`: a streamed answer's words, their stable keys and fade timeline.
#![cfg_attr(
    not(test),
    expect(dead_code, reason = "a streamed row reads it in 9.6j3")
)]

use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap, HashSet},
    rc::Rc,
};

use crate::{
    cindy_json::{js_number, js_round},
    markdown::hast::{Element, Hast, Prop},
    theme_import::color::js_trim,
};

pub mod cache;
#[cfg(test)]
mod tests;

/// `FADE_DURATION_MS`.
const DURATION: f64 = 150.0;
/// `WORD_FADE_SEGMENT_DELAY_MS`.
const STEP: f64 = 16.0;
/// `WORD_FADE_MAX_DELAY_MS`.
const BACKLOG: f64 = 96.0;
/// `WORD_FADE_MAX_VISIBLE_DELAY_MS`.
const LONGEST: f64 = 160.0;
/// `WORD_FADE_BACKLOG_STEP_MS`.
const BACKLOG_STEP: f64 = 4.0;

/// `FadeSegmentKind`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Kind {
    Text,
    Code,
}

/// A segment a run saw, under the key it drew with.
#[derive(Clone, Debug)]
struct Seen {
    kind: Kind,
    content: String,
    key: String,
}

/// `WordFadeTimeline`: when each key starts, in milliseconds, and the keys that ended.
#[derive(Clone, Default)]
pub struct Timeline {
    start: HashMap<String, f64>,
    settled: HashSet<String>,
    next: f64,
}

/// `WordFadeState`: `createWordFadeState` is its default.
#[derive(Default)]
pub struct Fade {
    next_id: u64,
    previous: Vec<Seen>,
    prefix: String,
    timeline: Rc<RefCell<Timeline>>,
    sources: BTreeMap<usize, Fade>,
}

impl Fade {
    /// `getOrCreateWordFadeSourceState`: the chunk at `start` keeps its own segments.
    pub fn source(&mut self, start: usize) -> &mut Fade {
        let timeline = self.timeline.clone();
        let made = || Fade {
            prefix: format!("s{start}-"),
            timeline,
            ..Fade::default()
        };
        self.sources.entry(start).or_insert_with(made)
    }

    /// `createWordFadeCandidate`: a render's copy, which a commit publishes.
    pub fn candidate(&self) -> Fade {
        let timeline = self.timeline.borrow().clone();
        Fade {
            next_id: self.next_id,
            previous: self.previous.clone(),
            prefix: self.prefix.clone(),
            timeline: Rc::new(RefCell::new(timeline)),
            sources: BTreeMap::new(),
        }
    }

    /// `createWholeDocumentWordFadeCandidate`: the chunk at `start` reads every chunk's segments.
    pub fn whole_candidate(&self, start: usize) -> Fade {
        let committed = self.sources.get(&start).expect("the chunk has its state");
        let mut candidate = committed.candidate();
        let all = self.sources.values().flat_map(|source| &source.previous);
        let all: Vec<Seen> = all.cloned().collect();
        if !all.is_empty() {
            candidate.previous = all;
        }
        candidate
    }

    /// `retainOnlyWordFadeSourceState`.
    pub fn retain_only(&mut self, start: usize) {
        self.sources.retain(|key, _| *key == start);
    }

    /// `commitWordFadeCandidate`.
    pub fn commit(&mut self, candidate: &Fade) {
        self.next_id = candidate.next_id;
        self.previous.clone_from(&candidate.previous);
        let settled = candidate.timeline.borrow().settled.clone();
        self.timeline.borrow_mut().settled.extend(settled);
    }

    /// `markWordFadeSettled`.
    pub fn settle(&self, key: &str) {
        self.timeline.borrow_mut().settled.insert(key.to_owned());
    }

    /// `isWordFadeSettled`.
    pub fn settled(&self, key: &str) -> bool {
        self.timeline.borrow().settled.contains(key)
    }

    /// `scheduleWordFadeSegment`: the milliseconds until `key` starts, queued on first sight.
    pub fn schedule(&self, key: &str, now: f64) -> f64 {
        let mut timeline = self.timeline.borrow_mut();
        let start = match timeline.start.get(key) {
            Some(start) => *start,
            None => {
                let start = timeline.next.max(now).min(now + LONGEST);
                let delay = js_round(start - now).max(0.0);
                let step = if delay < BACKLOG { STEP } else { BACKLOG_STEP };
                timeline.start.insert(key.to_owned(), start);
                timeline.next = start + step;
                start
            }
        };
        js_round(start - now)
    }

    /// `assignKeys`: a segment seen before keeps its key, by place first, then by content.
    fn keys(&mut self, segments: &[(Kind, String)]) -> Vec<String> {
        let mut unmatched: HashSet<usize> = (0..self.previous.len()).collect();
        let mut by_content: HashMap<(Kind, &str), Vec<usize>> = HashMap::new();
        for (nth, seen) in self.previous.iter().enumerate().rev() {
            let same = by_content.entry((seen.kind, &seen.content)).or_default();
            same.push(nth);
        }
        let mut keys = Vec::with_capacity(segments.len());
        for (nth, (kind, content)) in segments.iter().enumerate() {
            let placed = self.previous.get(nth).filter(|seen| {
                let grown = *kind == Kind::Text && !seen.content.is_empty();
                let alike =
                    seen.content == *content || (grown && content.starts_with(&seen.content));
                unmatched.contains(&nth) && seen.kind == *kind && alike
            });
            if let Some(seen) = placed {
                unmatched.remove(&nth);
                keys.push(seen.key.clone());
                continue;
            }
            let mut same = by_content.get_mut(&(*kind, content.as_str()));
            let mut found = same.as_mut().and_then(|same| same.pop());
            while found.is_some_and(|at| !unmatched.contains(&at)) {
                found = same.as_mut().and_then(|same| same.pop());
            }
            match found {
                Some(at) => {
                    unmatched.remove(&at);
                    keys.push(self.previous[at].key.clone());
                }
                None => {
                    keys.push(format!("wf-{}{}", self.prefix, self.next_id));
                    self.next_id += 1;
                }
            }
        }
        let seen = segments
            .iter()
            .zip(&keys)
            .map(|((kind, content), key)| Seen {
                kind: *kind,
                content: content.clone(),
                key: key.clone(),
            });
        self.previous = seen.collect();
        keys
    }
}

/// `estimateWordFadeDelay`.
fn estimate(nth: usize) -> f64 {
    let raw = nth as f64 * STEP;
    if raw <= BACKLOG {
        return raw;
    }
    LONGEST.min(BACKLOG + (nth as f64 - (BACKLOG / STEP).floor()) * BACKLOG_STEP)
}

fn skipped(element: &Element) -> bool {
    let katex = matches!(element.prop("className"), Some(Prop::Words(words)) if words.iter().any(|word| word.starts_with("katex")));
    matches!(element.tag, "pre" | "script" | "style" | "textarea") || katex
}

/// A text node's words, none when it holds white space alone.
fn worded(text: &str) -> Option<Rc<Vec<String>>> {
    let words = cache::split_words(text);
    let blank = words.is_empty() || (words.len() == 1 && js_trim(&words[0]).is_empty());
    (!blank).then_some(words)
}

/// `collect`: the segments in document order, and how many list items hold them.
fn collect(nodes: &[Hast], out: &mut Vec<(Kind, String)>) -> usize {
    let mut items = 0;
    for node in nodes {
        match node {
            Hast::Element(element) if skipped(element) => {}
            Hast::Element(element) if element.tag == "code" => {
                let mut text = String::new();
                crate::markdown::hast::string(node, &mut text);
                out.push((Kind::Code, text));
            }
            Hast::Element(element) => {
                items += usize::from(element.tag == "li") + collect(&element.children, out);
            }
            Hast::Text(text) => {
                let words = worded(text).unwrap_or_default();
                let said = words.iter().filter(|word| !js_trim(word).is_empty());
                out.extend(said.map(|word| (Kind::Text, word.clone())));
            }
            Hast::Raw => {}
        }
    }
    items
}

/// One run's keys and clock, and the segment the walk has come to.
struct Run<'a> {
    keys: &'a [String],
    timeline: &'a Timeline,
    now: f64,
    at: usize,
}

impl Run<'_> {
    /// `renderDelay` as the `--wf-delay` style.
    fn style(&self, nth: usize) -> Prop {
        let delay = match self.timeline.start.get(&self.keys[nth]) {
            Some(start) => js_round(start - self.now),
            None => estimate(nth),
        };
        Prop::Text(format!("--wf-delay:{}ms", js_number(delay)))
    }

    /// `makeFadeNode` about the next segment.
    fn span(&mut self, children: Vec<Hast>) -> Hast {
        let nth = self.at;
        self.at += 1;
        let key = &self.keys[nth];
        let mut props = Vec::new();
        if !self.timeline.settled.contains(key) {
            props.push(("className", Prop::Words(vec!["stream-word".into()])));
        }
        props.push(("style", self.style(nth)));
        props.push(("dataWfKey", Prop::Text(key.clone())));
        Hast::Element(Element {
            tag: "span",
            props,
            children,
        })
    }

    /// `applyMarkerFade`: the item's marker fades with the first segment the item holds.
    fn marker(&self, item: &mut Element, from: usize) {
        let key = (self.at > from).then(|| &self.keys[from]);
        if key.is_some_and(|key| self.timeline.settled.contains(key)) {
            return;
        }
        let mut set = |name: &'static str, value: Prop| match item
            .props
            .iter_mut()
            .find(|(held, _)| *held == name)
        {
            Some(held) => held.1 = value,
            None => item.props.push((name, value)),
        };
        set("dataStreamMarker", Prop::Flag(true));
        // An item with no segment keeps the key it came with.
        if let Some(key) = key {
            set("dataWfKey", Prop::Text(key.clone()));
        }
        let style = match key {
            Some(_) => self.style(from),
            None => Prop::Text("--wf-delay:0ms".into()),
        };
        set("style", style);
    }

    /// `makeSlotNodes` for every slot under `nodes`, in `collect`'s order.
    fn wrap(&mut self, nodes: Vec<Hast>) -> Vec<Hast> {
        let mut out = Vec::with_capacity(nodes.len());
        for node in nodes {
            match node {
                Hast::Element(element) if skipped(&element) => out.push(Hast::Element(element)),
                Hast::Element(element) if element.tag == "code" => {
                    let code = Hast::Element(element);
                    out.push(self.span(vec![code]));
                }
                Hast::Element(mut element) => {
                    let from = self.at;
                    element.children = self.wrap(std::mem::take(&mut element.children));
                    if element.tag == "li" {
                        self.marker(&mut element, from);
                    }
                    out.push(Hast::Element(element));
                }
                Hast::Text(text) => match worded(&text) {
                    None => out.push(Hast::Text(text)),
                    Some(words) => self.words(&words, &mut out),
                },
                Hast::Raw => out.push(Hast::Raw),
            }
        }
        out
    }

    /// A text slot's nodes: a span per word, white space before the first word plain.
    fn words(&mut self, words: &[String], out: &mut Vec<Hast>) {
        let first = out.len();
        for word in words {
            if !js_trim(word).is_empty() {
                let span = self.span(vec![Hast::Text(word.clone())]);
                out.push(span);
            } else if let Some(Hast::Text(last)) = out[first..].last_mut() {
                last.push_str(word);
            } else if !word.is_empty() {
                out.push(Hast::Text(word.clone()));
            }
        }
    }
}

/// `rehypeStreamWordFade`'s transformer on a renderer tree, at `now` milliseconds.
pub fn fade(tree: &mut Vec<Hast>, state: &mut Fade, now: f64) {
    let mut segments = Vec::new();
    if collect(tree, &mut segments) == 0 && segments.is_empty() {
        return;
    }
    let keys = state.keys(&segments);
    let mut timeline = state.timeline.borrow_mut();
    // A span that remounted may never hear its animation end: time settles it.
    for key in &keys {
        if timeline
            .start
            .get(key)
            .is_some_and(|start| now - start >= DURATION)
        {
            timeline.settled.insert(key.clone());
        }
    }
    let mut run = Run {
        keys: &keys,
        timeline: &timeline,
        now,
        at: 0,
    };
    *tree = run.wrap(std::mem::take(tree));
    assert_eq!(run.at, keys.len(), "the walk and the collect count alike");
}

#[cfg(test)]
impl Fade {
    fn seen(&self) -> serde_json::Value {
        let kind = |seen: &Seen| match seen.kind {
            Kind::Text => "text",
            Kind::Code => "inline-code",
        };
        let seen = self.previous.iter();
        seen.map(|seen| serde_json::json!([kind(seen), seen.content, seen.key]))
            .collect()
    }

    /// The state as the golden's `snapshot` writes it.
    fn snapshot(&self) -> serde_json::Value {
        use crate::cindy_json::js_value as number;
        let timeline = self.timeline.borrow();
        let sources = self.sources.iter();
        let sources: Vec<_> = sources
            .map(|(key, source)| serde_json::json!([key, source.next_id, source.seen()]))
            .collect();
        let start: BTreeMap<&String, f64> = timeline.start.iter().map(|(k, v)| (k, *v)).collect();
        let start: Vec<_> = start
            .iter()
            .map(|(key, at)| serde_json::json!([key, number(*at)]))
            .collect();
        let mut settled: Vec<&String> = timeline.settled.iter().collect();
        settled.sort();
        serde_json::json!({
            "nextId": self.next_id,
            "previous": self.seen(),
            "sources": sources,
            "start": start,
            "settled": settled,
            "next": number(timeline.next),
        })
    }
}
