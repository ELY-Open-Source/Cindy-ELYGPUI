//! `MarkdownTargetLink`: what an `a` of the renderer tree draws as, and what its link opens.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};

use gpui::{
    AnyElement, App, Div, ElementId, FontStyle, FontWeight, InteractiveElement as _,
    IntoElement as _, ParentElement as _, Pixels, Role, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, canvas, div, prelude::FluentBuilder as _,
};

use super::{
    chip, handoff, jump,
    link_chip::{self, Act, Want},
    links::{self, Shared, Spots, Stop, What},
    markdown::text::Swap,
};
use crate::{
    appearance_settings::text_size,
    deep_link,
    i18n::{Arg, Translator},
    local_path::smart::{self, Smart},
    markdown::hast::{self, Element, Hast, Prop},
    markdown_target::{self, Local, Target, resolved},
    theme_import::color::js_trim,
    tip, toast,
};

pub mod file;
pub mod file_menu;
#[cfg(test)]
mod tests;

use file::{Asks, Chip, FileFace, Resolved};

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
    /// `SessionHandoffCard`.
    Card(deep_link::Card),
    /// `SessionLinkChip` or `ProjectLinkChip`.
    Chip(chip::Chip),
    /// A session or project link that names nothing: `<span>{label ?? href}</span>`.
    Words(String),
    /// A web address, with the open-with menu.
    External(SharedString),
    Inline(Go),
    /// A resolved local target, and the `href` that is its chip's title.
    File(FileFace, SharedString, Resolved),
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
    /// The resolved targets among the `a` and inline `code` nodes, and their mounted chips.
    files: HashMap<*const Element, FileFace>,
    chips: HashMap<*const Element, Chip>,
    /// Each deep link's chip: its width in window pixels, its height, and its element until drawn.
    links: HashMap<*const Element, (f32, f32, RefCell<Option<AnyElement>>)>,
    /// Each handoff card until drawn, and its metrics.
    cards: HashMap<*const Element, (RefCell<Option<AnyElement>>, handoff::Metrics)>,
    shared: Shared,
}

fn text<'a>(element: &'a Element, name: &str) -> Option<&'a str> {
    match element.prop(name) {
        Some(Prop::Text(value)) => Some(value),
        _ => None,
    }
}

/// The tree's `a` nodes and its inline `code` nodes, each kind in document order.
fn collect<'a>(nodes: &'a [Hast], links: &mut Vec<&'a Element>, codes: &mut Vec<&'a Element>) {
    for node in nodes {
        if let Hast::Element(element) = node {
            let inline = || ["dataFencedCode", "className"].map(|name| element.prop(name));
            match element.tag {
                "a" => links.push(element),
                "code" if inline() == [None, None] => codes.push(element),
                _ => {}
            }
            collect(&element.children, links, codes);
        }
    }
}

/// What an `a` takes from the elements about it: an `em`'s slant, and its weight.
type Around = (bool, FontWeight);

/// Each `a` node with what stands about it.
fn around(nodes: &[Hast], (em, weight): Around, found: &mut HashMap<*const Element, Around>) {
    for node in nodes {
        if let Hast::Element(element) = node {
            if element.tag == "a" {
                found.insert(element, (em, weight));
            }
            let weight = match element.tag {
                "strong" => FontWeight::BOLD,
                "h1" | "h2" | "h3" | "th" => FontWeight::MEDIUM,
                _ => weight,
            };
            around(
                &element.children,
                (em || element.tag == "em", weight),
                found,
            );
        }
    }
}

/// A resolved target's icon: `FileTypeIcon`, which a folder has none of.
fn icon(path: &str, kind: Local) -> Option<&'static [u8]> {
    (kind != Local::Directory).then(|| chip::file_icon(path))
}

/// The face of a target, once its hook has had its say; `label` is its children's text.
fn face(target: Target, bare: bool, dir: &str, label: &str) -> Face {
    match target {
        Target::Resolved {
            href,
            path,
            kind,
            at,
        } => {
            // A bare path lights as a link, and so does a label of prose.
            let boxed = !bare && file::code_label(&href, at, label);
            let shown = boxed || !js_trim(label).is_empty();
            let icon = icon(&path, kind).filter(|_| shown);
            let file = Resolved {
                path: path.into(),
                folder: kind == Local::Directory,
                at,
                cwd: dir.to_owned().into(),
            };
            Face::File(FileFace { boxed, icon }, href.into(), file)
        }
        Target::External { href } => Face::External(href.into()),
        Target::Anchor { id, .. } => Face::Inline(Go::Heading(id)),
        Target::LocalImageUrl { .. } => Face::Inline(Go::Image),
        Target::Candidate { .. } if !bare => Face::Inline(Go::Local(target, dir.to_owned())),
        // The audio player comes with 9.13.
        Target::Audio { .. }
        | Target::Candidate { .. }
        | Target::Code { .. }
        | Target::Plain { .. } => Face::Span,
    }
}

