//! The sidebar's conversation search: the query `conversationSearchContext` shares, and the top
//! nav's Search row as `SidebarInlineSearch` draws it.

use ely_gpui_component::{
    forms::{InputEvent, TextInput},
    primitives::IconName,
};
use gpui::{
    AnyElement, App, AppContext as _, DispatchPhase, Entity, FocusHandle, Global, HitboxBehavior,
    HoverListenerMode, InteractiveElement as _, IntoElement, MouseDownEvent, ParentElement as _,
    Role, StatefulInteractiveElement as _, Styled as _, Subscription, Window, canvas, div,
    prelude::FluentBuilder as _,
};

use crate::{
    appearance_settings::text_size, i18n::Translator, lucide::icon, preferences::Preferences,
    theme::Appearance, theme_import::color::js_trim, tip::tip, units::css,
};

mod marks;
mod options;
pub mod projects;
pub mod results;
pub mod run;
pub mod scroll;

pub use options::Options;

/// The field and its query, made with the first window that draws the row, and the options.
pub struct Search {
    field: Option<Field>,
    pub options: Options,
    /// A choice in the open menu took the press under way.
    chosen: bool,
    run: run::Run,
    /// The results' hitbox this frame, part of the search surface.
    surface: Option<gpui::Hitbox>,
    scroll: scroll::Scroll,
}

struct Field {
    input: Entity<TextInput>,
    query: String,
    _events: Subscription,
}

impl Global for Search {}

/// The search with the sort read back from the preferences.
pub fn install(cx: &mut App) {
    let options = Options::load(cx.global::<Preferences>());
    log::info!(target: "search", "sort {} loaded", options.sort);
    cx.set_global(Search {
        field: None,
        options,
        chosen: false,
        run: run::Run::default(),
        surface: None,
        scroll: scroll::Scroll::default(),
    });
}

/// `search.query`; empty before the row is drawn.
pub fn query(cx: &App) -> &str {
    cx.global::<Search>()
        .field
        .as_ref()
        .map_or("", |f| f.query.as_str())
}

/// `search.trimmed`.
pub fn trimmed(cx: &App) -> &str {
    js_trim(query(cx))
}

fn field(window: &mut Window, cx: &mut App) -> Entity<TextInput> {
    if let Some(field) = &cx.global::<Search>().field {
        return field.input.clone();
    }
    let label = cx
        .global::<Translator>()
        .t("ccAgent.search.placeholder", &[]);
    let input = cx.new(|cx| TextInput::new(window, cx).label(label));
    let events = window.subscribe(&input, cx, |input, event, _, cx| {
        if *event == InputEvent::Changed {
            let text = input.read(cx).text().to_owned();
            log::debug!(target: "search", "query {} chars", text.chars().count());
            if let Some(field) = cx.global_mut::<Search>().field.as_mut() {
                field.query = text;
            }
            run::sync(cx);
            cx.refresh_windows();
        }
    });
    cx.global_mut::<Search>().field = Some(Field {
        input: input.clone(),
        query: String::new(),
        _events: events,
    });
    input
}

/// `search.reset` and `setQuery('')`: the query goes, the field stays.
fn reset(cx: &mut App) {
    let Some(input) = cx
        .global::<Search>()
        .field
        .as_ref()
        .map(|f| f.input.clone())
    else {
        return;
    };
    input.update(cx, |input, cx| input.set_text("", cx));
    if let Some(field) = cx.global_mut::<Search>().field.as_mut() {
        field.query.clear();
    }
    log::info!(target: "search", "query cleared");
    run::sync(cx);
    cx.refresh_windows();
}

/// `section="scrollable"` with Search its only entry: `shrink-0 px-3` around the row.
pub fn section(window: &mut Window, cx: &mut App) -> AnyElement {
    div()
        .flex_none()
        .px(css(12.0))
        .child(row(window, cx))
        .into_any_element()
}

