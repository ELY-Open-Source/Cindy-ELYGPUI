//! `MarkdownTargetLink`: what an `a` of the renderer tree draws as, and what its link opens.

use std::{cell::Cell, collections::HashMap};

use gpui::{
    AnyElement, App, Div, ElementId, InteractiveElement as _, IntoElement as _, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, canvas,
    prelude::FluentBuilder as _,
};

use super::{
    jump,
    links::{self, Shared, Stop, What},
};
use crate::{
    deep_link,
    i18n::{Arg, Translator},
    local_path::smart::{self, Smart},
    markdown::hast::{Element, Hast, Prop},
    markdown_target::{self, Target, resolved},
    tip, toast,
};

#[cfg(test)]
mod tests;

/// What `openInlineTarget` opens.
#[derive(Clone)]
pub enum Go {
    /// An element of the page, by its id.
    Heading(String),
    /// A stored image's address.
    Image,
    /// An authored local link: its candidate and the task's folder.
    Local(Target, String),
}

/// How an `a` draws.
pub enum Face {
    /// `<span>`: its text alone.
    Span,
    /// A session, project or session-card link: its chip and card come with 9.6h5.
    Later,
    /// A web address, with the open-with menu.
    External(SharedString),
    Inline(Go),
}

struct Anchor {
    face: Face,
    /// The `href` the renderer handed on, which Copy link copies.
    href: SharedString,
    id: Option<String>,
    title: Option<SharedString>,
    /// Whether a box of it is drawn yet: the first is its Tab stop.
    drawn: Cell<bool>,
}

/// A message's `a` elements in document order, each by its node's address.
pub struct Anchors {
    key: String,
    by: HashMap<*const Element, usize>,
    list: Vec<Anchor>,
    shared: Shared,
}

fn text<'a>(element: &'a Element, name: &str) -> Option<&'a str> {
    match element.prop(name) {
        Some(Prop::Text(value)) => Some(value),
        _ => None,
    }
}

fn collect<'a>(nodes: &'a [Hast], out: &mut Vec<&'a Element>) {
    for node in nodes {
        if let Hast::Element(element) = node {
            if element.tag == "a" {
                out.push(element);
            }
            collect(&element.children, out);
        }
    }
}

/// The face of a target, once its hook has had its say.
fn face(target: Target, bare: bool, dir: &str) -> Face {
    match target {
        Target::External { href } => Face::External(href.into()),
        Target::Anchor { id, .. } => Face::Inline(Go::Heading(id)),
        Target::LocalImageUrl { .. } => Face::Inline(Go::Image),
        Target::Candidate { .. } if !bare => Face::Inline(Go::Local(target, dir.to_owned())),
        // The audio player comes with 9.13, a resolved target's chip and link with 9.6h4.
        Target::Audio { .. }
        | Target::Resolved { .. }
        | Target::Candidate { .. }
        | Target::Code { .. }
        | Target::Plain { .. } => Face::Span,
    }
}

/// The `a` renderer for the node mounted as `id`, in a privileged message of a local task.
fn anchor(id: &str, a: &Element, files: &[(String, String)], dir: &str, cx: &mut App) -> Anchor {
    let href = text(a, "href");
    let raw = text(a, "data-cindy-raw-local-link-href");
    let handed = raw.or(href);
    let deep = |path| href.is_some_and(|href| deep_link::strip(href, path).is_some());
    let face = if deep("session-card/") || deep("session/") || deep("project/") {
        Face::Later
    } else {
        let target = markdown_target::link(handed, files);
        let lit = resolved::resolved(id, &target, dir, false, cx);
        face(
            lit.unwrap_or(target),
            a.prop("data-bare-path").is_some(),
            dir,
        )
    };
    Anchor {
        face,
        href: handed.unwrap_or_default().to_owned().into(),
        id: text(a, "id").map(str::to_owned),
        title: text(a, "title").map(|title| title.to_owned().into()),
        drawn: Cell::new(false),
    }
}