/// A session or project link's face: its chip with the author's label, else its words.
fn deep(href: &str, label: &str, wants: &mut Vec<Want>, cx: &App) -> Option<Face> {
    let session = deep_link::strip(href, "session/").is_some();
    if !session && deep_link::strip(href, "project/").is_none() {
        return None;
    }
    let label = Some(js_trim(label)).filter(|label| !label.is_empty() && *label != href);
    let chip = match session {
        true => {
            let known = (link_chip::held(cx), cx.global::<Translator>());
            link_chip::session(href, label, known, wants)
        }
        false => link_chip::project(href, label),
    };
    Some(match chip {
        Some(chip) => Face::Chip(chip),
        None => Face::Words(label.unwrap_or(href).to_owned()),
    })
}

/// The `a` renderer for the node mounted as `id`, in a privileged message of a local task.
fn anchor(
    id: &str,
    a: &Element,
    (files, dir): (&[(String, String)], &str),
    wants: &mut Vec<Want>,
    cx: &mut App,
) -> Anchor {
    let href = text(a, "href");
    let raw = text(a, "data-cindy-raw-local-link-href");
    let handed = raw.or(href);
    let mut label = String::new();
    (a.children.iter()).for_each(|child| hast::string(child, &mut label));
    let face = if let Some(card) = href.and_then(deep_link::parse_card) {
        Face::Card(card)
    } else if let Some(face) = href.and_then(|href| deep(href, &label, wants, cx)) {
        face
    } else {
        let target = markdown_target::link(handed, files);
        let lit = resolved::resolved(id, &target, dir, false, cx);
        let bare = a.prop("data-bare-path").is_some();
        face(lit.unwrap_or(target), bare, dir, &label)
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
    let (mut found, mut codes) = (Vec::new(), Vec::new());
    collect(tree, &mut found, &mut codes);
    let mut wants = Vec::new();
    let nodes = found.iter().enumerate();
    let list = nodes.map(|(n, a)| anchor(&format!("{key}-a{n}"), a, (files, dir), &mut wants, cx));
    let list: Vec<Anchor> = list.collect();
    if !wants.is_empty() {
        cx.defer(move |cx| link_chip::fetch(wants, cx));
    }
    // A Tab stop per `a`, then one per inline code.
    let stops = list.len() + codes.len();
    let shared = links::shared(&format!("{key}-md"), stops, window, cx);
    let (mut faces, mut chips) = (HashMap::new(), HashMap::new());
    // A chip's boxes are its own record: the menu key asks under them.
    let asks = |file: Resolved| Asks {
        file,
        asked: shared.asked().clone(),
        spots: Spots::default(),
    };
    for (n, (a, anchor)) in found.iter().zip(&list).enumerate() {
        let Face::File(face, href, file) = &anchor.face else {
            continue;
        };
        let node = *a as *const Element;
        faces.insert(node, *face);
        if face.boxed {
            let (id, focus) = (format!("{key}-a{n}"), shared.stop(n).0);
            let chip = file::chip(&id, href.clone(), focus, asks(file.clone()), window, cx);
            chips.insert(node, chip);
        }
    }
    for (n, code) in codes.iter().enumerate() {
        let id = format!("{key}-i{n}");
        let Some((face, file)) = inline_code(&id, code, dir, cx) else {
            continue;
        };
        let (node, focus) = (*code as *const Element, shared.stop(list.len() + n).0);
        faces.insert(node, face);
        let chip = file::chip(&id, file.path.clone(), focus, asks(file), window, cx);
        chips.insert(node, chip);
    }
    let mut links = HashMap::new();
    let mut about = HashMap::new();
    around(tree, (false, FontWeight::NORMAL), &mut about);
    let mut cards = HashMap::new();
    for (n, (a, anchor)) in found.iter().zip(&list).enumerate() {
        let Face::Card(card) = &anchor.face else {
            continue;
        };
        let node = *a as *const Element;
        let id = format!("{key}-card-a{n}");
        let target = deep_link::Session {
            session_id: card.session_id.clone(),
            message_client_id: None,
            device_id: None,
        };
        link_chip::mounted(&id, (0, n), target, cx);
        let (focus, weight) = (shared.stop(n).0, about[&node].1);
        let (element, metrics) = handoff::element(&id, card, weight, focus, window, cx);
        cards.insert(node, (RefCell::new(Some(element)), metrics));
    }
    let size = text_size(12.0, cx);
    for (n, (a, anchor)) in found.iter().zip(&list).enumerate() {
        let Face::Chip(chip) = &anchor.face else {
            continue;
        };
        let id = format!("{key}-chip-a{n}");
        if let Some(Act::Session(href)) = chip.act() {
            link_chip::drawn(&id, (0, n), href, cx);
        }
        // `InlineReferenceChip` resets the weight alone: an `em` about the link slants its label.
        let italic = chip.slants() && about[&(*a as *const Element)].0;
        let mut style = window.text_style();
        if italic {
            style.font_style = FontStyle::Italic;
        }
        let width = chip.width(size, &style, window, cx);
        let element = chip.element(id, (Pixels::ZERO, size), Some(&shared), window, cx);
        let element = match italic {
            true => div().italic().child(element).into_any_element(),
            false => element,
        };
        let drawn = (width.into(), chip.height(), RefCell::new(Some(element)));
        links.insert(*a as *const Element, drawn);
    }
    let by = found.iter().enumerate().map(|(n, a)| (*a as *const _, n));
    Anchors {
        links,
        cards,
        key: key.to_owned(),
        by: by.collect(),
        list,
        files: faces,
        chips,
        shared,
    }
}

/// `InlineCodeWithTarget` for the node mounted as `id`: its chip's face and file, once resolved.
fn inline_code(id: &str, code: &Element, dir: &str, cx: &mut App) -> Option<(FileFace, Resolved)> {
    let mut text = String::new();
    (code.children.iter()).for_each(|child| hast::string(child, &mut text));
    let target = markdown_target::inline_code(&text)?;
    let lit = resolved::resolved(id, &target, dir, false, cx)?;
    let Target::Resolved { path, kind, at, .. } = lit else {
        panic!("a hook lights a candidate as a resolved target");
    };
    let face = FileFace {
        boxed: true,
        icon: icon(&path, kind),
    };
    let file = Resolved {
        path: path.into(),
        folder: kind == Local::Directory,
        at,
        cwd: dir.to_owned().into(),
    };
    Some((face, file))
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
    let Some(lit) = resolved::lit(target, found) else {
        return links::failed("chat.markdownRenderer.fileUnavailable", window, cx);
    };
    let Target::Resolved { path, kind, .. } = lit else {
        panic!("a lookup lights a candidate as a resolved target");
    };
    if kind != Local::Directory && file_menu::html(&path) {
        return file_menu::page(&path, None, window, cx);
    }
    // `activateResolvedLocalTarget` comes with 9.13.
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
    /// Whether nothing in the message asks the menu: no `a`, and no inline code that is a chip.
    pub fn is_empty(&self) -> bool {
        self.list.is_empty() && self.chips.is_empty()
    }

    pub fn shared(&self) -> &Shared {
        &self.shared
    }

    /// Whether the node draws as a link: underlined, and broken anywhere.
    pub fn marked(&self, node: *const Element) -> bool {
        !matches!(
            self.list[self.by[&node]].face,
            Face::Span | Face::File(FileFace { boxed: true, .. }, ..)
        )
    }

    /// The face of an `a` or an inline `code` that is a resolved target.
    pub fn file(&self, node: *const Element) -> Option<FileFace> {
        self.files.get(&node).copied()
    }

    /// What the reader puts in place of the node's own inline face.
    pub fn swap(&self, node: *const Element) -> Option<Swap> {
        if let Some(file) = self.file(node) {
            return Some(Swap::File(file));
        }
        match &self.list[*self.by.get(&node)?].face {
            Face::Chip(_) => Some(Swap::Chip),
            Face::Words(words) => Some(Swap::Words(words.clone())),
            Face::Card(_) => Some(Swap::Card),
            _ => None,
        }
    }

    /// A deep link's chip: its width in window pixels and its margin box's height.
    pub fn link_size(&self, node: *const Element) -> (f32, f32) {
        let (width, height, _) = &self.links[&node];
        (*width, *height)
    }

    /// A handoff card, drawn once.
    pub fn card(&self, node: *const Element) -> AnyElement {
        let card = self.cards[&node].0.take();
        card.expect("a card is drawn once")
    }

    pub fn card_metrics(&self, node: *const Element) -> handoff::Metrics {
        self.cards[&node].1
    }

    /// A deep link's chip, which a row draws once.
    pub fn link_chip(&self, node: *const Element) -> AnyElement {
        let drawn = self.links[&node].2.take();
        drawn.expect("a link chip is drawn once")
    }

    /// The mounted chip of a node whose face is the chip's box.
    pub fn chip(&self, node: *const Element) -> &Chip {
        &self.chips[&node]
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
        let name = format!("{}-a{nth}-{at}", self.key);
        let named = |piece: Div| piece.id(ElementId::Name(name.into()));
        let listened = match &anchor.face {
            // A chip's box and a card take their own clicks.
            Face::Span | Face::Card(_) | Face::File(FileFace { boxed: true, .. }, ..) => {
                return piece.into_any_element();
            }
            Face::Chip(_) | Face::Words(_) => panic!("a deep link's face holds no piece"),
            Face::File(_, _, file) => {
                let asks = Asks {
                    file: file.clone(),
                    asked: self.shared.asked().clone(),
                    spots,
                };
                file::listen(named(piece), first, &focus, Role::Link, &asks)
            }
            Face::External(url) => {
                let (what, stop) = (
                    What::Url(url.clone()),
                    Stop {
                        first,
                        focus,
                        spots,
                    },
                );
                links::listen(named(piece), what, stop, self.shared.asked())
            }
            Face::Inline(go) => {
                let what = What::Target(go.clone(), anchor.href.clone());
                let stop = Stop {
                    first,
                    focus,
                    spots,
                };
                links::listen(named(piece), what, stop, self.shared.asked())
            }
        };
        // The native `title` is gpui's one tooltip; an empty one shows nothing.
        let title = anchor.title.clone().filter(|title| !title.is_empty());
        listened
            .when_some(title, |el, title| el.tooltip(tip::tip(title)))
            .into_any_element()
    }
}
