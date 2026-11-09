//! `RailNav`'s Pinned tiles: every pinned task as a round tile, with `SessionPreviewCard` on hover.

use std::{collections::HashMap, rc::Rc};

use gpui::{
    AnyElement, App, Bounds, ElementId, Global, HoverListenerMode, InteractiveElement as _,
    IntoElement, ParentElement as _, Pixels, Role, StatefulInteractiveElement as _, Styled as _,
    Window, anchored, deferred, div, point, prelude::FluentBuilder as _,
};

use crate::{
    appearance_settings::text_size,
    i18n::{Arg, Translator},
    router,
    sessions::{Sessions, card, group::Session, open, reorder, row, sortable, title, view_mode},
    theme::Appearance,
    theme_import::color::js_trim,
    units::{css, window_px},
};

const LIST: &str = "rail-pinned";
/// `PREVIEW_WIDTH`.
const PREVIEW_WIDTH: f32 = 208.0;

/// The tile under the pointer with its bounds when entered, and each tile's bounds this frame.
#[derive(Default)]
struct Preview {
    shown: Option<(String, Bounds<Pixels>)>,
    tiles: HashMap<String, Bounds<Pixels>>,
}

impl Global for Preview {}

/// `pinnedTileLabel`: a Latin first word up to 7 characters, else the first 4 UTF-16 units.
pub fn label(title: &str) -> String {
    let trimmed = js_trim(title);
    let word: String = trimmed
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-')
        .collect();
    let latin = word
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric());
    if latin && word.len() >= 2 {
        return word.chars().take(7).collect();
    }
    let units: Vec<u16> = trimmed.encode_utf16().take(4).collect();
    String::from_utf16_lossy(&units)
}

/// The pinned tasks the tiles show, by `manualPinnedOrder` rank, unranked ones last.
pub fn tiles(sessions: &[Session], order: &[String]) -> Vec<Session> {
    let mut pinned: Vec<Session> = sessions
        .iter()
        .filter(|s| s.pinned_at.is_some() && s.status != "archived" && !s.orca_worker)
        .cloned()
        .collect();
    if order.is_empty() {
        return pinned;
    }
    let rank = |s: &Session| {
        order
            .iter()
            .position(|id| *id == s.id)
            .unwrap_or(usize::MAX)
    };
    pinned.sort_by_key(rank);
    pinned
}

/// The tiles in a sortable column, then the divider when there are any.
pub fn section(window: &mut Window, cx: &mut App) -> Vec<AnyElement> {
    if !cx.has_global::<Preview>() {
        cx.set_global(Preview::default());
    }
    let Some(loaded) = cx.global::<Sessions>().loaded.as_ref() else {
        return Vec::new();
    };
    let shown = tiles(&loaded.sessions, &loaded.settings.pinned_order);
    // A tile that left takes its card with it; prepaint records the drawn ones again.
    let preview = cx.global_mut::<Preview>();
    preview.tiles.clear();
    if preview
        .shown
        .as_ref()
        .is_some_and(|(id, _)| !shown.iter().any(|s| s.id == *id))
    {
        preview.shown = None;
    }
    if shown.is_empty() {
        return Vec::new();
    }
    let among = Rc::new(sortable::Among {
        list: LIST,
        ids: Rc::new(shown.iter().map(|s| s.id.clone()).collect()),
        columns: 1,
    });
    let ordered = sortable::arrange(LIST, shown, 1, |s| s.id.clone(), cx);
    let rows = ordered
        .into_iter()
        .flatten()
        .map(|s| {
            let id = s.id.clone();
            let draw: sortable::Draw = {
                let s = s.clone();
                Rc::new(move |window, cx| tile(&s, window, cx))
            };
            let child = tile(&s, window, cx);
            sortable::item(&id, among.clone(), true, child, draw, cx)
        })
        .collect();
    let on_reorder: sortable::OnReorder = Rc::new(|ids, window, cx| {
        log::info!(target: "sessions", "rail pinned reorder of {}", ids.len());
        reorder::reorder(ids, window, cx)
    });
    let list = sortable::list(LIST, rows, on_reorder)
        .flex()
        .flex_none()
        .w_full()
        .flex_col()
        .items_center()
        .gap(css(3.0));
    let border = cx.global::<Appearance>().hsl_token("sidebar-border");
    let divider = div()
        .debug_selector(|| "rail-pin-divider".into())
        .flex_none()
        .my(css(7.0))
        .h(css(1.0))
        .w(css(22.0))
        .bg(border);
    let mut out = vec![list.into_any_element(), divider.into_any_element()];
    out.extend(card_for(cx));
    out
}

