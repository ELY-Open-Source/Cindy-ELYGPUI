//! `UserMessageUrlLink`: the web addresses of an unfolded message, with `useOpenWithMenu`.

use std::ops::Range;

use ely_gpui_component::{
    menus::{ContextMenu, Menu, MenuItem},
    primitives::IconName,
};
use gpui::{
    AnyElement, App, Bounds, ClickEvent, ClipboardItem, ElementId, Entity, FocusHandle,
    HighlightStyle, InteractiveElement as _, IntoElement, Modifiers, MouseButton,
    ParentElement as _, Pixels, Point, Role, SharedString, StatefulInteractiveElement as _,
    Styled as _, StyledText, TextLayout, UnderlineStyle, Window, WrappedLineLayout, canvas, div,
    point, prelude::FluentBuilder as _, size,
};

use crate::{
    i18n::Translator,
    link_open::{self, Target},
    linkify, open_external, toast,
    units::css,
};

/// A web address and where its text lies in the drawn text.
pub struct Link {
    pub range: Range<usize>,
    pub url: SharedString,
}

/// `useOpenWithMenu`'s state: the numbered ask and the address it is for.
#[derive(Default)]
struct Asked {
    request: Option<(u64, Point<Pixels>)>,
    url: SharedString,
}

/// The web links of `body`, moved to where the drawn text keeps them; `map` pairs offsets.
pub fn place(body: &str, map: &[(usize, usize)]) -> Vec<Link> {
    let urls = linkify::find(body)
        .into_iter()
        .filter(|found| matches!(found.link, linkify::Link::Url));
    urls.map(|found| {
        let url = &body[found.range.clone()];
        let start = found.range.start - body[..found.range.start].matches('\r').count();
        let (from, to) = map[map.partition_point(|(from, _)| *from <= start) - 1];
        let start = to + start - from;
        Link {
            range: start..start + url.len(),
            url: url.to_owned().into(),
        }
    })
    .collect()
}

/// A box per stretch of the range's glyphs on a visual line, from the shaped line's origin.
fn boxes(line: &WrappedLineLayout, range: Range<usize>, height: Pixels) -> Vec<Bounds<Pixels>> {
    let runs = line.unwrapped_layout.runs.iter().enumerate();
    let glyphs =
        runs.flat_map(|(run, shaped)| shaped.glyphs.iter().enumerate().map(move |g| (run, g)));
    let cuts: Vec<(usize, usize)> = line
        .wrap_boundaries()
        .iter()
        .map(|b| (b.run_ix, b.glyph_ix))
        .collect();
    // Each glyph's x, its row with that row's first x, and whether the link holds it.
    let (mut placed, mut row, mut origin) = (Vec::new(), 0, Pixels::ZERO);
    for (run, (nth, glyph)) in glyphs {
        if cuts.get(row) == Some(&(run, nth)) {
            (row, origin) = (row + 1, glyph.position.x);
        }
        let inside = range.contains(&glyph.index);
        placed.push((glyph.position.x, (row, origin), inside));
    }
    // The shaper lists right-to-left text from the right: x gives the visual order.
    placed.sort_by(|a, b| f32::from(a.0).total_cmp(&f32::from(b.0)));
    let (mut out, mut open) = (Vec::new(), None);
    let mut close = |from: Pixels, to: Pixels, (row, origin): (usize, Pixels)| {
        let at = point(from - origin, height * row as f32);
        out.push(Bounds::new(at, size(to - from, height)));
    };
    for (x, row, inside) in placed {
        if let Some((from, on)) = open.filter(|(_, on)| !inside || *on != row) {
            close(from, x, on);
            open = None;
        }
        if inside && open.is_none() {
            open = Some((x, row));
        }
    }
    if let Some((from, on)) = open {
        close(from, line.unwrapped_layout.width, on);
    }
    out
}

