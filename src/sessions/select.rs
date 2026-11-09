//! Bulk selection as `handleSessionClick` keeps it: Shift spans the rows drawn, Mod or Ctrl
//! toggles, a plain click clears. Rows that leave the list leave the selection.

use std::collections::HashSet;

use ely_gpui_component::primitives::IconName;
use gpui::{
    AnyElement, App, Global, InteractiveElement as _, IntoElement, Modifiers, ParentElement as _,
    Role, StatefulInteractiveElement as _, Styled as _, Window, canvas, div,
};

use super::Sessions;
use crate::{
    appearance_settings::text_size,
    i18n::{Arg, Translator},
    lucide::icon,
    router::{self, Route, Router},
    theme::Appearance,
    tip::tip,
    units::{css, window_px},
};

/// `selectedSessionIds` and `selectionAnchorSessionId`.
#[derive(Default)]
pub struct State {
    pub ids: HashSet<String>,
    anchor: Option<String>,
}

/// The rows the last frame drew, in paint order, each with its card slot's rank.
#[derive(Default)]
pub struct Drawn {
    rows: Vec<(Option<usize>, String)>,
    rank: Option<usize>,
}

impl Global for Drawn {}

/// Landing on New or Scheduled clears it, as `onNewMakerMatch` and `onScheduleMatch` do.
pub fn install(cx: &mut App) {
    cx.set_global(Drawn::default());
    cx.observe_global::<Router>(|cx| {
        if matches!(
            router::route(router::path(cx)),
            Route::New | Route::Scheduled
        ) {
            clear(cx);
        }
    })
    .detach();
}

/// `hasSessionSelectionModifier`.
pub fn modified(m: &Modifiers) -> bool {
    m.shift || m.platform || m.control
}

pub fn is_selected(id: &str, cx: &App) -> bool {
    cx.global::<Sessions>().selection.ids.contains(id)
}

/// `getVisibleSidebarSessionIds`: ranked card slots first, row-major, then paint order.
fn visible(cx: &App) -> Vec<String> {
    let mut rows = cx.global::<Drawn>().rows.clone();
    rows.sort_by_key(|(rank, _)| rank.map_or((1, 0), |r| (0, r)));
    let mut seen = HashSet::new();
    rows.into_iter()
        .filter_map(|(_, id)| seen.insert(id.clone()).then_some(id))
        .collect()
}

/// A Shift, Mod or Ctrl click.
pub fn click(id: &str, m: &Modifiers, cx: &mut App) {
    let state = &mut cx.global_mut::<Sessions>().selection;
    if !m.shift {
        if !state.ids.remove(id) {
            state.ids.insert(id.to_owned());
        }
        state.anchor = Some(id.to_owned());
        log::info!(target: "sessions", "selection toggled {id}: {} selected", state.ids.len());
        return cx.refresh_windows();
    }
    let shown = visible(cx);
    let state = &mut cx.global_mut::<Sessions>().selection;
    let anchor = state
        .anchor
        .as_deref()
        .filter(|a| shown.iter().any(|s| s == a))
        .unwrap_or(id);
    let at = |x: &str| shown.iter().position(|s| s == x);
    let range: Vec<String> = match (at(anchor), at(id)) {
        (Some(a), Some(t)) => shown[a.min(t)..=a.max(t)].to_vec(),
        _ => vec![id.to_owned()],
    };
    let mut next = match m.platform || m.control {
        true => state.ids.clone(),
        false => HashSet::new(),
    };
    next.extend(range.into_iter().filter(|r| shown.contains(r)));
    state.ids = next;
    state.anchor.get_or_insert_with(|| id.to_owned());
    log::info!(target: "sessions", "selection range to {id}: {} selected", state.ids.len());
    cx.refresh_windows();
}

/// A plain click: the selection goes, the task becomes the anchor.
pub fn plain(id: &str, cx: &mut App) {
    let state = &mut cx.global_mut::<Sessions>().selection;
    if !state.ids.is_empty() {
        state.ids.clear();
        log::info!(target: "sessions", "selection cleared by a click on {id}");
        cx.refresh_windows();
    }
    cx.global_mut::<Sessions>().selection.anchor = Some(id.to_owned());
}

/// `handleClearSelection`.
pub fn clear(cx: &mut App) {
    let state = &mut cx.global_mut::<Sessions>().selection;
    if state.ids.is_empty() && state.anchor.is_none() {
        return;
    }
    log::info!(target: "sessions", "selection cleared: {} were selected", state.ids.len());
    *state = State::default();
    cx.refresh_windows();
}

