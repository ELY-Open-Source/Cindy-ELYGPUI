//! The list's scroll under the search, as `CCAgentSidebarUpper`'s effects keep it.

use gpui::{
    App, DispatchPhase, FocusHandle, IntoElement, MouseDownEvent, MouseUpEvent, Pixels, Point,
    ScrollHandle, Styled as _, Window, canvas, point, px,
};

use super::{Search, Which, menu_open, past_menu, run, trimmed};

#[derive(Default)]
pub struct Scroll {
    /// `lastListScrollTopRef`: the list's offset before the search.
    last: Point<Pixels>,
    /// The options as the effect keys them, with `searchProjectKey`.
    key: Option<(run::Key, Option<Vec<String>>)>,
    was_active: bool,
    down: bool,
    pending: bool,
    /// `freezeListScrollOnOpenRef`, and the scroll to the top it still owes.
    frozen: bool,
    to_top: bool,
    /// The focus the row last saw, for `focusin`.
    focus: Option<FocusHandle>,
    handle: Option<ScrollHandle>,
}

/// `openSignal` moved with no search running: the list freezes and goes to the top.
pub(super) fn freeze(cx: &mut App) {
    if !trimmed(Which::Inline, cx).is_empty() {
        return;
    }
    let s = &mut cx.global_mut::<Search>().scroll;
    (s.frozen, s.to_top) = (true, true);
    log::debug!(target: "search", "list scroll frozen");
}

/// `onFocusIn` outside the search: a Tab stop took focus, so the frozen list goes back.
pub(super) fn focused(within: bool, window: &Window, cx: &mut App) {
    let now = window.focused(cx).filter(|f| f.tab_stop);
    let s = &mut cx.global_mut::<Search>().scroll;
    if s.focus == now {
        return;
    }
    let landed = now.is_some();
    s.focus = now;
    if within || !landed || !s.frozen || s.down {
        return;
    }
    if let Some(handle) = s.handle.clone() {
        log::debug!(target: "search", "focus left the search");
        restore(s, &handle);
    }
}

/// The effect over the search's dependencies; the list's offset is kept while it shows.
pub fn sync(handle: &ScrollHandle, cx: &mut App) {
    let inline = &super::of(Which::Inline, cx).options;
    let key = (run::key(Which::Inline, cx), inline.projects.clone());
    let active = !key.0.query.is_empty();
    let s = &mut cx.global_mut::<Search>().scroll;
    s.handle = Some(handle.clone());
    if s.key.as_ref() != Some(&key) {
        if active {
            (s.frozen, s.pending) = (false, false);
            handle.set_offset(point(handle.offset().x, px(0.0)));
            log::debug!(target: "search", "list scrolled to the top");
        } else if s.was_active {
            match s.down {
                true => s.pending = true,
                false => restore(s, handle),
            }
        }
        s.was_active = active;
        s.key = Some(key);
    }
    if s.frozen && std::mem::take(&mut s.to_top) {
        handle.set_offset(point(handle.offset().x, px(0.0)));
    }
    if !active && !s.pending && !s.frozen {
        s.last = handle.offset();
    }
}

fn restore(s: &mut Scroll, handle: &ScrollHandle) {
    (s.pending, s.frozen) = (false, false);
    handle.set_offset(s.last);
    log::debug!(target: "search", "list scroll restored to {:?}", s.last.y);
}

/// The window's pointer listeners; a held restore runs after the pointer lifts.
pub fn pointer(handle: ScrollHandle) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |_, _, window, _| {
            window.on_mouse_event(|e: &MouseDownEvent, phase, window, cx| {
                if phase != DispatchPhase::Capture {
                    return;
                }
                cx.global_mut::<Search>().scroll.down = true;
                if cx.global::<Search>().scroll.frozen {
                    pressed(e, window, cx);
                }
            });
            window.on_mouse_event(move |_: &MouseUpEvent, phase, window, cx| {
                if phase != DispatchPhase::Capture {
                    return;
                }
                let s = &mut cx.global_mut::<Search>().scroll;
                s.down = false;
                if !std::mem::take(&mut s.pending) {
                    return;
                }
                let handle = handle.clone();
                window.defer(cx, move |window, cx| {
                    restore(&mut cx.global_mut::<Search>().scroll, &handle);
                    window.refresh();
                });
            });
        },
    )
    .absolute()
}

/// `isOutsideSearch`: a press past the row and its poppers restores once the pointer lifts.
fn pressed(e: &MouseDownEvent, window: &mut Window, cx: &mut App) {
    let search = cx.global::<Search>();
    let row = search.row.clone();
    let inside = row
        .as_ref()
        .is_some_and(|(hitbox, ..)| hitbox.is_hovered_at(e.position, window))
        || crate::sidebar::top_nav::more::over_menu(e.position, window, cx)
        || crate::sessions::rail::search::over(e.position, window, cx);
    if inside {
        return;
    }
    let menus = row.map(|(_, menu, filter)| (menu, filter));
    let open = menus
        .into_iter()
        .chain(crate::sessions::rail::search::menu(cx))
        .find(|(m, f)| menu_open(m, f, window, cx));
    let Some(menu) = open else {
        return hold(cx);
    };
    // The open menu takes the press first: a row it chose is inside.
    past_menu(menu, window, cx, |_, cx| hold(cx));
}

fn hold(cx: &mut App) {
    log::debug!(target: "search", "a press outside the search");
    cx.global_mut::<Search>().scroll.pending = true;
}

#[cfg(test)]
mod tests;
