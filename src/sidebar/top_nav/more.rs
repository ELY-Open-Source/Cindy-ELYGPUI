//! `SidebarNavigationMoreMenu`; under the defaults it holds Customize alone.

use ely_gpui_component::{
    forms::surface,
    primitives::{FocusNext, FocusPrev, IconName},
    theme::ActiveTheme as _,
};
use gpui::{
    AnyElement, App, Bounds, ClickEvent, DispatchPhase, FocusHandle, Global, Hitbox,
    HitboxBehavior, HoverListenerMode, InteractiveElement as _, IntoElement, MouseButton,
    MouseDownEvent, ParentElement as _, Pixels, Point, Role, StatefulInteractiveElement as _,
    Styled as _, Window, anchored, canvas, deferred, div, point, prelude::FluentBuilder as _,
};

use super::customize::Draft;
use crate::{
    appearance_settings::text_size,
    i18n::Translator,
    lucide::icon,
    sidebar::hover_menu::{self, HoverMenu},
    theme::Appearance,
    units::{css, window_px},
};

#[derive(Default)]
pub struct More {
    menu: HoverMenu,
    /// Customize's draft while it replaces the rows.
    pub customize: Option<Draft>,
    trigger: Option<Bounds<Pixels>>,
    /// The open menu's hitbox this frame, part of the search's popper exception.
    panel: Option<Hitbox>,
    /// Escape while the menu is open, wherever focus is.
    _escape: Option<gpui::Subscription>,
}

impl Global for More {}

fn menu(cx: &mut App) -> &mut HoverMenu {
    &mut cx.global_mut::<More>().menu
}

struct Focus {
    trigger: FocusHandle,
    content: FocusHandle,
    item: FocusHandle,
}

fn focus(window: &mut Window, cx: &mut App) -> Focus {
    let mut handle = |key: &'static str, tab: bool| {
        window
            .use_keyed_state(key, cx, |_, cx| cx.focus_handle().tab_stop(tab))
            .read(cx)
            .clone()
    };
    Focus {
        trigger: handle("nav-more-focus", true),
        content: handle("nav-more-menu-focus", false),
        item: handle("nav-more-customize-focus", false),
    }
}

/// The rows after Search: `-mt-1.5 flex flex-col gap-0.5 pr-3 pl-3`.
pub fn section(window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<More>() {
        // `useEscapeKeydown`: a document listener, not one on the focus path.
        let escape = cx.intercept_keystrokes(|event, window, cx| {
            if event.keystroke.key == "escape" && cx.global::<More>().menu.is_open() {
                log::info!(target: "sidebar", "More closed by Escape");
                hover_menu::close(menu, false, window, cx);
            }
        });
        cx.set_global(More {
            _escape: Some(escape),
            ..More::default()
        });
    }
    div()
        .flex()
        .flex_col()
        .flex_none()
        .gap(css(2.0))
        .mt(css(-6.0))
        .px(css(12.0))
        .child(row(window, cx))
        .into_any_element()
}

/// Whether a press at `at` lands on the open menu, as `data-radix-popper-content-wrapper` does.
pub fn over_menu(at: Point<Pixels>, window: &Window, cx: &App) -> bool {
    cx.try_global::<More>()
        .and_then(|m| m.panel.as_ref())
        .is_some_and(|h| h.is_hovered_at(at, window))
}

/// A click or a key opens it; `first` focuses Customize, else the content.
fn open_menu(f: (&FocusHandle, &FocusHandle), first: bool, window: &mut Window, cx: &mut App) {
    let (trigger, target) = f;
    hover_menu::open(menu, trigger.clone(), window, cx);
    window.focus(target, cx);
    log::info!(target: "sidebar", "More opened from the trigger, first item {first}");
}

