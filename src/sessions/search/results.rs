//! `SearchResultsBody` with `SearchResultRow` and `SearchHitRow`, drawn in place of the list.

use std::time::Duration;

use ely_gpui_component::{motion::Spinner, primitives::IconName, theme::IconSize};
use gpui::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, canvas, div, prelude::FluentBuilder as _,
};

use super::{Search, Which, marks, run, trimmed};
use crate::{
    appearance_settings::text_size,
    i18n::{Arg, Translator},
    localdb::search::{
        conversations::shown_title,
        merge::{ContentHit, Item, MatchKind},
    },
    lucide::icon,
    router,
    sessions::{open, row::relative, select},
    theme::Appearance,
    tip::{rich, tip},
    units::css,
};
use hits::{hit_row, more};

/// `MAX_VISIBLE_HITS_PER_RESULT`.
const VISIBLE_HITS: usize = 3;

/// `data-conversation-search-surface`: a press here keeps the query.
fn surface() -> impl IntoElement {
    canvas(
        |bounds, window, cx| {
            let hitbox = window.insert_hitbox(bounds, gpui::HitboxBehavior::Normal);
            cx.global_mut::<Search>().surface = Some(hitbox);
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
}

/// The results while a query runs, else none and the list shows.
pub fn body(w: Which, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let query = trimmed(w, cx).to_owned();
    if query.is_empty() {
        return None;
    }
    let (status, response) = run::state(w, cx);
    let results: Vec<Item> = response.map_or_else(Vec::new, |r| r.results.clone());
    let content = match status {
        run::Status::Searching => searching(cx),
        run::Status::Error => failed(cx),
        _ if results.is_empty() => none(cx),
        _ => div()
            .p(css(8.0))
            .children(
                results
                    .iter()
                    .map(|item| result(w, item, &query, window, cx)),
            )
            .into_any_element(),
    };
    Some(
        div()
            .id("search-results")
            .debug_selector(|| "search-results".into())
            .relative()
            .when(w == Which::Inline, |el| el.child(surface()))
            .child(content)
            .into_any_element(),
    )
}

fn t(key: &str, cx: &App) -> String {
    cx.global::<Translator>().t(key, &[])
}

fn none(cx: &App) -> AnyElement {
    let size = text_size(12.0, cx);
    div()
        .debug_selector(|| "search-none".into())
        .px(css(12.0))
        .py(css(24.0))
        .text_center()
        .text_size(css(size))
        .text_color(cx.global::<Appearance>().token("cmd-palette-empty"))
        .child(t("ccAgent.search.noResults", cx))
        .into_any_element()
}

fn searching(cx: &App) -> AnyElement {
    let size = text_size(12.0, cx);
    let meta = cx.global::<Appearance>().token("cmd-palette-item-meta");
    div()
        .debug_selector(|| "search-searching".into())
        .flex()
        .items_center()
        .justify_center()
        .gap(css(8.0))
        .px(css(12.0))
        .py(css(24.0))
        .text_size(css(size))
        .text_color(meta)
        .child(
            Spinner::new("search-spinner")
                .size(IconSize::Sm)
                .color(meta),
        )
        .child(t("ccAgent.search.searching", cx))
        .into_any_element()
}

/// `ErrorNotice` with a message only, `m-3`.
fn failed(cx: &App) -> AnyElement {
    let appearance = cx.global::<Appearance>();
    let size = text_size(13.0, cx);
    div()
        .debug_selector(|| "search-failed".into())
        .m(css(12.0))
        .flex()
        .items_start()
        .gap(css(8.0))
        .rounded(css(12.0))
        .border(css(1.0))
        .border_color(appearance.token("border-default"))
        .bg(appearance.token("surface-elevated"))
        .p(css(12.0))
        .child(div().mt(css(2.0)).flex_none().child(
            icon(IconName::CircleAlert.svg(), 16.0, 2.0).text_color(appearance.token("error-fg")),
        ))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .text_size(css(size))
                .line_height(css(20.0))
                .text_color(appearance.token("text-primary"))
                .child(t("ccAgent.search.failed", cx)),
        )
        .into_any_element()
}

/// `searchSourceLabel`; no hit has a vector rank before 11.0.
pub(super) fn source(hit: &ContentHit, cx: &App) -> String {
    match hit.fts_rank {
        Some(_) => t("ccAgent.search.source.keyword", cx),
        None => t("ccAgent.search.source.content", cx),
    }
}

pub(super) fn when(at: i64, cx: &App) -> String {
    let translator = cx.global::<Translator>();
    let t = |key: &str, count: i64| translator.t(key, &[("count", Arg::Number(count))]);
    relative(at, jiff::Timestamp::now().as_millisecond(), &t)
}

/// `renderSnippet(snippet) ?? renderKeywordHighlights(preview)`'s text.
pub(super) fn hit_text(hit: &ContentHit) -> &str {
    hit.snippet
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or(&hit.preview)
}

fn kind_key(kind: MatchKind) -> &'static str {
    match kind {
        MatchKind::Title => "ccAgent.search.kind.title",
        MatchKind::Content => "ccAgent.search.kind.content",
        MatchKind::Both => "ccAgent.search.kind.both",
    }
}

