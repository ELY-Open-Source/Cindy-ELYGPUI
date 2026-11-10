//! `UserMessageUrlLink`: a message's web addresses over its text, with `useOpenWithMenu`.

use std::{cell::RefCell, ops::Range, rc::Rc};

use ely_gpui_component::{
    menus::{ContextMenu, Menu, MenuItem},
    primitives::IconName,
};
use gpui::{
    AnyElement, App, Bounds, ClickEvent, ClipboardItem, ElementId, Entity, FocusHandle,
    HighlightStyle, InteractiveElement as _, IntoElement, LineLayout, Modifiers, MouseButton,
    ParentElement as _, Pixels, Point, Role, SharedString, StatefulInteractiveElement as _,
    Styled as _, StyledText, TextLayout, UnderlineStyle, Window, canvas, div, point,
    prelude::FluentBuilder as _, px, size,
};

use super::link_chip::{Act, act};
use crate::{
    i18n::Translator,
    link_open::{self, Target},
    open_external, toast,
    units::css,
};

/// A web address and the bytes of it a piece of text holds; `nth` counts the message's links.
pub struct Link {
    pub nth: usize,
    pub range: Range<usize>,
    pub url: SharedString,
}

/// What a menu was asked for: a web address, or a link chip by its element's name.
#[derive(Clone)]
pub enum What {
    Url(SharedString),
    Chip(Act, SharedString),
}

/// `useOpenWithMenu`'s and `useLinkContextMenu`'s state: the numbered ask and what it is for.
#[derive(Default)]
pub struct Asked {
    request: Option<(u64, Point<Pixels>)>,
    what: Option<What>,
}

/// A link's boxes in the window, in the order they were placed.
type Spots = Rc<RefCell<Vec<Bounds<Pixels>>>>;

/// What a message's links share: the menu's ask, and each link's Tab stop and boxes.
pub struct Shared {
    asked: Entity<Asked>,
    focus: Vec<FocusHandle>,
    spots: Vec<Spots>,
}

/// A box per stretch of the range's glyphs, from the shaped line's origin.
fn boxes(line: &LineLayout, range: Range<usize>, height: Pixels) -> Vec<Bounds<Pixels>> {
    let glyphs = line.runs.iter().flat_map(|run| run.glyphs.iter());
    let mut placed: Vec<(Pixels, bool)> = glyphs
        .map(|glyph| (glyph.position.x, range.contains(&glyph.index)))
        .collect();
    // The shaper lists right-to-left text from the right: x gives the visual order.
    placed.sort_by(|a, b| f32::from(a.0).total_cmp(&f32::from(b.0)));
    let (mut out, mut open) = (Vec::new(), None);
    let mut close = |from: Pixels, to: Pixels| {
        out.push(Bounds::new(point(from, px(0.0)), size(to - from, height)));
    };
    for (x, inside) in placed {
        if let Some(from) = open.filter(|_| !inside) {
            close(from, x);
            open = None;
        }
        if inside && open.is_none() {
            open = Some(x);
        }
    }
    if let Some(from) = open {
        close(from, line.width);
    }
    out
}

pub fn failed(key: &str, window: &Window, cx: &mut App) {
    let message = cx.global::<Translator>().t(key, &[]);
    toast::error(window, message, toast::Options::default(), cx);
}

/// `openExternal`, with `openLinkFailed` as its toast.
fn external(url: &str, window: &Window, cx: &mut App) {
    let opening = open_external::open(url, cx);
    let handle = window.window_handle();
    cx.spawn(async move |cx| {
        if opening.await {
            return;
        }
        let shown = handle.update(cx, |_, window, cx| {
            failed("chat.markdownRenderer.openLinkFailed", window, cx)
        });
        if let Err(error) = shown {
            log::warn!(target: "sessionView", "the link's failure had no window: {error:#}");
        }
    })
    .detach();
}

/// `openInSidebar`: the sidebar browser comes with 17.0, so the open fails as Cindy's fails.
fn sidebar(window: &Window, cx: &mut App) {
    log::error!(target: "sessionView", "no sidebar browser opens a link before 17.0");
    failed("chat.markdownRenderer.openInSidebarFailed", window, cx);
}

/// `activateMessageUrl` in a task; the mail preview comes with its chapter.
fn activate(url: &str, modifiers: Modifiers, button: MouseButton, window: &Window, cx: &mut App) {
    let apart = link_open::separate(modifiers, button);
    log::info!(target: "sessionView", "a message link activated, apart: {apart}");
    match !apart && link_open::for_url(url, cx) == Target::Sidebar {
        true => sidebar(window, cx),
        false => external(url, window, cx),
    }
}

