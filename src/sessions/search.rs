//! The sidebar's conversation search: the query `conversationSearchContext` shares, and the top
//! nav's Search row as `SidebarInlineSearch` draws it.

use ely_gpui_component::{
    forms::{InputEvent, TextInput},
    primitives::IconName,
};
use gpui::{
    AnyElement, App, AppContext as _, DispatchPhase, Entity, FocusHandle, Global, Hitbox,
    HitboxBehavior, HoverListenerMode, InteractiveElement as _, IntoElement, MouseDownEvent,
    ParentElement as _, Role, StatefulInteractiveElement as _, Styled as _, Subscription, Window,
    canvas, div, prelude::FluentBuilder as _,
};

use crate::{
    appearance_settings::text_size, i18n::Translator, lucide::icon, preferences::Preferences,
    theme::Appearance, theme_import::color::js_trim, tip::tip, units::css,
};

mod marks;
pub mod options;
pub mod projects;
pub mod results;
pub mod run;
pub mod scroll;
pub mod stick;

pub use options::Options;

/// Which `useConversationSearch` a call means: the Search row's or the rail popover's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    Inline,
    Rail,
}

/// One search: its field and query, filters and run.
#[derive(Default)]
pub struct Instance {
    field: Option<Field>,
    pub options: Options,
    run: run::Run,
}

/// Both searches, the sort they share, and the inline search's surface and scroll.
pub struct Search {
    inline: Instance,
    rail: Instance,
    /// `subscribeSearchSortBy`: one sort for both.
    pub sort: &'static str,
    /// A choice in the open menu took the press under way.
    chosen: bool,
    /// The results' hitbox this frame, part of the search surface.
    surface: Option<gpui::Hitbox>,
    scroll: scroll::Scroll,
    /// The pinned row's bottom this frame.
    pinned_bottom: Option<gpui::Pixels>,
}

struct Field {
    input: Entity<TextInput>,
    query: String,
    _events: Subscription,
}

impl Global for Search {}

/// The searches with the sort read back from the preferences.
pub fn install(cx: &mut App) {
    let sort = options::load_sort(cx.global::<Preferences>());
    log::info!(target: "search", "sort {sort} loaded");
    let mut inline = Instance::default();
    inline.run.enabled = true;
    cx.set_global(Search {
        inline,
        rail: Instance::default(),
        sort,
        chosen: false,
        surface: None,
        scroll: scroll::Scroll::default(),
        pinned_bottom: None,
    });
}

pub fn of(w: Which, cx: &App) -> &Instance {
    let s = cx.global::<Search>();
    match w {
        Which::Inline => &s.inline,
        Which::Rail => &s.rail,
    }
}

pub fn of_mut(w: Which, cx: &mut App) -> &mut Instance {
    let s = cx.global_mut::<Search>();
    match w {
        Which::Inline => &mut s.inline,
        Which::Rail => &mut s.rail,
    }
}

/// `search.query`; empty before its field is made.
pub fn query(w: Which, cx: &App) -> &str {
    of(w, cx).field.as_ref().map_or("", |f| f.query.as_str())
}

/// `search.trimmed`.
pub fn trimmed(w: Which, cx: &App) -> &str {
    js_trim(query(w, cx))
}

pub(crate) fn field(w: Which, window: &mut Window, cx: &mut App) -> Entity<TextInput> {
    if let Some(field) = &of(w, cx).field {
        return field.input.clone();
    }
    let label = cx
        .global::<Translator>()
        .t("ccAgent.search.placeholder", &[]);
    let input = cx.new(|cx| TextInput::new(window, cx).label(label));
    let events = window.subscribe(&input, cx, move |input, event, _, cx| {
        if *event == InputEvent::Changed {
            let text = input.read(cx).text().to_owned();
            log::debug!(target: "search", "{w:?} query {} chars", text.chars().count());
            if let Some(field) = of_mut(w, cx).field.as_mut() {
                field.query = text;
            }
            run::sync(w, cx);
            cx.refresh_windows();
        }
    });
    of_mut(w, cx).field = Some(Field {
        input: input.clone(),
        query: String::new(),
        _events: events,
    });
    input
}

