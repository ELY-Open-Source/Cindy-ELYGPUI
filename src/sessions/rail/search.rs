//! `ConversationSearchBox`: the rail's Search tile and its popover over the rail's own search.

use ely_gpui_component::primitives::{FocusScope, IconName};
use gpui::{
    AnyElement, App, Bounds, DispatchPhase, FocusHandle, Global, Hitbox, HitboxBehavior,
    InteractiveElement as _, IntoElement, MouseDownEvent, ParentElement as _, Pixels, Point, Role,
    StatefulInteractiveElement as _, Styled as _, Subscription, Window, anchored, canvas, deferred,
    div, point, prelude::FluentBuilder as _,
};

use crate::{
    appearance_settings::text_size,
    i18n::Translator,
    lucide::icon,
    sessions::search::{self, Which, options, results, run},
    theme::Appearance,
    tip::tip,
    units::{css, to_css, window_px},
};

const ID: &str = "rail-search";

#[derive(Default)]
struct Popover {
    open: bool,
    /// This frame's handles, for a close from an event.
    focus: Option<Focus>,
    trigger: Option<Bounds<Pixels>>,
    /// The open panel's hitbox this frame.
    panel: Option<Hitbox>,
    /// Escape and focus leaving, while open.
    _watch: Vec<Subscription>,
}

impl Global for Popover {}

#[derive(Clone)]
struct Focus {
    trigger: FocusHandle,
    panel: FocusHandle,
    menu: FocusHandle,
    filter: FocusHandle,
}

fn focus(window: &mut Window, cx: &mut App) -> Focus {
    let mut keyed = |key: &'static str, tab: bool| {
        window
            .use_keyed_state(key, cx, |_, cx| cx.focus_handle().tab_stop(tab))
            .read(cx)
            .clone()
    };
    let (panel, menu, filter) = (
        keyed("rail-search-panel-focus", false),
        keyed("rail-search-menu", false),
        keyed("rail-search-filter-focus", true),
    );
    Focus {
        trigger: super::tab_stop(ID, window, cx),
        panel,
        menu,
        filter,
    }
}

/// The open popover's options menu handles, for the inline search's popper check.
pub fn menu(cx: &App) -> Option<(FocusHandle, FocusHandle)> {
    let p = cx.try_global::<Popover>()?;
    let f = p.focus.as_ref().filter(|_| p.open)?;
    Some((f.menu.clone(), f.filter.clone()))
}

/// Whether a press at `at` lands on the open popover, a popper to the inline search.
pub fn over(at: Point<Pixels>, window: &Window, cx: &App) -> bool {
    cx.try_global::<Popover>()
        .and_then(|p| p.panel.as_ref())
        .is_some_and(|h| h.is_hovered_at(at, window))
}

/// `handleOpenChange`: a close resets the query and focuses `back`, as `onCloseAutoFocus` does.
fn set_open(open: bool, back: Option<FocusHandle>, window: &mut Window, cx: &mut App) {
    if cx.global::<Popover>().open == open {
        return;
    }
    cx.global_mut::<Popover>().open = open;
    log::info!(target: "search", "rail search {}", if open { "opened" } else { "closed" });
    match open {
        true => {
            run::enable(Which::Rail, true, cx);
            let field = search::field(Which::Rail, window, cx)
                .read(cx)
                .focus()
                .clone();
            window.defer(cx, move |window, cx| window.focus(&field, cx));
        }
        false => {
            let p = cx.global_mut::<Popover>();
            (p._watch, p.panel) = (Vec::new(), None);
            search::reset(Which::Rail, cx);
            run::enable(Which::Rail, false, cx);
            if let Some(trigger) = back {
                window.focus(&trigger, cx);
            }
        }
    }
    cx.refresh_windows();
}

/// `DismissableLayer`'s Escape and focus outside; the open options menu takes Escape itself.
fn watch(f: &Focus, window: &mut Window, cx: &mut App) -> Vec<Subscription> {
    let (menu, filter, trigger) = (f.menu.clone(), f.filter.clone(), f.trigger.clone());
    let escape = cx.intercept_keystrokes(move |event, window, cx| {
        let open = cx.global::<Popover>().open;
        if open && event.keystroke.key == "escape" && !search::menu_open(&menu, &filter, window, cx)
        {
            log::info!(target: "search", "rail search closed by Escape");
            set_open(false, Some(trigger.clone()), window, cx);
        }
    });
    let trigger = f.trigger.clone();
    let blur = window.on_focus_out(&f.panel, cx, move |_, window, cx| {
        // A focus on the trigger is its own toggle.
        if cx.global::<Popover>().open && !trigger.is_focused(window) {
            log::info!(target: "search", "rail search closed: focus left");
            set_open(false, None, window, cx);
        }
    });
    vec![escape, blur]
}