/// `handleCopyLink`; gpui's clipboard write reports no failure.
fn copy(url: &str, window: &Window, cx: &mut App) {
    cx.write_to_clipboard(ClipboardItem::new_string(url.to_owned()));
    let message = cx
        .global::<Translator>()
        .t("chat.markdownRenderer.linkCopied", &[]);
    toast::success(window, message, toast::Options::default(), cx);
}

/// The asked menu, Copy link last; Ely's `Link` stands in for lucide's `link-2`.
fn menu(what: Option<&What>, cx: &App) -> Menu {
    let t = cx.global::<Translator>();
    let label = |key: &str| t.t(&format!("chat.markdownRenderer.{key}"), &[]);
    let copy = |href: &SharedString| {
        let copied = href.clone();
        MenuItem::new(label("copyLink"))
            .icon(IconName::Link)
            .on_click(move |window, cx| copy(&copied, window, cx))
    };
    let url = match what {
        None => return Menu::new(),
        Some(What::Chip(to, id)) => {
            let (Act::Session(href) | Act::Project(href)) = to;
            return act::menu(to, id, cx).separator().item(copy(href));
        }
        Some(What::Url(url)) => url,
    };
    let out = url.clone();
    let sidebar = MenuItem::new(label("openInSidebarBrowser"))
        .icon(IconName::PanelRight)
        .on_click(|window, cx| sidebar(window, cx));
    let browser = MenuItem::new(label("openInDefaultBrowser"))
        .icon(IconName::Globe)
        .keys(label("shiftClick"))
        .on_click(move |window, cx| external(&out, window, cx));
    Menu::new()
        .item(sidebar)
        .item(browser)
        .separator()
        .item(copy(url))
}

pub fn ask(asked: &Entity<Asked>, what: What, at: Point<Pixels>, cx: &mut App) {
    asked.update(cx, |asked, cx| {
        let number = asked.request.map_or(1, |(number, _)| number + 1);
        *asked = Asked {
            request: Some((number, at)),
            what: Some(what),
        };
        cx.notify();
    });
}

/// One box of a link; the first is its Tab stop.
struct Piece {
    id: String,
    first: bool,
    size: gpui::Size<Pixels>,
    url: SharedString,
    focus: FocusHandle,
    asked: Entity<Asked>,
    spots: Spots,
}

impl Piece {
    fn element(self) -> AnyElement {
        let Piece {
            id,
            first,
            size,
            url,
            focus,
            asked,
            spots,
        } = self;
        let selector = id.clone();
        let (left, middle, right, keys) = (url.clone(), url.clone(), url.clone(), url);
        let (pressed, menu, by_key) = (focus.clone(), asked.clone(), asked);
        let back = focus.clone();
        div()
            .id(ElementId::Name(id.into()))
            .debug_selector(move || selector)
            .w(size.width)
            .h(size.height)
            .cursor_pointer()
            .when(first, |el| {
                el.track_focus(&focus)
                    .role(Role::Link)
                    .aria_label(keys.clone())
                    .on_key_down(move |event, window, cx| {
                        let k = &event.keystroke;
                        if k.key == "menu" || (k.key == "f10" && k.modifiers.shift) {
                            window.prevent_default();
                            cx.stop_propagation();
                            // `linkMenuPosition` for a key: the link's left and bottom.
                            let spots = spots.borrow();
                            let left = spots.iter().map(|b| b.left()).reduce(Pixels::min);
                            let bottom = spots.last().map(Bounds::bottom);
                            let under = point(left.unwrap_or_default(), bottom.unwrap_or_default());
                            ask(&by_key, What::Url(keys.clone()), under, cx);
                        } else if matches!(k.key.as_str(), "enter" | "space") {
                            window.prevent_default();
                            activate(&keys, k.modifiers, MouseButton::Left, window, cx);
                        }
                    })
            })
            .on_any_mouse_down(move |_, window, cx| pressed.focus(window, cx))
            // Keys activate on key down above; gpui's key-up click is not Cindy's.
            .on_click(move |event, window, cx| {
                if matches!(event, ClickEvent::Mouse(_)) {
                    cx.stop_propagation();
                    activate(&left, event.modifiers(), MouseButton::Left, window, cx);
                }
            })
            .on_aux_click(move |event, window, cx| {
                let ClickEvent::Mouse(mouse) = event else {
                    return;
                };
                if mouse.up.button == MouseButton::Middle {
                    cx.stop_propagation();
                    activate(&middle, event.modifiers(), MouseButton::Middle, window, cx);
                }
            })
            .on_mouse_down(MouseButton::Right, move |event, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
                // The stop keeps the press from the listener above: the menu hands focus back here.
                back.focus(window, cx);
                ask(&menu, What::Url(right.clone()), event.position, cx);
            })
            .into_any_element()
    }
}