/// `SearchResultRow`.
fn result(w: Which, item: &Item, query: &str, window: &mut Window, cx: &mut App) -> AnyElement {
    let s = &item.session;
    let expanded = window.use_keyed_state(
        SharedString::from(format!("search-hits-{}", s.id)),
        cx,
        |_, _| false,
    );
    let shown = *expanded.read(cx);
    let unnamed = t("ccAgent.common.unnamedSession", cx);
    let title = shown_title(&s.title, Some(&unnamed)).to_owned();
    let primary = item.hits.first();
    let visible = match shown {
        true => &item.hits[..],
        false => &item.hits[..item.hits.len().min(VISIBLE_HITS)],
    };
    let hidden = item.hits.len() - visible.len();
    let source_text = primary.map_or_else(
        || t("ccAgent.search.source.titleOnly", cx),
        |h| source(h, cx),
    );
    let companion = s.companion.as_ref().map(|c| c.name.clone());
    let folder = match companion {
        Some(_) => None,
        None => s.working_dir.clone(),
    };
    let meta: Vec<String> = [
        companion,
        Some(source_text.clone()),
        Some(when(s.updated_at, cx)),
        folder,
    ]
    .into_iter()
    .flatten()
    .filter(|p| !p.is_empty())
    .collect();
    let meta = meta.join(" · ");
    let subtitle = match primary {
        Some(hit) => Some(marks::keywords(hit_text(hit), query, cx)),
        None => s
            .working_dir
            .as_deref()
            .filter(|d| !d.is_empty())
            .map(|d| marks::keywords(d, "", cx)),
    };
    let preview = primary
        .filter(|h| !h.preview.is_empty())
        .map(|h| hit_text(h).to_owned());
    let card = tip_card(title.clone(), preview, query.to_owned());
    let hits: Vec<AnyElement> = visible
        .iter()
        .map(|hit| hit_row(w, &s.id, hit, query, window, cx))
        .collect();
    let more = (hidden > 0).then(|| more(&s.id, hidden, expanded, window, cx));
    let head = button(format!("search-result-{}", s.id), window, cx);
    let appearance = cx.global::<Appearance>();
    let (size13, size12, size11, size10) = (
        text_size(13.0, cx),
        text_size(12.0, cx),
        text_size(11.0, cx),
        text_size(10.0, cx),
    );
    let chip = appearance.token("surface-chip");
    let tertiary = appearance.token("text-tertiary");
    let id = s.id.clone();
    let probe = id.clone();
    let chosen = id.clone();
    let head = head
        .on_click(move |_, window, cx| choose(w, &chosen, None, window, cx))
        .flex()
        .w_full()
        .items_start()
        .gap(css(10.0))
        .rounded(css(8.0))
        .px(css(12.0))
        .py(css(10.0))
        .tooltip(card)
        .tooltip_show_delay(Duration::from_millis(250))
        .child(
            div()
                .mt(css(2.0))
                .flex()
                .flex_none()
                .size(css(20.0))
                .items_center()
                .justify_center()
                .rounded(css(9999.0))
                .bg(chip)
                .child(
                    icon(IconName::MessageSquare.svg(), 13.0, 2.0)
                        .text_color(appearance.token("cmd-palette-item-icon")),
                ),
        )
        .child(
            div()
                .min_w_0()
                .flex_1()
                .child(
                    div()
                        .flex()
                        .min_w_0()
                        .items_center()
                        .gap(css(8.0))
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .truncate()
                                .text_size(css(size13))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(appearance.token("text-primary"))
                                .child(marks::title(&title, &item.title_indices, cx)),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!("search-kind-{}", s.id)))
                                .flex_none()
                                .rounded(css(9999.0))
                                .bg(chip)
                                .px(css(6.0))
                                .py(css(2.0))
                                .text_size(css(size10))
                                .line_height(css(size10))
                                .text_color(tertiary)
                                .tooltip(tip(source_text))
                                .tooltip_show_delay(Duration::from_millis(150))
                                .child(t(kind_key(item.kind), cx)),
                        ),
                )
                .when(!meta.is_empty(), |el| {
                    el.child(
                        div()
                            .mt(css(4.0))
                            .truncate()
                            .text_size(css(size11))
                            .line_height(css(size11))
                            .text_color(tertiary)
                            .child(meta),
                    )
                })
                .when_some(subtitle.filter(|_| visible.is_empty()), |el, text| {
                    el.child(
                        div()
                            .mt(css(6.0))
                            .line_clamp(4)
                            .text_size(css(size12))
                            .line_height(css(size12 * 1.375))
                            .text_color(appearance.token("text-secondary"))
                            .child(text),
                    )
                }),
        );
    let hits = (!visible.is_empty()).then(|| {
        div()
            .mt(css(2.0))
            .flex()
            .flex_col()
            .gap(css(4.0))
            .px(css(12.0))
            .pb(css(8.0))
            .pl(css(40.0))
            .children(hits)
            .children(more)
    });
    div()
        .relative()
        .rounded(css(8.0))
        .id(SharedString::from(format!("search-row-{}", s.id)))
        .hover(|st| st.bg(appearance.token("cmd-palette-item-hover")))
        // The popover sits outside the sidebar's root, so its rows are not the sidebar's.
        .when(w == Which::Inline, |el| {
            el.child(canvas(move |_, _, cx| select::drawn(&probe, cx), |_, _, _, _| {}).absolute())
        })
        .child(head)
        .children(hits)
        .into_any_element()
}