/// `handlePinnedClick`: the preview and any panel go; the task shown stays.
fn clicked(id: &str, cx: &mut App) {
    cx.global_mut::<Preview>().shown = None;
    super::sections::close_all(cx);
    if open::is_active(id, cx) {
        return;
    }
    log::info!(target: "sessions", "rail pinned tile {id} opened");
    router::navigate(&open::route(id), cx);
}

/// One tile: the 36px pill with the status glyph, the short label under it.
fn tile(s: &Session, window: &mut Window, cx: &mut App) -> AnyElement {
    let active = open::is_active(&s.id, cx);
    let hovered = window.use_keyed_state(
        ElementId::Name(format!("rail-pin-hover-{}", s.id).into()),
        cx,
        |_, _| false,
    );
    let lit = *hovered.read(cx);
    let t = cx.global::<Translator>();
    let shown = title::display(s, t);
    let appearance = cx.global::<Appearance>();
    let (active_bg, active_border, hover) = (
        appearance.hsl_token("sidebar-item-active"),
        appearance.token("sidebar-item-active-border"),
        appearance.hsl_token("sidebar-item-hover"),
    );
    let (fg, secondary) = (
        appearance.hsl_token("foreground"),
        appearance.token("text-secondary"),
    );
    let size = text_size(10.0, cx);
    let glyph = row::status_glyph(s, Some(15.0), active, cx);
    let pill = div()
        .relative()
        .flex()
        .flex_none()
        .size(css(36.0))
        .items_center()
        .justify_center()
        .rounded(css(9999.0))
        .map(|el| match active {
            true => el
                .bg(active_bg)
                .border(css(1.0))
                .border_color(active_border),
            false => el.when(lit, |el| el.bg(hover)),
        })
        .child(glyph);
    let name = div()
        .max_w(css(56.0))
        .truncate()
        .text_size(css(size))
        .line_height(css(size * 1.2))
        .map(|el| match active {
            true => el.font_weight(gpui::FontWeight::SEMIBOLD).text_color(fg),
            false => el.font_weight(gpui::FontWeight::MEDIUM).text_color(if lit {
                fg
            } else {
                secondary
            }),
        })
        .child(label(&shown));
    let focus = super::tab_stop(&format!("rail-pin-{}", s.id), window, cx);
    let (id, hover_id, key) = (s.id.clone(), s.id.clone(), s.id.clone());
    let record = move |bounds, _: &mut Window, cx: &mut App| {
        cx.global_mut::<Preview>().tiles.insert(key.clone(), bounds);
    };
    div()
        .id(ElementId::Name(format!("rail-pin-{}", s.id).into()))
        .track_focus(&focus)
        .debug_selector({
            let id = s.id.clone();
            move || format!("rail-pin-{id}")
        })
        .role(Role::Button)
        .aria_label(shown)
        .flex()
        .w_full()
        .flex_none()
        .flex_col()
        .items_center()
        .gap(css(2.0))
        .py(css(2.0))
        .cursor_pointer()
        .hover_listener_mode(HoverListenerMode::InputModalityIndependent)
        .on_hover(move |on, window, cx| {
            hovered.update(cx, |h, cx| {
                *h = *on;
                cx.notify();
            });
            hover_preview(&hover_id, *on, window, cx);
        })
        .on_click(move |_, _, cx| clicked(&id, cx))
        .relative()
        .child(gpui::canvas(record, |_, _, _, _| {}).absolute().inset_0())
        .child(pill)
        .child(name)
        .into_any_element()
}