/// The links' hit boxes over a piece of text, placed once the text is laid out.
fn overlay(key: String, links: Vec<Link>, layout: TextLayout, shared: &Shared) -> impl IntoElement {
    let asked = shared.asked.clone();
    let stops = |link: &Link| {
        (
            shared.focus[link.nth].clone(),
            shared.spots[link.nth].clone(),
        )
    };
    let stops: Vec<(FocusHandle, Spots)> = links.iter().map(stops).collect();
    let prepaint = move |_, window: &mut Window, cx: &mut App| {
        let line = layout.line_layout_for_index(0);
        let line = line.expect("a piece with a link is drawn");
        let origin = layout.position_for_index(0);
        let origin = origin.expect("a drawn line has its origin");
        let mut drawn = Vec::new();
        for (link, (focus, spots)) in links.iter().zip(stops) {
            let found = boxes(
                &line.unwrapped_layout,
                link.range.clone(),
                layout.line_height(),
            );
            for bounds in found {
                let at = origin + bounds.origin;
                let row = spots.borrow().len();
                spots.borrow_mut().push(Bounds::new(at, bounds.size));
                let piece = Piece {
                    id: format!("{key}-link-{}-{row}", link.nth),
                    first: row == 0,
                    size: bounds.size,
                    url: link.url.clone(),
                    focus: focus.clone(),
                    asked: asked.clone(),
                    spots: spots.clone(),
                };
                let mut piece = piece.element();
                piece.prepaint_as_root(at, bounds.size.into(), window, cx);
                drawn.push(piece);
            }
        }
        drawn
    };
    let paint = |_, drawn: Vec<AnyElement>, window: &mut Window, cx: &mut App| {
        for mut piece in drawn {
            piece.paint(window, cx);
        }
    };
    canvas(prepaint, paint).absolute().size_full()
}

impl Shared {
    pub fn asked(&self) -> &Entity<Asked> {
        &self.asked
    }
}

/// The state `count` links of the message at `key` share.
pub fn shared(key: &str, count: usize, window: &mut Window, cx: &mut App) -> Shared {
    let state = |name: String| ElementId::Name(name.into());
    let asked = window.use_keyed_state(state(format!("{key}-link-asked")), cx, |_, _| {
        Asked::default()
    });
    let stop = |n| {
        let id = state(format!("{key}-link-{n}-focus"));
        let stop = window.use_keyed_state(id, cx, |_, cx| cx.focus_handle().tab_stop(true));
        stop.read(cx).clone()
    };
    Shared {
        asked,
        focus: (0..count).map(stop).collect(),
        spots: (0..count).map(|_| Spots::default()).collect(),
    }
}

/// `underline`; gpui places the line itself, so `underline-offset-2` has no port.
pub fn underline(window: &Window) -> HighlightStyle {
    HighlightStyle {
        underline: Some(UnderlineStyle {
            thickness: css(1.0).to_pixels(window.rem_size()),
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// `text` with `marks` underlined.
pub fn marked(text: String, mut marks: Vec<Range<usize>>, window: &Window) -> StyledText {
    marks.sort_by_key(|mark| mark.start);
    let underline = underline(window);
    StyledText::new(text).with_highlights(marks.into_iter().map(|mark| (mark, underline)))
}

/// A piece of the message's text with its links and `marks` underlined.
pub fn piece(
    key: &str,
    (text, mut marks): (String, Vec<Range<usize>>),
    links: Vec<Link>,
    shared: &Shared,
    window: &Window,
) -> AnyElement {
    marks.extend(links.iter().map(|link| link.range.clone()));
    let styled = marked(text, marks, window);
    let layout = styled.layout().clone();
    div()
        .relative()
        .flex_none()
        .child(styled)
        .child(overlay(key.to_owned(), links, layout, shared))
        .into_any_element()
}

/// The menu the message's links share, around its text.
pub fn host(key: &str, shared: &Shared, text: AnyElement, cx: &App) -> AnyElement {
    let (request, what) = {
        let asked = shared.asked.read(cx);
        (asked.request, asked.what.clone())
    };
    let id = ElementId::Name(format!("{key}-link-menu").into());
    ContextMenu::new(id, menu(what.as_ref(), cx))
        .manual(request)
        .child(text)
        .into_any_element()
}

#[cfg(test)]
pub(super) mod tests;