/// The row's tip: the title, then the hit's text under a rule.
fn tip_card(
    title: String,
    preview: Option<String>,
    query: String,
) -> impl Fn(&mut Window, &mut App) -> gpui::AnyView {
    rich(500.0, move |_, cx| {
        let appearance = cx.global::<Appearance>();
        let size = text_size(12.0, cx);
        div()
            .flex()
            .flex_col()
            .gap(css(8.0))
            .child(
                div()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child(title.clone()),
            )
            .when_some(preview.as_deref(), |el, text| {
                el.child(
                    div()
                        .border_t(css(1.0))
                        .border_color(appearance.token("cmd-palette-border"))
                        .pt(css(8.0))
                        .text_size(css(size))
                        .line_height(css(size * 1.375))
                        .text_color(appearance.token("tooltip-text"))
                        .child(marks::keywords(text, &query, cx)),
                )
            })
            .into_any_element()
    })
}

/// A `<button>`: a Tab stop that Enter or Space clicks.
pub(super) fn button(id: String, window: &mut Window, cx: &mut App) -> gpui::Stateful<gpui::Div> {
    let focus = window
        .use_keyed_state(SharedString::from(format!("{id}-focus")), cx, |_, cx| {
            cx.focus_handle().tab_stop(true)
        })
        .read(cx)
        .clone();
    div()
        .id(SharedString::from(id.clone()))
        .debug_selector(move || id)
        .track_focus(&focus)
        .role(gpui::Role::Button)
}

/// `handleSelect`: the inline query stays; the popover's `onResultChosen` closes it.
pub(super) fn choose(w: Which, id: &str, hit: Option<&str>, window: &mut Window, cx: &mut App) {
    log::info!(target: "search", "{w:?} result {id} chosen, hit {hit:?}");
    if w == Which::Rail {
        crate::sessions::rail::search::chosen(window, cx);
    }
    router::navigate(&open::route(id), cx);
}

mod hits;

#[cfg(test)]
mod tests;
