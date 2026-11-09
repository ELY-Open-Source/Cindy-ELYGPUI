//! The list's scroll under the search: top on each search, the list's place back once the query
//! clears, after the pointer lifts, as `CCAgentSidebarUpper`'s layout effect does.

use gpui::{
    App, DispatchPhase, IntoElement, MouseDownEvent, MouseUpEvent, Pixels, Point, ScrollHandle,
    Styled as _, canvas, point, px,
};

use super::{Search, run};

#[derive(Default)]
pub struct Scroll {
    /// `lastListScrollTopRef`: the list's offset before the search.
    last: Point<Pixels>,
    /// The options as the effect keys them, with `searchProjectKey`.
    key: Option<(run::Key, Option<Vec<String>>)>,
    was_active: bool,
    down: bool,
    pending: bool,
}

/// The effect over the search's dependencies; the list's offset is kept while it shows.
pub fn sync(handle: &ScrollHandle, cx: &mut App) {
    let key = (run::key(cx), cx.global::<Search>().options.projects.clone());
    let active = !key.0.query.is_empty();
    let s = &mut cx.global_mut::<Search>().scroll;
    if s.key.as_ref() != Some(&key) {
        if active {
            s.pending = false;
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
    if !active && !s.pending {
        s.last = handle.offset();
    }
}

fn restore(s: &mut Scroll, handle: &ScrollHandle) {
    s.pending = false;
    handle.set_offset(s.last);
    log::debug!(target: "search", "list scroll restored to {:?}", s.last.y);
}

/// The window's pointer listeners; a held restore runs after the pointer lifts.
pub fn pointer(handle: ScrollHandle) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |_, _, window, _| {
            window.on_mouse_event(|_: &MouseDownEvent, phase, _, cx| {
                if phase == DispatchPhase::Capture {
                    cx.global_mut::<Search>().scroll.down = true;
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

#[cfg(test)]
mod tests;