fn failed(key: &str, window: &Window, cx: &mut App) {
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

/// The open-with menu; Ely's `Link` stands in for lucide's `link-2`.
fn menu(url: &SharedString, cx: &App) -> Menu {
    let t = cx.global::<Translator>();
    let label = |key: &str| t.t(&format!("chat.markdownRenderer.{key}"), &[]);
    let (out, copied) = (url.clone(), url.clone());
    let sidebar = MenuItem::new(label("openInSidebarBrowser"))
        .icon(IconName::PanelRight)
        .on_click(|window, cx| sidebar(window, cx));
    let browser = MenuItem::new(label("openInDefaultBrowser"))
        .icon(IconName::Globe)
        .keys(label("shiftClick"))
        .on_click(move |window, cx| external(&out, window, cx));
    let copy = MenuItem::new(label("copyLink"))
        .icon(IconName::Link)
        .on_click(move |window, cx| copy(&copied, window, cx));
    Menu::new()
        .item(sidebar)
        .item(browser)
        .separator()
        .item(copy)
}

fn ask(asked: &Entity<Asked>, url: &SharedString, at: Point<Pixels>, cx: &mut App) {
    asked.update(cx, |asked, cx| {
        let number = asked.request.map_or(1, |(number, _)| number + 1);
        *asked = Asked {
            request: Some((number, at)),
            url: url.clone(),
        };
        cx.notify();
    });
}

/// One visual line of a link; the first is its Tab stop.
struct Piece {
    id: String,
    first: bool,
    size: gpui::Size<Pixels>,
    url: SharedString,
    focus: FocusHandle,
    asked: Entity<Asked>,
    /// `linkMenuPosition` for a key: the link's left and bottom.
    under: Point<Pixels>,
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
            under,
        } = self;
        let selector = id.clone();
        let (left, middle, right, keys) = (url.clone(), url.clone(), url.clone(), url);
        let (pressed, menu, by_key) = (focus.clone(), asked.clone(), asked);
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
                            ask(&by_key, &keys, under, cx);
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
                ask(&menu, &right, event.position, cx);
            })
            .into_any_element()
    }
}

/// The links' hit boxes over the text, placed once the text is laid out.
fn overlay(
    key: String,
    links: Vec<Link>,
    layout: TextLayout,
    focus: Vec<FocusHandle>,
    asked: Entity<Asked>,
) -> impl IntoElement {
    let prepaint = move |_, window: &mut Window, cx: &mut App| {
        let text = layout.text();
        let mut drawn = Vec::new();
        for (n, (link, focus)) in links.iter().zip(focus).enumerate() {
            let start = text[..link.range.start].rfind('\n').map_or(0, |at| at + 1);
            let line = layout.line_layout_for_index(link.range.start);
            let line = line.expect("a link lies in a drawn line");
            let origin = layout.position_for_index(start);
            let origin = origin.expect("a drawn line has its origin");
            let within = link.range.start - start..link.range.end - start;
            let boxes = boxes(&line, within, layout.line_height());
            let left = boxes.iter().map(|b| b.left()).reduce(Pixels::min);
            let bottom = boxes.last().map(Bounds::bottom);
            let under = origin + point(left.unwrap_or_default(), bottom.unwrap_or_default());
            for (row, bounds) in boxes.into_iter().enumerate() {
                let piece = Piece {
                    id: format!("{key}-link-{n}-{row}"),
                    first: row == 0,
                    size: bounds.size,
                    url: link.url.clone(),
                    focus: focus.clone(),
                    asked: asked.clone(),
                    under,
                };
                let mut piece = piece.element();
                piece.prepaint_as_root(origin + bounds.origin, bounds.size.into(), window, cx);
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

/// The text with its links underlined, and the menu they share.
pub fn text(
    key: &str,
    text: String,
    links: Vec<Link>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let state = |name: String| ElementId::Name(name.into());
    let asked = window.use_keyed_state(state(format!("{key}-link-asked")), cx, |_, _| {
        Asked::default()
    });
    let stop = |n| {
        let id = state(format!("{key}-link-{n}-focus"));
        let stop = window.use_keyed_state(id, cx, |_, cx| cx.focus_handle().tab_stop(true));
        stop.read(cx).clone()
    };
    let focus: Vec<FocusHandle> = (0..links.len()).map(stop).collect();
    // `underline`; gpui places the line itself, so `underline-offset-2` has no port.
    let underline = HighlightStyle {
        underline: Some(UnderlineStyle {
            thickness: css(1.0).to_pixels(window.rem_size()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let marks = links.iter().map(|link| (link.range.clone(), underline));
    let styled = StyledText::new(text).with_highlights(marks);
    let layout = styled.layout().clone();
    let (request, url) = {
        let asked = asked.read(cx);
        (asked.request, asked.url.clone())
    };
    ContextMenu::new(state(format!("{key}-link-menu")), menu(&url, cx))
        .manual(request)
        .child(styled)
        .child(overlay(key.to_owned(), links, layout, focus, asked))
        .into_any_element()
}

#[cfg(test)]
mod tests;
