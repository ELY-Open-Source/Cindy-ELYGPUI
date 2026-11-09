//! `SearchHitRow` and the `moreHits` button under a result.

use gpui::{
    AnyElement, App, InteractiveElement as _, IntoElement as _, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Window, div,
};

use super::{Which, button, choose, hit_text, marks, source, when};
use crate::{
    appearance_settings::text_size,
    i18n::{Arg, Translator},
    localdb::search::merge::ContentHit,
    theme::Appearance,
    units::css,
};

/// The hit and More box; `strong` is the text a hover gives.
fn hit_box(
    id: String,
    strong: Option<gpui::Hsla>,
    window: &mut Window,
    cx: &mut App,
) -> gpui::Stateful<gpui::Div> {
    let button = button(id, window, cx);
    let appearance = cx.global::<Appearance>();
    let (border, elevated) = (
        appearance.token("border-default"),
        appearance.token("surface-elevated"),
    );
    button
        .flex()
        .w_full()
        .items_start()
        .rounded(css(6.0))
        .border(css(1.0))
        .border_color(gpui::transparent_black())
        .px(css(8.0))
        .py(css(6.0))
        .hover(move |s| {
            let s = s.border_color(border).bg(elevated);
            match strong {
                Some(color) => s.text_color(color),
                None => s,
            }
        })
}

/// `SearchHitRow`.
pub(super) fn hit_row(
    w: Which,
    session: &str,
    hit: &ContentHit,
    query: &str,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let row = hit_box(format!("search-hit-{}", hit.message_id), None, window, cx);
    let appearance = cx.global::<Appearance>();
    let (size12, size10) = (text_size(12.0, cx), text_size(10.0, cx));
    let meta = format!("{} · {}", when(hit.created_at, cx), source(hit, cx));
    let (session, message) = (session.to_owned(), hit.message_id.clone());
    row.on_click(move |_, window, cx| {
        cx.stop_propagation();
        choose(w, &session, Some(&message), window, cx);
    })
    .child(
        div()
            .min_w_0()
            .flex_1()
            .child(
                div()
                    .line_clamp(2)
                    .text_size(css(size12))
                    .line_height(css(size12 * 1.375))
                    .text_color(appearance.token("text-secondary"))
                    .child(marks::keywords(hit_text(hit), query, cx)),
            )
            .child(
                div()
                    .mt(css(2.0))
                    .truncate()
                    .text_size(css(size10))
                    .line_height(css(size10))
                    .text_color(appearance.token("text-tertiary"))
                    .child(meta),
            ),
    )
    .into_any_element()
}

/// The `moreHits` button: every hit of the row shows.
pub(super) fn more(
    id: &str,
    hidden: usize,
    expanded: gpui::Entity<bool>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let strong = cx.global::<Appearance>().token("text-primary");
    let row = hit_box(format!("search-more-{id}"), Some(strong), window, cx);
    let appearance = cx.global::<Appearance>();
    let size = text_size(10.0, cx);
    let label = cx.global::<Translator>().t(
        "ccAgent.search.moreHits",
        &[("count", Arg::Number(hidden as i64))],
    );
    row.text_size(css(size))
        .line_height(css(size))
        .text_color(appearance.token("text-tertiary"))
        .on_click(move |_, _, cx| {
            cx.stop_propagation();
            expanded.update(cx, |shown, cx| {
                *shown = true;
                cx.notify();
            });
            log::debug!(target: "search", "every hit shown");
        })
        .child(label)
        .into_any_element()
}