/// `search.reset` and `setQuery('')`: the query goes, the field stays.
pub(crate) fn reset(w: Which, cx: &mut App) {
    let Some(input) = of(w, cx).field.as_ref().map(|f| f.input.clone()) else {
        return;
    };
    input.update(cx, |input, cx| input.set_text("", cx));
    if let Some(field) = of_mut(w, cx).field.as_mut() {
        field.query.clear();
    }
    log::info!(target: "search", "{w:?} query cleared");
    run::sync(w, cx);
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
    let input = field(Which::Inline, window, cx);
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
    let query = query(Which::Inline, cx).to_owned();
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
            reset(Which::Inline, cx);
            window.blur(cx);
        })
        .child(
            icon(IconName::X.svg(), 13.0, 2.0)
                .text_color(tertiary)
                .group_hover("nav-search-clear", |s| s.text_color(strong)),
        );
    let border = appearance.token("border-default");
    let surface = appearance.token("sidebar-search-input-bg");
    let running = !trimmed(Which::Inline, cx).is_empty();
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
            el.child(outside(Some((menu.clone(), filter.clone()))).inset_0())
        })
        .child(icon(IconName::Search.svg(), 15.0, 1.8).text_color(nav))
        .child(body)
        .when(expanded && !query.is_empty(), |el| el.child(clear_button))
        .when(expanded, |el| {
            // The trigger and its menu; focus inside, off the trigger, is `menuOpen`.
            el.child(div().track_focus(&menu).child(options::button(
                Which::Inline,
                &filter,
                window,
                cx,
            )))
        })
        .into_any_element()
}

/// Whether the options menu holds focus.
pub(crate) fn menu_open(
    menu: &FocusHandle,
    filter: &FocusHandle,
    window: &Window,
    cx: &App,
) -> bool {
    menu.contains_focused(window, cx) && !filter.is_focused(window)
}

/// A press past the open menu: once done, `out` runs unless a row took it or the menu stayed.
pub(crate) fn past_menu(
    (menu, filter): (FocusHandle, FocusHandle),
    window: &mut Window,
    cx: &mut App,
    out: impl FnOnce(&mut Window, &mut App) + 'static,
) {
    cx.global_mut::<Search>().chosen = false;
    // Read, not taken: both searches may wait on the same press.
    window.defer(cx, move |window, cx| {
        let chosen = cx.global::<Search>().chosen;
        if !chosen && !menu_open(&menu, &filter, window, cx) {
            out(window, cx);
        }
    });
}

/// The provider's listener while the row is unmounted, as Customize leaves it: no surface, no menu.
pub fn listen(cx: &App) -> Option<AnyElement> {
    let running = !trimmed(Which::Inline, cx).is_empty();
    running.then(|| outside(None).size_0().into_any_element())
}

/// The provider's pointer-down outside `data-conversation-search-surface`: the query goes.
/// A press the open menu keeps, as `data-radix-popper-content-wrapper`, leaves it.
fn outside(menu: Option<(FocusHandle, FocusHandle)>) -> gpui::Canvas<Hitbox> {
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
                    || results.is_some_and(|r| r.is_hovered_at(e.position, window))
                    || crate::sidebar::top_nav::more::over_menu(e.position, window, cx)
                    || crate::sessions::rail::search::over(e.position, window, cx);
                if phase != DispatchPhase::Capture || inside {
                    return;
                }
                // The rail popover's open menu is a popper too.
                let rail = crate::sessions::rail::search::menu(cx);
                let Some((menu, filter)) = menu
                    .clone()
                    .into_iter()
                    .chain(rail)
                    .find(|(m, f)| menu_open(m, f, window, cx))
                else {
                    log::info!(target: "search", "a press outside the search");
                    return reset(Which::Inline, cx);
                };
                past_menu((menu, filter), window, cx, |_, cx| {
                    log::info!(target: "search", "a press outside the search and its menu");
                    reset(Which::Inline, cx);
                });
            });
        },
    )
    .absolute()
}

#[cfg(test)]
mod tests;