/// `SidebarInlineSearch`: the nav row at rest, the field while hovered, focused or holding a
/// query. Focus moves straight to the field, which stands for `forceOpen`.
fn row(window: &mut Window, cx: &mut App) -> AnyElement {
    let input = field(window, cx);
    let input_focus = input.read(cx).focus().clone();
    let hovered = window.use_keyed_state("nav-search-hovered", cx, |_, _| false);
    let clear_focus = window
        .use_keyed_state("nav-search-clear-focus", cx, |_, cx| {
            cx.focus_handle().tab_stop(true)
        })
        .read(cx)
        .clone();
    let rest = window
        .use_keyed_state("nav-search-rest", cx, |_, cx| {
            cx.focus_handle().tab_stop(true)
        })
        .read(cx)
        .clone();
    let filter = window
        .use_keyed_state("nav-search-filter-focus", cx, |_, cx| {
            cx.focus_handle().tab_stop(true)
        })
        .read(cx)
        .clone();
    let menu = window
        .use_keyed_state("nav-search-menu", cx, |_, cx| cx.focus_handle())
        .read(cx)
        .clone();
    if rest.is_focused(window) {
        window.focus(&input_focus, cx);
    }
    let query = query(cx).to_owned();
    let within = input_focus.is_focused(window)
        || clear_focus.is_focused(window)
        || menu.contains_focused(window, cx);
    let expanded = *hovered.read(cx) || within || !query.is_empty();
    let translator = cx.global::<Translator>();
    let (open, placeholder, clear) = (
        translator.t("ccAgent.search.open", &[]),
        translator.t("ccAgent.search.placeholder", &[]),
        translator.t("ccAgent.search.clear", &[]),
    );
    let appearance = cx.global::<Appearance>();
    let nav = appearance.token("sidebar-nav-text");
    let size = text_size(14.0, cx);
    let body = match expanded {
        true => div()
            .relative()
            .flex()
            .flex_1()
            .min_w_0()
            .items_center()
            .text_color(appearance.token("text-primary"))
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
            .child(input.clone())
            .into_any_element(),
        false => div()
            .id("nav-search-open")
            .debug_selector(|| "nav-search-open".into())
            .track_focus(&rest)
            .role(Role::Button)
            .aria_label(open.clone())
            .flex_1()
            .min_w_0()
            .truncate()
            .text_color(nav)
            .on_click(move |_, window, cx| window.focus(&input_focus, cx))
            .child(open)
            .into_any_element(),
    };
    let tertiary = appearance.token("text-tertiary");
    let strong = appearance.token("text-primary");
    let hover_bg = appearance.hsl_token("sidebar-item-hover");
    let clear_button = div()
        .id("nav-search-clear")
        .debug_selector(|| "nav-search-clear".into())
        .track_focus(&clear_focus)
        .role(Role::Button)
        .aria_label(clear.clone())
        .group("nav-search-clear")
        .flex()
        .flex_none()
        .size(css(20.0))
        .items_center()
        .justify_center()
        .rounded(css(9999.0))
        .cursor_pointer()
        .hover(|s| s.bg(hover_bg))
        .tooltip(tip(clear))
        .on_click(|_, window, cx| {
            reset(cx);
            window.blur(cx);
        })
        .child(
            icon(IconName::X.svg(), 13.0, 2.0)
                .text_color(tertiary)
                .group_hover("nav-search-clear", |s| s.text_color(strong)),
        );
    let border = appearance.token("border-default");
    let surface = appearance.token("sidebar-search-input-bg");
    let running = !trimmed(cx).is_empty();
    div()
        .id("nav-search")
        .debug_selector(|| "nav-search".into())
        .relative()
        .flex()
        .h(css(32.0))
        .w_full()
        .items_center()
        .gap(css(10.0))
        .rounded(css(9999.0))
        .border(css(1.0))
        .pl(css(12.0))
        .text_size(css(size))
        .line_height(css(size))
        .map(|el| match expanded {
            true => el.border_color(border).bg(surface).pr(css(6.0)),
            false => el.border_color(gpui::transparent_black()).pr(css(12.0)),
        })
        .hover_listener_mode(HoverListenerMode::InputModalityIndependent)
        .on_hover(move |h, _, cx| {
            hovered.update(cx, |hovered, cx| {
                *hovered = *h;
                cx.notify();
            })
        })
        .when(running, |el| {
            el.child(outside(menu.clone(), filter.clone()))
        })
        .child(icon(IconName::Search.svg(), 15.0, 1.8).text_color(nav))
        .child(body)
        .when(expanded && !query.is_empty(), |el| el.child(clear_button))
        .when(expanded, |el| {
            // The trigger and its menu; focus inside, off the trigger, is `menuOpen`.
            el.child(
                div()
                    .track_focus(&menu)
                    .child(options::button(&filter, window, cx)),
            )
        })
        .into_any_element()
}

/// Whether the options menu holds focus.
fn menu_open(menu: &FocusHandle, filter: &FocusHandle, window: &Window, cx: &App) -> bool {
    menu.contains_focused(window, cx) && !filter.is_focused(window)
}

/// The provider's pointer-down outside `data-conversation-search-surface`: the query goes.
/// A press the open menu keeps, as `data-radix-popper-content-wrapper`, leaves it.
fn outside(menu: FocusHandle, filter: FocusHandle) -> impl IntoElement {
    canvas(
        |bounds, window, cx| {
            cx.global_mut::<Search>().surface = None;
            window.insert_hitbox(bounds, HitboxBehavior::Normal)
        },
        move |_, hitbox, window, _| {
            window.on_mouse_event(move |e: &MouseDownEvent, phase, window, cx| {
                // `closest(surface)` on the press's real target.
                let results = cx.global::<Search>().surface.as_ref();
                let inside = hitbox.is_hovered_at(e.position, window)
                    || results.is_some_and(|r| r.is_hovered_at(e.position, window));
                if phase != DispatchPhase::Capture || inside {
                    return;
                }
                if !menu_open(&menu, &filter, window, cx) {
                    log::info!(target: "search", "a press outside the search");
                    return reset(cx);
                }
                // The menu closes on a press past its panels, or on a choice; once done, see.
                cx.global_mut::<Search>().chosen = false;
                let (menu, filter) = (menu.clone(), filter.clone());
                window.defer(cx, move |window, cx| {
                    let chosen = std::mem::take(&mut cx.global_mut::<Search>().chosen);
                    if !chosen && !menu_open(&menu, &filter, window, cx) {
                        log::info!(target: "search", "a press outside the search and its menu");
                        reset(cx);
                    }
                });
            });
        },
    )
    .absolute()
    .inset_0()
}

#[cfg(test)]
mod tests;