/// A new frame of rows begins.
pub fn begin(cx: &mut App) {
    *cx.global_mut::<Drawn>() = Drawn::default();
}

/// A row painted this frame.
pub fn drawn(id: &str, cx: &mut App) {
    let drawn = cx.global_mut::<Drawn>();
    let rank = drawn.rank;
    drawn.rows.push((rank, id.to_owned()));
}

/// `data-sidebar-row-order` for the rows painted after it, until the next mark.
pub fn rank(rank: Option<usize>) -> impl IntoElement {
    canvas(
        move |_, _, cx| cx.global_mut::<Drawn>().rank = rank,
        |_, _, _, _| {},
    )
    .absolute()
}

/// The list's end: `pruneSelectionToRenderedRows`, while anything is selected.
pub fn end() -> impl IntoElement {
    canvas(|_, _, cx| prune(cx), |_, _, _, _| {}).absolute()
}

fn prune(cx: &mut App) {
    if cx.global::<Sessions>().selection.ids.is_empty() {
        return;
    }
    let shown: HashSet<String> = visible(cx).into_iter().collect();
    let state = &mut cx.global_mut::<Sessions>().selection;
    let before = state.ids.len();
    state.ids.retain(|id| shown.contains(id));
    let anchor_gone = state.anchor.as_ref().is_some_and(|a| !shown.contains(a));
    if anchor_gone {
        state.anchor = None;
    }
    if state.ids.len() < before || anchor_gone {
        log::info!(target: "sessions", "selection pruned to {} drawn", state.ids.len());
        cx.refresh_windows();
    }
}

#[cfg(test)]
mod tests;

/// The bulk bar while anything is selected: the count and Clear. Archive and Delete come with
/// 18.0, as the row's do.
pub fn bar(window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let count = cx.global::<Sessions>().selection.ids.len();
    if count == 0 {
        return None;
    }
    let translator = cx.global::<Translator>();
    let text = translator.t(
        "ccAgent.sidebar.bulkSelection.selected",
        &[("count", Arg::Number(count as i64))],
    );
    let label = translator.t("ccAgent.sidebar.bulkSelection.clear", &[]);
    let focus = window
        .use_keyed_state("bulk-clear-focus", cx, |_, cx| {
            cx.focus_handle().tab_stop(true)
        })
        .read(cx)
        .clone();
    let appearance = cx.global::<Appearance>();
    let (meta, strong, hover) = (
        appearance.token("cmd-palette-item-meta"),
        appearance.token("msg-assistant-text"),
        appearance.token("cmd-palette-item-hover"),
    );
    // `focus-visible:ring-2`.
    let ring = gpui::BoxShadow {
        color: appearance.token("focus-ring-soft"),
        offset: gpui::point(gpui::px(0.0), gpui::px(0.0)),
        blur_radius: gpui::px(0.0),
        spread_radius: window_px(2.0, cx),
        inset: false,
    };
    let clear_button = div()
        .id("bulk-clear")
        .debug_selector(|| "bulk-clear".into())
        .track_focus(&focus)
        .role(Role::Button)
        .aria_label(label.clone())
        .group("bulk-clear")
        .flex()
        .flex_none()
        .size(css(24.0))
        .items_center()
        .justify_center()
        .rounded(css(9999.0))
        .cursor_pointer()
        .hover(|s| s.bg(hover))
        .focus_visible(|s| s.shadow(vec![ring]))
        .tooltip(tip(label))
        .on_click(|_, _, cx| clear(cx))
        .child(
            icon(IconName::X.svg(), 13.0, 2.0)
                .text_color(meta)
                .group_hover("bulk-clear", |s| s.text_color(strong)),
        );
    let pill = div()
        .flex()
        .h(css(32.0))
        .items_center()
        .gap(css(4.0))
        .rounded(css(9999.0))
        .px(css(8.0))
        .pl(css(12.0))
        .bg(appearance.token("chat-input-chip-bg"))
        .text_color(strong)
        .border(css(1.0))
        .border_color(appearance.token("cmd-palette-border"))
        .child(
            div()
                .debug_selector(|| "bulk-count".into())
                .min_w_0()
                .flex_1()
                .truncate()
                .text_size(css(text_size(12.0, cx)))
                .line_height(css(16.0))
                .font_weight(gpui::FontWeight::MEDIUM)
                .child(text),
        )
        .child(clear_button);
    Some(
        div()
            .flex_none()
            .px(css(12.0))
            .pb(css(8.0))
            .child(pill)
            .into_any_element(),
    )
}