/// The message's links: `files` are the task's attached files, `dir` its folder.
pub fn read(
    key: &str,
    tree: &[Hast],
    files: &[(String, String)],
    dir: &str,
    window: &mut Window,
    cx: &mut App,
) -> Anchors {
    let mut found = Vec::new();
    collect(tree, &mut found);
    let nodes = found.iter().enumerate();
    let list = nodes.map(|(n, a)| anchor(&format!("{key}-a{n}"), a, files, dir, cx));
    let list: Vec<Anchor> = list.collect();
    let by = found.iter().enumerate().map(|(n, a)| (*a as *const _, n));
    Anchors {
        key: key.to_owned(),
        by: by.collect(),
        shared: links::shared(&format!("{key}-md"), list.len(), window, cx),
        list,
    }
}

/// The resolved lookup of an authored link's click.
fn landed(target: &Target, found: &Smart, window: &Window, cx: &mut App) {
    if let Smart::Multiple(candidates) = found {
        let count = Arg::Number(candidates.len() as i64);
        let t = cx.global::<Translator>();
        let message = t.t("chat.markdownRenderer.duplicateFiles", &[("count", count)]);
        toast::error(window, message, toast::Options::default(), cx);
        return;
    }
    if resolved::lit(target, found).is_none() {
        return links::failed("chat.markdownRenderer.fileUnavailable", window, cx);
    }
    // `openHtmlFileByPreference` and `activateResolvedLocalTarget` come with 9.13.
    log::warn!(target: "sessionView", "a local link resolved: nothing opens it before 9.13");
}

/// `openInlineTarget`.
pub fn open(go: &Go, window: &mut Window, cx: &mut App) {
    match go {
        Go::Heading(id) if id.is_empty() => {}
        Go::Heading(id) => {
            if !jump::to(id, window, cx) {
                links::failed("chat.markdownRenderer.anchorNotFound", window, cx);
            }
        }
        Go::Image => {
            log::warn!(target: "sessionView", "no lightbox opens an image link before 9.13");
        }
        Go::Local(target, dir) => {
            let Target::Candidate { href, .. } = target else {
                panic!("an authored local link holds a candidate");
            };
            log::info!(target: "sessionView", "an authored local link resolves again");
            let looking = smart::resolve_smart(href, dir, true, cx);
            let (handle, target) = (window.window_handle(), target.clone());
            cx.spawn(async move |cx| {
                let found = looking.await;
                let shown = handle.update(cx, |_, window, cx| landed(&target, &found, window, cx));
                if let Err(error) = shown {
                    log::warn!(target: "sessionView", "the link's answer had no window: {error:#}");
                }
            })
            .detach();
        }
    }
}

impl Anchors {
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub fn shared(&self) -> &Shared {
        &self.shared
    }

    /// Whether the node draws as a link: underlined, and broken anywhere.
    pub fn marked(&self, node: *const Element) -> bool {
        !matches!(self.list[self.by[&node]].face, Face::Span)
    }

    /// A box of the node, drawn as `piece` and named `at`: its id, and its link's clicks and keys.
    pub fn dress(&self, node: *const Element, piece: Div, at: &str) -> AnyElement {
        let nth = self.by[&node];
        let anchor = &self.list[nth];
        let first = !anchor.drawn.replace(true);
        let (focus, spots) = self.shared.stop(nth);
        let (id, held) = (anchor.id.clone().filter(|_| first), spots.clone());
        let probe = canvas(
            move |bounds, _, cx| {
                held.borrow_mut().push(bounds);
                if let Some(id) = &id {
                    jump::painted(id, bounds, cx);
                }
            },
            |_, _, _, _| {},
        );
        let piece = piece.relative().child(probe.absolute().inset_0());
        let what = match &anchor.face {
            Face::Span | Face::Later => return piece.into_any_element(),
            Face::External(url) => What::Url(url.clone()),
            Face::Inline(go) => What::Target(go.clone(), anchor.href.clone()),
        };
        let stop = Stop {
            first,
            focus,
            spots,
        };
        let name = format!("{}-a{nth}-{at}", self.key);
        let piece = piece.id(ElementId::Name(name.into()));
        // The native `title` is gpui's one tooltip; an empty one shows nothing.
        let title = anchor.title.clone().filter(|title| !title.is_empty());
        links::listen(piece, what, stop, self.shared.asked())
            .when_some(title, |el, title| el.tooltip(tip::tip(title)))
            .into_any_element()
    }
}