/// `setPreview(null)`, as opening a panel does.
pub fn hide_preview(window: &mut Window, cx: &mut App) {
    if cx
        .try_global::<Preview>()
        .is_some_and(|p| p.shown.is_some())
    {
        cx.global_mut::<Preview>().shown = None;
        window.refresh();
    }
}

/// `onMouseEnter` and `onMouseLeave`: the card follows the tile under the pointer.
fn hover_preview(id: &str, on: bool, window: &mut Window, cx: &mut App) {
    let preview = cx.global_mut::<Preview>();
    match on {
        true => match preview.tiles.get(id) {
            Some(bounds) => preview.shown = Some((id.to_owned(), *bounds)),
            None => return,
        },
        false if preview.shown.as_ref().is_some_and(|(p, _)| p == id) => preview.shown = None,
        false => return,
    }
    window.refresh();
}

/// `SessionPreviewCard` at the tile's right, with no hitbox; running and unread come with 10.0.
fn card_for(cx: &mut App) -> Option<AnyElement> {
    let (id, bounds) = cx.global::<Preview>().shown.clone()?;
    let loaded = cx.global::<Sessions>().loaded.as_ref()?;
    let s = loaded.sessions.iter().find(|s| s.id == id)?.clone();
    let t = cx.global::<Translator>();
    let shown = title::display(&s, t);
    let body = preview_body(&s, cx);
    let now = jiff::Timestamp::now().as_millisecond();
    let tr = |key: &str, n: i64| t.t(key, &[("count", Arg::Number(n))]);
    let when = row::relative(s.updated_at, now, &tr);
    let appearance = cx.global::<Appearance>();
    let (s13, s11, s10) = (
        text_size(13.0, cx),
        text_size(11.0, cx),
        text_size(10.0, cx),
    );
    let card = div()
        .debug_selector(|| "rail-pin-preview".into())
        .w(css(PREVIEW_WIDTH))
        .rounded(css(12.0))
        .border(css(1.0))
        .border_color(appearance.hsl_token("sidebar-border"))
        .bg(appearance.token("surface-elevated"))
        .px(css(12.0))
        .pt(css(10.0))
        .pb(css(11.0))
        .child(
            div()
                .text_size(css(s13))
                .line_height(css(s13 * 1.3))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(appearance.hsl_token("foreground"))
                .child(shown),
        )
        .when_some(body, |el, body| {
            el.child(
                div()
                    .mt(css(4.0))
                    .text_size(css(s11))
                    .line_height(css(s11 * 1.45))
                    .line_clamp(4)
                    .text_color(appearance.token("text-secondary"))
                    .child(body),
            )
        })
        .child(
            div()
                .mt(css(7.0))
                .text_size(css(s10))
                .line_height(css(s10))
                .text_color(appearance.token("text-tertiary"))
                .child(when),
        );
    let at = point(
        bounds.right() + window_px(10.0, cx),
        bounds.top() - window_px(4.0, cx),
    );
    Some(
        deferred(
            anchored()
                .position(at)
                .snap_to_window_with_margin(window_px(8.0, cx))
                .child(card),
        )
        .with_priority(1)
        .into_any_element(),
    )
}

/// `resolveSessionCardBody` with the Pinned section's mode as the variant.
fn preview_body(s: &Session, cx: &App) -> Option<String> {
    let body = match view_mode::pinned(cx) {
        view_mode::Pinned::Card => card::body(s),
        _ => s.preview.as_deref().map(js_trim).filter(|p| !p.is_empty()),
    };
    body.map(str::to_owned)
}

#[cfg(test)]
pub(crate) mod tests;
