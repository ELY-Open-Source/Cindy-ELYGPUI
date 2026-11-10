//! `MessageStream`'s list: the scroller, its column and a row per render item.

use gpui::{
    AnyElement, App, InteractiveElement as _, IntoElement as _, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Window, div,
};

use super::{items::items, link_chip, user_row, width::Widths};
use crate::{
    chat_message::merge::get, history_render::Projection, render_items::Item, rich_answer::js::Js,
    units::css,
};

/// A render item's row: `flex flex-col gap-2` under `data-render-item-key`.
fn row(
    item: &Item,
    projection: &Projection,
    widths: Widths,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyElement> {
    // The other items come with 9.5g to 9.13.
    let Item::Message { key, message } = item else {
        return None;
    };
    let message = &projection.rows[*message];
    if get(message, "role").and_then(Js::str) != Some("user") {
        return None;
    }
    let bubble = user_row::element(key, message, widths, window, cx)?;
    let selector = format!("item-{key}");
    let row = div()
        .debug_selector(move || selector)
        .flex()
        .flex_col()
        .gap(css(8.0))
        .child(bubble);
    Some(row.into_any_element())
}

/// The scroll container, `mx-auto pt-7` at `messageWidth`, then `msg-stream-items`.
pub fn element(widths: Widths, window: &mut Window, cx: &mut App) -> AnyElement {
    let projection = items(cx).and_then(|items| items.as_ref().ok()).cloned();
    link_chip::begin(cx);
    let rows: Vec<AnyElement> = projection
        .iter()
        .flat_map(|p| p.items.iter().map(move |i| (i, p)))
        .filter_map(|(item, p)| row(item, p, widths, window, cx))
        .collect();
    link_chip::end(cx);
    let list = div().flex().flex_col().gap(css(14.0)).children(rows);
    let content = div()
        .debug_selector(|| "session-messages".into())
        .relative()
        .mx_auto()
        .w_full()
        .max_w(css(widths.message))
        .pt(css(28.0))
        .child(list);
    div()
        .id("session-scroll")
        .debug_selector(|| "session-scroll".into())
        .size_full()
        .overflow_y_scroll()
        .child(content)
        .into_any_element()
}
