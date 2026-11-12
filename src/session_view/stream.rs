//! `MessageStream`'s list: the scroller, its column and a row per render item.

use gpui::{
    AnyElement, App, InteractiveElement as _, IntoElement as _, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _,
};

use super::{
    assistant_row, attachments, items::items, jump, link_chip, select, user_row, width::Widths,
};
use crate::{
    chat_message::merge::get, history_render::Projection, markdown_target::resolved,
    render_items::Item, rich_answer::js::Js, units::css, workspace_media,
};

/// `collectStableLocalFileRefs`: the files the task's user messages attached, name then path.
fn attached(projection: &Projection) -> Vec<(String, String)> {
    let rows = projection.rows.iter();
    let rows = rows.filter(|row| get(row, "role").and_then(Js::str) == Some("user"));
    let files = rows.flat_map(|row| attachments::files(row));
    files
        .map(|(name, path)| (name.to_owned(), path.to_owned()))
        .collect()
}

/// A render item's row: `flex flex-col gap-2` under `data-render-item-key`.
fn row(
    item: &Item,
    (projection, files): (&Projection, &[(String, String)]),
    widths: Widths,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyElement> {
    // The other items come with 9.7 to 9.13.
    let Item::Message { key, message } = item else {
        return None;
    };
    let message = &projection.rows[*message];
    let bubble = match get(message, "role").and_then(Js::str) {
        Some("user") => user_row::element(key, message, widths, window, cx)?,
        Some("assistant") => assistant_row::element(key, message, files, widths, window, cx)?,
        _ => return None,
    };
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
    let files = projection.as_deref().map(attached).unwrap_or_default();
    let handle = jump::begin(window, cx);
    link_chip::begin(cx);
    select::begin(cx);
    resolved::begin(cx);
    workspace_media::begin(cx);
    let rows: Vec<AnyElement> = projection
        .iter()
        .flat_map(|p| p.items.iter().map(move |i| (i, p)))
        .filter_map(|(item, p)| row(item, (p, &files), widths, window, cx))
        .collect();
    resolved::end(cx);
    workspace_media::end(cx);
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
    let scroller = div()
        .id("session-scroll")
        .debug_selector(|| "session-scroll".into())
        .size_full()
        .overflow_y_scroll()
        .when_some(handle, |el, handle| el.track_scroll(&handle));
    select::listen(scroller).child(content).into_any_element()
}
