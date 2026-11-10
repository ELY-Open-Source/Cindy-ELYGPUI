//! `SessionContentHeader` in the content header's slot.

use ely_gpui_component::{menus::menu_under, primitives::IconName};
use gpui::{
    AnyElement, App, DispatchPhase, ElementId, InteractiveElement as _, IntoElement as _,
    MouseButton, MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, Point, Role,
    StatefulInteractiveElement as _, Styled as _, Window, canvas, div,
};

use crate::{
    appearance_settings::text_size,
    i18n::Translator,
    lucide::icon,
    sessions::{self, group::Session, rename, row::Scope, task_menu, title},
    theme::Appearance,
    tip::tip,
    units::{self, css},
};

/// `MANUAL_DRAG_DEAD_ZONE_PX`.
const DRAG_DEAD_ZONE: f32 = 4.0;

/// The task the header shows: the `active` bucket's row, else the content's.
pub fn task(cx: &App) -> Option<&Session> {
    let content = super::session(cx)?;
    let listed = sessions::find(&content.id, cx).filter(|s| s.status == "active");
    Some(listed.unwrap_or(content))
}

/// `flex min-w-0 items-center gap-0.5 pl-1`: the pin, the title or its field, then More.
pub fn element(window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let Some(s) = task(cx).cloned() else {
        // The header unmounts with its task, and its edit with it.
        rename::drop_header(cx);
        return None;
    };
    let meta = cx.global::<Appearance>().token("cmd-palette-item-meta");
    let pin = s
        .pinned_at
        .is_some()
        .then(|| {
            icon(IconName::Pin.svg(), 13.0, 2.0)
                .mr(css(4.0))
                .text_color(meta)
        })
        .map(|pin| pin.debug_selector(|| "session-header-pin".into()));
    // `isEditing` outlives a task switch: the field stays, whatever task opened it.
    let (title, more) = match rename::header_editing(cx) {
        Some(editing) => (rename::header_field(editing, window, cx), None),
        None => (title(&s, window, cx), Some(more(&s, window, cx))),
    };
    let root = div()
        .debug_selector(|| "session-header".into())
        .flex()
        .min_w_0()
        .items_center()
        .gap(css(2.0))
        .pl(css(4.0))
        .children(pin)
        .child(title)
        .children(more);
    Some(root.into_any_element())
}

/// The title: a double click renames; a press that moves past the dead zone drags the window.
fn title(s: &Session, window: &mut Window, cx: &mut App) -> AnyElement {
    let text = title::header(s, cx.global::<Translator>());
    let fg = cx.global::<Appearance>().hsl_token("foreground");
    // `useManualWindowDrag`: where the press began, until it moves the window or lifts.
    let press = window.use_keyed_state("session-header-title-press", cx, |_, _| None);
    let (down, held) = (press.clone(), press);
    div()
        .id("session-header-title")
        .debug_selector(|| "session-header-title".into())
        .min_w_0()
        .max_w(window.viewport_size().width * 0.4)
        .truncate()
        .cursor_default()
        .text_size(css(text_size(14.0, cx)))
        .line_height(css(text_size(20.0, cx)))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(fg)
        .tooltip(tip(text.clone()))
        .on_mouse_down(MouseButton::Left, move |event, window, cx| {
            // `WINDOW_NO_DRAG_STYLE`: the header's own drag stays out.
            window.prevent_default();
            down.update(cx, |p: &mut Option<Point<Pixels>>, _| {
                *p = Some(event.position)
            });
        })
        .on_click(move |event, window, cx| {
            // A double click renames and never zooms the window.
            cx.stop_propagation();
            if event.click_count() != 2 {
                return;
            }
            match task(cx).cloned() {
                Some(s) => rename::start(&s, Scope::Header, window, cx),
                None => log::warn!(target: "sessionHeader", "rename: the header holds no task"),
            }
        })
        .child(text)
        .child(canvas(|_, _, _| {}, move |_, _, window, _| capture(held, window)).absolute())
        .into_any_element()
}

/// `setPointerCapture`: the press is followed over the whole window until it moves it or lifts.
fn capture(press: gpui::Entity<Option<Point<Pixels>>>, window: &mut Window) {
    let moved = press.clone();
    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
        let Some(start) = *moved.read(cx) else { return };
        if phase != DispatchPhase::Capture {
            return;
        }
        if event.pressed_button != Some(MouseButton::Left) {
            // The release never came (`pointercancel`).
            return moved.update(cx, |p, _| *p = None);
        }
        let delta = event.position - start;
        let (x, y) = (units::to_css(delta.x, cx), units::to_css(delta.y, cx));
        if x.hypot(y) < DRAG_DEAD_ZONE {
            return;
        }
        moved.update(cx, |p, _| *p = None);
        log::debug!(target: "sessionHeader", "title press moves the window");
        window.start_window_move();
    });
    window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
        if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
            press.update(cx, |p, _| *p = None);
        }
    });
}

/// More: `h-7 w-7 rounded-md`, `Ellipsis` 15, opening `SessionTaskMenu`.
fn more(s: &Session, window: &mut Window, cx: &mut App) -> AnyElement {
    let appearance = cx.global::<Appearance>();
    let (meta, hover_bg, hover_fg) = (
        appearance.token("cmd-palette-item-meta"),
        appearance.hsl_token("titlebar-button-hover"),
        appearance.hsl_token("foreground"),
    );
    let t = cx.global::<Translator>();
    let label = t.t("ccAgent.sessionHeader.moreActions", &[]);
    let menu = task_menu::menu(s, Scope::Header, t);
    let id = ElementId::Name(format!("session-header-menu-{}", s.id).into());
    // A native button: a Tab stop that a press focuses.
    let focus = window
        .use_keyed_state("session-header-more-focus", cx, |_, cx| {
            cx.focus_handle().tab_stop(true)
        })
        .read(cx)
        .clone();
    let pressed = focus.clone();
    menu_under(
        id,
        &menu,
        move |click| {
            div()
                .id("session-header-more")
                .debug_selector(|| "session-header-more".into())
                .track_focus(&focus)
                .role(Role::Button)
                .aria_label(label.clone())
                .group("session-header-more")
                .flex()
                .flex_none()
                .size(css(28.0))
                .items_center()
                .justify_center()
                .rounded(css(6.0))
                .hover(|st| st.bg(hover_bg))
                .tooltip(tip(label))
                // `WINDOW_NO_DRAG_STYLE`: the press focuses the button and stops there.
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    window.focus(&pressed, cx);
                    cx.stop_propagation();
                })
                .on_click(move |event, window, cx| {
                    cx.stop_propagation();
                    click(event, window, cx);
                })
                .child(
                    icon(IconName::Ellipsis.svg(), 15.0, 2.0)
                        .text_color(meta)
                        .group_hover("session-header-more", |st| st.text_color(hover_fg)),
                )
                .into_any_element()
        },
        window,
        cx,
    )
    .into_any_element()
}

#[cfg(test)]
mod tests;
