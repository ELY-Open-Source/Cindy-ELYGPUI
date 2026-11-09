//! `historyView.ts`'s page joins and the detail walk.

use std::{collections::HashSet, future::Future};

use super::{Message, Snapshot};
use crate::history_view::{
    Item,
    reader::{DetailPage, Page},
};

fn first_source_id<T: Message>(item: &Item<T>) -> Option<&str> {
    match item {
        Item::Work { summary, .. } => Some(&summary.first_message_id),
        Item::Messages { messages, .. } => messages.first().map(Message::id),
    }
}

/// `historyPrefixBoundary`: the older item the page's first item continues.
fn prefix_boundary<T: Message>(page: &Page<T>, previous: &[Item<T>]) -> Option<usize> {
    let first = page.items.first();
    let source = first.and_then(first_source_id);
    previous.iter().position(|item| {
        first.is_some_and(|f| item.key() == f.key())
            || (source.is_some() && first_source_id(item) == source)
    })
}

/// `retainHistoryViewPrefix`: older pages kept only across a proven source boundary.
pub fn retain_prefix<T: Message>(page: Page<T>, previous: &Snapshot<T>) -> Page<T> {
    if !page.has_more || previous.items.is_empty() {
        return page;
    }
    match prefix_boundary(&page, &previous.items) {
        Some(boundary) if boundary > 0 => {
            let mut items = previous.items[..boundary].to_vec();
            items.extend(page.items);
            Page {
                items,
                has_more: previous.has_more,
                next_cursor: previous.next_cursor.clone(),
            }
        }
        _ => page,
    }
}

fn started_at<T: Message>(item: Option<&Item<T>>) -> f64 {
    match item {
        Some(Item::Work { summary, .. }) => summary.started_at_ms,
        Some(Item::Messages { messages, .. }) => messages
            .first()
            .map_or(f64::NAN, |m| crate::js_date::parse(m.created_at())),
        None => crate::js_date::parse(""),
    }
}

/// `readContinuousHistoryPage`: older pages fill a missed gap; `None` once no longer current.
pub async fn continuous<T, F, R>(
    latest: Page<T>,
    previous: &Snapshot<T>,
    mut read_older: R,
    current: impl Fn() -> bool,
) -> Result<Option<Page<T>>, String>
where
    T: Message,
    R: FnMut(String) -> F,
    F: Future<Output = Result<Page<T>, String>>,
{
    let mut page = latest;
    let mut seen = HashSet::new();
    while current()
        && !previous.items.is_empty()
        && page.has_more
        && prefix_boundary(&page, &previous.items).is_none()
        && started_at(page.items.first()) >= started_at(previous.items.first())
    {
        let cursor = match page.next_cursor.clone() {
            Some(cursor) if !seen.contains(&cursor) => cursor,
            _ => return Err("History page cursor did not advance".into()),
        };
        seen.insert(cursor.clone());
        let older = read_older(cursor).await?;
        if !current() {
            return Ok(None);
        }
        let keys: HashSet<&str> = page.items.iter().map(Item::key).collect();
        let mut items: Vec<Item<T>> = older
            .items
            .into_iter()
            .filter(|item| !keys.contains(item.key()))
            .collect();
        items.extend(page.items);
        page = Page {
            items,
            has_more: older.has_more,
            next_cursor: older.next_cursor,
        };
    }
    Ok(current().then_some(page))
}

/// `readHistoryWorkDetails`: every page of a range, while current.
pub async fn work_details<T, F, R>(
    mut read_page: R,
    current: impl Fn() -> bool,
    mut on_page: impl FnMut(Vec<T>),
) -> Result<(), String>
where
    R: FnMut(Option<String>) -> F,
    F: Future<Output = Result<DetailPage<T>, String>>,
{
    let mut cursor = None;
    let mut seen = HashSet::new();
    while current() {
        let page = read_page(cursor.clone()).await?;
        if !current() {
            return Ok(());
        }
        on_page(page.messages);
        if !page.has_more {
            return Ok(());
        }
        match page.next_cursor {
            Some(next) if seen.insert(next.clone()) => cursor = Some(next),
            _ => return Err("History detail cursor did not advance".into()),
        }
    }
    Ok(())
}
