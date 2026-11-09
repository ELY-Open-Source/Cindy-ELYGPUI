//! `RailPanels`: `RailPanelShell`, the Dialogues panel and the closes both panels share.

use std::cmp::Ordering;

use gpui::{
    AnyElement, App, Bounds, DispatchPhase, FocusHandle, Hitbox, HitboxBehavior,
    InteractiveElement as _, IntoElement, MouseDownEvent, MouseMoveEvent, ParentElement as _,
    Pixels, Point, StatefulInteractiveElement as _, Styled as _, Window, anchored, canvas,
    deferred, div, point,
};

use super::sections::{self, Panels, Section};
use crate::{
    appearance_settings::text_size,
    i18n::{Arg, Translator},
    localdb::search::merge::COLLATOR,
    preferences::Preferences,
    sessions::{
        Sessions,
        fold::{self, ShowAll},
        group::Session,
        main_list::{lists, text},
        row::{Scope, row},
    },
    theme::Appearance,
    units::{css, window_px},
};

const DIALOGUE_LIMIT_KEY: &str = "sidebar.collapse.dialogueLimit";
/// `w-[264px]`.
const WIDTH: f32 = 264.0;

/// Each level's hitbox and focus this frame, and the project rows', for the listeners.
#[derive(Default)]
struct Shell {
    hitbox: [Option<Hitbox>; 2],
    focus: [Option<FocusHandle>; 2],
    /// Each project row's key, bounds and hitbox.
    rows: Vec<(String, Bounds<Pixels>, Hitbox)>,
}

impl gpui::Global for Shell {}

/// `panelHasEditingFocus` or `panelHasBlockingOverlay`: a panel row's field or menu holds focus.
pub(super) fn suppressed(window: &Window, cx: &App) -> bool {
    let editing = cx
        .global::<Sessions>()
        .editing
        .as_ref()
        .is_some_and(|e| e.scope == Scope::Rail);
    // Ely's menus hold a focus outside the tab order while open; rows and More are tab stops.
    let focus = cx.try_global::<Shell>().map(|s| s.focus.clone());
    let menu = focus.into_iter().flatten().flatten().any(|panel| {
        panel.contains_focused(window, cx)
            && window
                .focused(cx)
                .is_some_and(|f| !f.tab_stop && f != panel)
    });
    editing || menu
}

/// `compareDialogueSessions` with `recency`, the only order the rail's panel is given.
fn by_recency(a: &Session, b: &Session) -> Ordering {
    let rank = |s: &Session| match s.status.as_str() {
        "active" => 0,
        "archived" => 1,
        _ => 2,
    };
    rank(a)
        .cmp(&rank(b))
        .then(b.activity_at.cmp(&a.activity_at))
        .then_with(|| COLLATOR.compare(&a.id, &b.id))
}

/// The open panels deferred over the content, and the listeners they share.
pub fn panel(window: &mut Window, cx: &mut App) -> Vec<AnyElement> {
    cx.set_global(Shell::default());
    let p = cx.default_global::<Panels>();
    let (Some(section), Some(anchor)) = (p.open, p.anchor) else {
        return Vec::new();
    };
    let watch = canvas(|_, _, _| {}, |_, _, window, _| listen(window))
        .absolute()
        .size_0()
        .into_any_element();
    let body = match section {
        Section::Dialogues => dialogues(cx),
        Section::Projects => projects::body(window, cx),
    };
    let mut out = vec![watch, shell(0, anchor, body, window, cx)];
    if let Some((anchor, body)) = level::body(cx) {
        out.push(shell(1, anchor, body, window, cx));
    }
    out
}

/// `RailPanelShell` at `level` 0 or 1, beside `anchor`.
fn shell(
    level: usize,
    anchor: Bounds<Pixels>,
    body: AnyElement,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let (id, gap) = match level {
        0 => ("rail-panel", 12.0),
        _ => ("rail-project-panel", 8.0),
    };
    let focus = window
        .use_keyed_state(format!("{id}-focus"), cx, |_, cx| cx.focus_handle())
        .read(cx)
        .clone();
    cx.default_global::<Shell>().focus[level] = Some(focus.clone());
    focus_in(&focus, level, window, cx);
    let appearance = cx.global::<Appearance>();
    let shell = div()
        .id(id)
        .debug_selector(move || id.into())
        .track_focus(&focus)
        .occlude()
        .w(css(WIDTH))
        .rounded(css(12.0))
        .border(css(1.0))
        .border_color(appearance.hsl_token("sidebar-border"))
        .bg(appearance.token("surface-elevated"))
        .p(css(6.0))
        .shadow(vec![appearance.shadow_token("shadow-menu", cx)])
        .child(
            canvas(
                move |bounds, window, cx| {
                    let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                    cx.default_global::<Shell>().hitbox[level] = Some(hitbox);
                },
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0(),
        )
        .child(body);
    let at = point(
        anchor.right() + window_px(gap, cx),
        anchor.top() - window_px(6.0, cx),
    );
    deferred(
        anchored()
            .position(at)
            .snap_to_window_with_margin(window_px(8.0, cx))
            .child(shell),
    )
    // `z-[48]`, then `z-[49]` for the project level.
    .with_priority(1 + level)
    .into_any_element()
}

/// A keyboard open moves focus to the level's first stop, once; with none it stays put.
fn focus_in(panel: &FocusHandle, level: usize, window: &mut Window, cx: &mut App) {
    let p = cx.global_mut::<Panels>();
    let flag = match level {
        0 => p.via_keyboard.then_some(&mut p.focused_in),
        _ => p
            .project
            .as_mut()
            .filter(|o| o.via_keyboard)
            .map(|o| &mut o.focused_in),
    };
    let Some(flag) = flag.filter(|f| !**f) else {
        return;
    };
    *flag = true;
    let panel = panel.clone();
    window.defer(cx, move |window, cx| {
        let before = window.focused(cx);
        window.focus(&panel, cx);
        window.focus_next(cx);
        let inside = panel.contains_focused(window, cx) && !panel.is_focused(window);
        log::info!(target: "sidebar", "rail panel {level}: focus moved in {inside}");
        if !inside && let Some(before) = before {
            window.focus(&before, cx);
        }
    });
}

/// `panelHead`: the title and its count.
fn head(title: String, count: usize, cx: &App) -> AnyElement {
    let appearance = cx.global::<Appearance>();
    let (s12, s10) = (text_size(12.0, cx), text_size(10.0, cx));
    let count = cx.global::<Translator>().t(
        "ccAgent.sidebar.railNavCount",
        &[("count", Arg::Number(count as i64))],
    );
    div()
        .flex()
        .items_baseline()
        .gap(css(6.0))
        .px(css(10.0))
        .pb(css(4.0))
        .pt(css(6.0))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .truncate()
                .text_size(css(s12))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(appearance.hsl_token("foreground"))
                .child(title),
        )
        .child(
            div()
                .flex_none()
                .text_size(css(s10))
                .text_color(appearance.token("text-tertiary"))
                .child(count),
        )
        .into_any_element()
}