/// `onResultChosen`: the popover closes before the task opens.
pub fn chosen(window: &mut Window, cx: &mut App) {
    let trigger = cx
        .global::<Popover>()
        .focus
        .as_ref()
        .map(|f| f.trigger.clone());
    set_open(false, trigger, window, cx);
}

/// The tile, `PopoverTrigger`: a click toggles the popover.
pub fn tile(window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<Popover>() {
        cx.set_global(Popover::default());
    }
    let f = focus(window, cx);
    cx.global_mut::<Popover>().focus = Some(f.clone());
    let open = cx.global::<Popover>().open;
    let label = cx.global::<Translator>().t("ccAgent.search.open", &[]);
    let handles = f.clone();
    let tile = super::tile(ID, label, IconName::Search.svg(), open, window, cx)
        .on_click(move |_, window, cx| {
            let open = cx.global::<Popover>().open;
            set_open(!open, Some(handles.trigger.clone()), window, cx);
            if !open {
                cx.global_mut::<Popover>()._watch = watch(&handles, window, cx);
            }
        })
        .child(
            canvas(
                |bounds, _, cx| cx.global_mut::<Popover>().trigger = Some(bounds),
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0(),
        );
    match open {
        true => div()
            .child(tile)
            .child(panel(f, window, cx))
            .into_any_element(),
        false => tile.into_any_element(),
    }
}

/// `PopoverContent side="right" align="start" sideOffset={8} collisionPadding={12}`.
fn panel(f: Focus, window: &mut Window, cx: &mut App) -> AnyElement {
    let Some(trigger) = cx.global::<Popover>().trigger else {
        return div().into_any_element();
    };
    let at = point(trigger.right() + window_px(8.0, cx), trigger.top());
    let viewport = window.viewport_size();
    let width = 640.0_f32.min(to_css(viewport.width, cx) - 32.0);
    let max_results = 620.0_f32.min(to_css(viewport.height, cx) - 140.0);
    let appearance = cx.global::<Appearance>();
    let border = appearance.token("cmd-palette-border");
    let content = div()
        .id("rail-search-panel")
        .debug_selector(|| "rail-search-panel".into())
        // A layer over the content: a press here reaches nothing beneath.
        .occlude()
        .w(css(width))
        .rounded(css(12.0))
        .border(css(1.0))
        .border_color(border)
        .bg(appearance.token("cmd-palette-bg"))
        .text_color(appearance.token("cmd-palette-item-text"))
        .shadow(vec![appearance.shadow_token("cmd-palette-shadow", cx)])
        .child(
            canvas(
                |bounds, window, cx| {
                    let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                    cx.global_mut::<Popover>().panel = Some(hitbox);
                },
                {
                    let menu = (f.menu.clone(), f.filter.clone());
                    move |_, _, window, _| listen(menu, window)
                },
            )
            .absolute()
            .inset_0(),
        )
        .child(
            div()
                .border_b(css(1.0))
                .border_color(border)
                .p(css(8.0))
                .child(field(&f.panel, window, cx))
                .child(
                    div()
                        .mt(css(8.0))
                        .flex()
                        .items_center()
                        .gap(css(8.0))
                        .child(div().track_focus(&f.menu).child(options::button(
                            Which::Rail,
                            &f.filter,
                            window,
                            cx,
                        ))),
                ),
        )
        .child(body(max_results, window, cx));
    // Radix's `FocusScope loop`: Tab and Shift-Tab wrap inside.
    let scope = FocusScope::new(&f.panel).trap().child(content);
    deferred(
        anchored()
            .position(at)
            .snap_to_window_with_margin(window_px(12.0, cx))
            .child(scope),
    )
    .with_priority(1)
    .into_any_element()
}

/// The field row: `h-9 rounded-full border px-3`, with clear once there is a query.
fn field(panel: &FocusHandle, window: &mut Window, cx: &mut App) -> AnyElement {
    let panel = panel.clone();
    let input = search::field(Which::Rail, window, cx);
    let clear_focus = window
        .use_keyed_state("rail-search-clear-focus", cx, |_, cx| {
            cx.focus_handle().tab_stop(true)
        })
        .read(cx)
        .clone();
    let query = search::query(Which::Rail, cx).to_owned();
    let t = cx.global::<Translator>();
    let (placeholder, clear) = (
        t.t("ccAgent.search.placeholder", &[]),
        t.t("ccAgent.search.clear", &[]),
    );
    let appearance = cx.global::<Appearance>();
    let size = text_size(13.0, cx);
    let (tertiary, strong) = (
        appearance.token("text-tertiary"),
        appearance.token("text-primary"),
    );
    let hover = appearance.hsl_token("sidebar-item-hover");
    let clear_button = div()
        .id("rail-search-clear")
        .debug_selector(|| "rail-search-clear".into())
        .track_focus(&clear_focus)
        .role(Role::Button)
        .group("rail-search-clear")
        .flex()
        .flex_none()
        .size(css(20.0))
        .items_center()
        .justify_center()
        .rounded(css(9999.0))
        .cursor_pointer()
        .hover(|s| s.bg(hover))
        .aria_label(clear.clone())
        .tooltip(tip(clear))
        // Clear leaves with its focus; the panel keeps it, as `body` does, so the popover stays.
        .on_click(move |_, window, cx| {
            window.focus(&panel, cx);
            search::reset(Which::Rail, cx);
        })
        .child(
            icon(IconName::X.svg(), 13.0, 2.0)
                .text_color(tertiary)
                .group_hover("rail-search-clear", |s| s.text_color(strong)),
        );
    div()
        .flex()
        .h(css(36.0))
        .items_center()
        .gap(css(8.0))
        .rounded(css(9999.0))
        .border(css(1.0))
        .border_color(appearance.token("border-default"))
        .bg(appearance.token("surface-elevated"))
        .px(css(12.0))
        .text_size(css(size))
        .line_height(css(size))
        .child(
            icon(IconName::Search.svg(), 15.0, 2.0)
                .text_color(appearance.token("cmd-palette-item-icon")),
        )
        .child(
            div()
                .relative()
                .flex()
                .flex_1()
                .min_w_0()
                .items_center()
                .text_color(strong)
                // Ely draws its placeholder in `fg_subtle`; Cindy's is `text-placeholder`.
                .when(query.is_empty(), |el| {
                    el.child(
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .items_center()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_color(appearance.token("text-placeholder"))
                            .child(placeholder),
                    )
                })
                .child(input),
        )
        .when(!query.is_empty(), |el| el.child(clear_button))
        .into_any_element()
}

/// `SearchResultsBody` with `maxHeightClass`; the empty line before a query.
fn body(max_height: f32, window: &mut Window, cx: &mut App) -> AnyElement {
    let Some(results) = results::body(Which::Rail, window, cx) else {
        let size = text_size(12.0, cx);
        return div()
            .debug_selector(|| "rail-search-empty".into())
            .px(css(12.0))
            .py(css(24.0))
            .text_center()
            .text_size(css(size))
            .text_color(cx.global::<Appearance>().token("cmd-palette-empty"))
            .child(cx.global::<Translator>().t("ccAgent.search.empty", &[]))
            .into_any_element();
    };
    div()
        .id("rail-search-results")
        .max_h(css(max_height))
        .overflow_y_scroll()
        .child(results)
        .into_any_element()
}

/// `onPointerDownOutside`; the trigger toggles itself, and a press the open menu keeps stays.
fn listen(menu: (FocusHandle, FocusHandle), window: &mut Window) {
    window.on_mouse_event(move |e: &MouseDownEvent, phase, window, cx| {
        let p = cx.global::<Popover>();
        let on_trigger = p.trigger.is_some_and(|b| b.contains(&e.position));
        let on_panel = p
            .panel
            .as_ref()
            .is_some_and(|h| h.is_hovered_at(e.position, window));
        if phase != DispatchPhase::Capture || on_trigger || on_panel || !p.open {
            return;
        }
        if !search::menu_open(&menu.0, &menu.1, window, cx) {
            log::info!(target: "search", "rail search closed by a press outside");
            return set_open(false, None, window, cx);
        }
        search::past_menu(menu.clone(), window, cx, |window, cx| {
            log::info!(target: "search", "rail search closed by a press outside its menu");
            set_open(false, None, window, cx);
        });
    });
}

#[cfg(test)]
mod tests;