/// The trigger row, `ROW_CLASS` lit while the menu is open.
fn row(window: &mut Window, cx: &mut App) -> AnyElement {
    let f = focus(window, cx);
    let t = cx.global::<Translator>();
    let label = t.t("sidebar.navigation.more", &[]);
    let open = cx.global::<More>().menu.is_open();
    if !open {
        cx.global_mut::<More>().panel = None;
    }
    let appearance = cx.global::<Appearance>();
    let text = appearance.token("sidebar-nav-text");
    let hover = appearance.hsl_token("sidebar-item-hover");
    let size = text_size(14.0, cx);
    let (content, item) = (f.content.clone(), f.item.clone());
    let (own, own_key, key_item) = (f.trigger.clone(), f.trigger.clone(), f.item.clone());
    let trigger = div()
        .id("nav-more")
        .debug_selector(|| "nav-more".into())
        .track_focus(&f.trigger)
        .role(Role::Button)
        .aria_label(label.clone())
        .aria_expanded(open)
        .relative()
        .flex()
        .h(css(32.0))
        .w_full()
        .items_center()
        .gap(css(10.0))
        .rounded(css(9999.0))
        .px(css(12.0))
        .cursor_pointer()
        .text_size(css(size))
        .line_height(css(size))
        .text_color(text)
        .hover(|s| s.bg(hover))
        .when(open, |el| el.bg(hover))
        .hover_listener_mode(HoverListenerMode::InputModalityIndependent)
        .on_hover(|hovered, window, cx| match hovered {
            true => hover_menu::enter_trigger(menu, window, cx),
            false => hover_menu::leave(menu, window, cx),
        })
        // `onTriggerPointerDown`: an open menu keeps the press from toggling or taking focus.
        .on_mouse_down(MouseButton::Left, |event, window, cx| {
            if cx.global::<More>().menu.is_open() && !event.modifiers.control {
                window.prevent_default();
            }
        })
        .on_click(move |event: &ClickEvent, window, cx| {
            if cx.global::<More>().menu.is_open() {
                return;
            }
            let first = event.is_keyboard();
            let target = if first { &item } else { &content };
            open_menu((&own, target), first, window, cx);
        })
        .on_key_down(move |event, window, cx| {
            if event.keystroke.key == "down" {
                cx.stop_propagation();
                open_menu((&own_key, &key_item), true, window, cx);
            }
        })
        .child(
            canvas(
                |bounds, _, cx| cx.global_mut::<More>().trigger = Some(bounds),
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0(),
        )
        .child(icon(IconName::Ellipsis.svg(), 15.0, 1.8).text_color(text))
        .child(label);
    let trigger = trigger.into_any_element();
    match open {
        true => div()
            .child(trigger)
            .child(panel(f, window, cx))
            .into_any_element(),
        false => trigger,
    }
}

/// Right of the trigger, flipped left when only that side fits, and never over the titlebar.
fn place(
    trigger: Bounds<Pixels>,
    width: Pixels,
    viewport: Pixels,
    gap: Pixels,
    top: Pixels,
) -> Point<Pixels> {
    let right = trigger.right() + gap;
    let left = trigger.left() - gap - width;
    let x = match right + width > viewport - gap && left >= gap {
        true => left,
        false => right,
    };
    point(x, trigger.top().max(top))
}

/// `DropdownMenuContent side="right" align="start" sideOffset={8}`, under the titlebar strip.
fn panel(f: Focus, window: &Window, cx: &mut App) -> AnyElement {
    let viewport = window.viewport_size().width;
    let state = cx.global::<More>();
    let Some(trigger) = state.trigger else {
        return div().into_any_element();
    };
    let width = state
        .panel
        .as_ref()
        .map_or(window_px(172.0, cx), |p| p.bounds.size.width);
    let gap = window_px(8.0, cx);
    let at = place(trigger, width, viewport, gap, window_px(58.0, cx));
    let label = cx
        .global::<Translator>()
        .t("sidebar.navigation.customize.title", &[]);
    let theme = cx.theme();
    let (fg, muted, hovered) = (theme.colors.fg, theme.colors.fg_muted, theme.colors.hover);
    let row_height = theme.control_height(ely_gpui_component::theme::ControlSize::Md);
    let radius = theme.radius(ely_gpui_component::theme::Radius::Md);
    let item = f.item.clone();
    let content = surface("nav-more-menu", cx)
        .debug_selector(|| "nav-more-menu".into())
        .track_focus(&f.content)
        .role(Role::Menu)
        .min_w(css(172.0))
        .p(css(4.0))
        .hover_listener_mode(HoverListenerMode::InputModalityIndependent)
        .on_hover(|hovered, window, cx| match hovered {
            true => hover_menu::enter_content(menu, cx),
            false => hover_menu::leave(menu, window, cx),
        })
        // `MenuContent` keeps Tab and Shift-Tab inside the menu.
        .capture_action(|_: &FocusNext, _, cx| cx.stop_propagation())
        .capture_action(|_: &FocusPrev, _, cx| cx.stop_propagation())
        // `Menu`'s roving focus over its one item.
        .on_key_down(move |event, window, cx| {
            if matches!(event.keystroke.key.as_str(), "down" | "up" | "home" | "end") {
                cx.stop_propagation();
                window.focus(&item, cx);
            }
        })
        .child(
            canvas(
                |bounds, window, cx| {
                    let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                    cx.global_mut::<More>().panel = Some(hitbox);
                },
                |_, _, window, _| listen(window),
            )
            .absolute()
            .inset_0(),
        )
        .child(
            div()
                .id("nav-more-customize")
                .debug_selector(|| "nav-more-customize".into())
                .track_focus(&f.item)
                .role(Role::MenuItem)
                .aria_label(label.clone())
                .flex()
                .items_center()
                .gap(css(10.0))
                .h(row_height)
                .px(css(8.0))
                .rounded(radius)
                .cursor_pointer()
                .text_color(fg)
                .hover(|s| s.bg(hovered))
                .focus(|s| s.bg(hovered))
                .on_click(|_, window, cx| {
                    log::info!(target: "sidebar", "Customize chosen from More");
                    hover_menu::close(menu, false, window, cx);
                    cx.global_mut::<More>().customize = Some(Draft::default());
                })
                .child(icon(IconName::SlidersHorizontal.svg(), 16.0, 1.8).text_color(muted))
                .child(label),
        );
    deferred(
        anchored()
            .position(at)
            .snap_to_window_with_margin(window_px(8.0, cx))
            .child(content),
    )
    .with_priority(1)
    .into_any_element()
}

/// `DismissableLayer`'s press outside.
fn listen(window: &mut Window) {
    window.on_mouse_event(|event: &MouseDownEvent, phase, window, cx| {
        let s = cx.global::<More>();
        let on_trigger = s.trigger.is_some_and(|b| b.contains(&event.position));
        let on_menu = s
            .panel
            .as_ref()
            .is_some_and(|h| h.is_hovered_at(event.position, window));
        if phase == DispatchPhase::Capture && !on_trigger && !on_menu {
            log::info!(target: "sidebar", "More closed by a press outside");
            hover_menu::close(menu, true, window, cx);
        }
    });
}

#[cfg(test)]
mod tests;