/// The Dialogues panel: its head, then `SessionEntryList`; the head's New comes with 8.10.
fn dialogues(cx: &mut App) -> AnyElement {
    let mut sessions = lists(cx).map(|l| l.dialogues).unwrap_or_default();
    sessions.sort_by(by_recency);
    let limit = fold::limit(text(cx.global::<Preferences>(), DIALOGUE_LIMIT_KEY), 10);
    let now = jiff::Timestamp::now().as_millisecond();
    let expanded = cx.global::<Panels>().show_all;
    let since = Some(fold::recent_since(now));
    let (visible, hidden) = fold::view(&sessions, limit, expanded, since, |s| s.activity_at);
    let title = cx
        .global::<Translator>()
        .t("ccAgent.sidebar.railNav.dialogues", &[]);
    let more = ShowAll::new("rail-panel-show-all".into(), sessions.len(), |cx| {
        cx.global_mut::<Panels>().show_all = true
    });
    let rows = entry_list(&visible, hidden, more, now, cx);
    div()
        .flex()
        .flex_col()
        .child(head(title, sessions.len(), cx))
        .child(scroll(rows))
        .into_any_element()
}

/// `SessionEntryList` in a panel: its rows, then Show all while some are folded.
fn entry_list(
    visible: &[&Session],
    hidden: usize,
    more: ShowAll,
    now: i64,
    cx: &mut App,
) -> Vec<AnyElement> {
    let mut rows: Vec<AnyElement> = visible
        .iter()
        .map(|s| row(s, false, None, now, Scope::Rail, cx))
        .collect();
    if hidden > 0 {
        rows.push(more.into_any_element());
    }
    rows
}

/// `max-h-[420px] overflow-y-auto`, a block box: rows keep their height and overflow.
fn scroll(children: Vec<AnyElement>) -> AnyElement {
    div()
        .id("rail-panel-list")
        .flex()
        .flex_col()
        .max_h(css(420.0))
        .overflow_y_scroll()
        .child(div().flex_none().flex().flex_col().children(children))
        .into_any_element()
}

/// Whether `at` hits the level's panel.
fn on_level(level: usize, at: Point<Pixels>, window: &Window, cx: &App) -> bool {
    cx.try_global::<Shell>()
        .and_then(|s| s.hitbox[level].as_ref())
        .is_some_and(|h| h.is_hovered_at(at, window))
}

/// Whether `at` falls on a panel or a rail tile: `RAIL_PANEL_KEEPALIVE_SELECTOR`.
pub(super) fn inside(at: Point<Pixels>, window: &Window, cx: &App) -> bool {
    on_level(0, at, window, cx) || on_level(1, at, window, cx) || sections::on_tile(at, window, cx)
}

/// The pointer keep-alive and a press outside, while the panel is open.
fn listen(window: &mut Window) {
    window.on_mouse_event(|e: &MouseMoveEvent, phase, window, cx| {
        if phase != DispatchPhase::Capture {
            return;
        }
        match inside(e.position, window, cx) {
            true => sections::cancel_close(cx),
            false => sections::schedule_close(window, cx),
        }
        level::pointer(e.position, window, cx);
    });
    window.on_mouse_event(|e: &MouseDownEvent, phase, window, cx| {
        if phase != DispatchPhase::Capture || inside(e.position, window, cx) {
            return;
        }
        if !suppressed(window, cx) {
            log::info!(target: "sidebar", "rail panel closed by a press outside");
            return sections::close_all(window, cx);
        }
        // A press on a row's open menu stays inside, as the popper wrapper does.
        cx.global_mut::<Panels>().chosen = false;
        window.defer(cx, |window, cx| {
            if !cx.global::<Panels>().chosen && !suppressed(window, cx) {
                log::info!(target: "sidebar", "rail panel closed by a press past its menu");
                sections::close_all(window, cx);
            }
        });
    });
}

/// Escape wherever focus is, while a panel is open; the field or a row's menu takes it first.
pub(super) fn escape(cx: &mut App) -> gpui::Subscription {
    cx.intercept_keystrokes(|e, window, cx| {
        let open = cx.try_global::<Panels>().is_some_and(|p| p.open.is_some());
        if open && e.keystroke.key == "escape" && !suppressed(window, cx) {
            log::info!(target: "sidebar", "rail panel closed by Escape");
            sections::close_all(window, cx);
        }
    })
}

mod level;
mod projects;
#[cfg(test)]
mod tests;
