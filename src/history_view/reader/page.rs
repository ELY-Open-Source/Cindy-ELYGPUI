//! `reader.page`: scan back to a visible page, then cut it.

use std::collections::HashSet;

use super::{
    Deps, Item, MAX_SCAN_BYTES, MAX_SCAN_ROWS, Opts, PAGE_BYTES, PAGE_ITEMS, Page, Reader, Result,
    Row, bytes, cuts_tool_context, each_messages, each_summary, first_id, id, ipc, is_boundary,
    is_live, json, page_start, safe_starts, text,
};
use crate::history_view::{completes_turn, project};

fn unavailable() -> super::Error {
    ipc(
        "UNSUPPORTED_CAPABILITY",
        "History view scan budget exceeded",
    )
}

/// The scan's running totals and budget.
struct Scan {
    rows: usize,
    bytes: usize,
    row_budget: usize,
}

impl Scan {
    /// `liveRows`: in-flight rows not yet stored, under the same budget.
    fn live(
        &self,
        deps: &dyn Deps,
        session: &str,
        before: bool,
        stored: &[Row],
    ) -> Result<Vec<Row>> {
        if before {
            return Ok(Vec::new());
        }
        let ids: HashSet<&str> = stored.iter().map(|r| text(r, "clientId")).collect();
        let mut live = Vec::new();
        let mut total = self.bytes;
        for row in deps.live(session) {
            if ids.contains(text(&row, "clientId")) {
                continue;
            }
            total += bytes(&row);
            if self.rows + live.len() + 1 > self.row_budget
                || live.len() >= MAX_SCAN_ROWS
                || total > MAX_SCAN_BYTES
            {
                return Err(unavailable());
            }
            live.push(row);
        }
        Ok(live)
    }
}

/// The page's projection from its first boundary, with the cursor's sealed answer as context.
fn project_from(
    raw: &[Row],
    live: &[Row],
    exhausted: bool,
    context: Option<&Row>,
    streaming: bool,
    lazy: bool,
) -> Vec<Item> {
    let boundary = match exhausted {
        true => 0,
        false => match raw.iter().position(is_boundary) {
            Some(at) => at,
            None => return Vec::new(),
        },
    };
    let all: Vec<Row> = raw.iter().chain(live).cloned().collect();
    if !exhausted
        && text(&raw[boundary], "role") == "assistant"
        && cuts_tool_context(&all, boundary)
    {
        return Vec::new();
    }
    let mut rows: Vec<Row> = all[boundary..].to_vec();
    rows.extend(context.cloned());
    let mut items = project(&rows, streaming, lazy);
    if let Some(context) = context {
        items.retain(|item| first_id(item) != id(context));
    }
    items
}

/// A page that needs no older batch: twenty items or its bytes.
pub(super) fn full(items: &[Item]) -> bool {
    items.len() >= PAGE_ITEMS || items_bytes(items) + 1024 >= PAGE_BYTES
}

fn items_bytes(items: &[Item]) -> usize {
    2 + items.iter().map(|i| bytes(&json::item(i))).sum::<usize>() + items.len().saturating_sub(1)
}

impl Reader {
    /// `reader.page`.
    pub fn page(
        &mut self,
        deps: &mut dyn Deps,
        session: &str,
        before: Option<&str>,
        lazy: bool,
    ) -> Result<Page> {
        let outlined = lazy && deps.outlines();
        let batch = if outlined { 1000 } else { 100 };
        let mut scan = Scan {
            rows: 0,
            bytes: 0,
            row_budget: if outlined { 100_000 } else { MAX_SCAN_ROWS },
        };
        let outline_revision = match outlined {
            false => None,
            true => {
                let revision = match deps.revision() {
                    Some(revision) => revision?,
                    None => {
                        self.read_revision += 1;
                        self.read_revision.to_string()
                    }
                };
                Some(format!("{}:{revision}", self.epoch))
            }
        };
        let before_anchor = before
            .map(|b| deps.anchor(session, b, outlined))
            .transpose()?;
        let mut cursor = before_anchor
            .as_ref()
            .map(|a| id(a).to_owned())
            .filter(|a| !is_live(a));
        let context = before_anchor
            .as_ref()
            .filter(|a| text(a, "role") == "assistant" && completes_turn(a));
        let streaming = before.is_none() && deps.running(session);
        let mut chunks: Vec<Vec<Row>> = Vec::new();
        let (mut raw, mut live, mut items) = (Vec::new(), Vec::new(), Vec::new());
        let mut exhausted;
        let mut projected;
        let joined =
            |chunks: &[Vec<Row>]| chunks.iter().rev().flatten().cloned().collect::<Vec<_>>();
        loop {
            if scan.rows >= scan.row_budget {
                return Err(unavailable());
            }
            let opts = Opts {
                limit: Some(batch as f64),
                before: cursor.clone(),
                ..Opts::default()
            };
            let rows = match outlined {
                true => deps.outline(session, &opts)?,
                false => deps.list(session, &opts)?,
            };
            projected = false;
            for row in &rows {
                scan.rows += 1;
                scan.bytes += bytes(row);
                if scan.rows > scan.row_budget || scan.bytes > MAX_SCAN_BYTES {
                    return Err(unavailable());
                }
            }
            let Some(last) = rows.last() else {
                exhausted = true;
                break;
            };
            let next = id(last).to_owned();
            if cursor.as_deref() == Some(next.as_str()) {
                return Err(ipc("INTERNAL", "History cursor did not advance"));
            }
            cursor = Some(next);
            exhausted = rows.len() < batch;
            let bounded = rows.iter().any(is_boundary);
            chunks.push(rows.into_iter().rev().collect());
            if !exhausted && !bounded {
                continue;
            }
            raw = joined(&chunks);
            live = scan.live(deps, session, before.is_some(), &raw)?;
            items = project_from(&raw, &live, exhausted, context, streaming, lazy);
            projected = true;
            if exhausted || full(&items) {
                break;
            }
        }
        if !projected {
            raw = joined(&chunks);
            live = scan.live(deps, session, before.is_some(), &raw)?;
            items = project_from(&raw, &live, exhausted, context, streaming, lazy);
        }
        let all: Vec<Row> = raw.iter().chain(&live).cloned().collect();
        let safe = safe_starts(&all);
        if outlined {
            let from = page_start(&items, &safe, false)?;
            let mut ids: Vec<String> = Vec::new();
            each_messages(&mut items[from..], &mut |rows| {
                ids.extend(
                    rows.iter()
                        .map(id)
                        .filter(|i| !is_live(i))
                        .map(str::to_owned),
                );
            });
            let mut seen = HashSet::new();
            ids.retain(|i| seen.insert(i.clone()));
            let hydrated: std::collections::HashMap<String, Row> = deps
                .hydrate(session, &ids)?
                .into_iter()
                .map(|r| (id(&r).to_owned(), r))
                .collect();
            each_messages(&mut items[from..], &mut |rows| {
                for row in rows.iter_mut() {
                    if let Some(full) = hydrated.get(id(row)) {
                        *row = full.clone();
                    }
                }
            });
        }
        if let Some(revision) = &outline_revision {
            each_summary(&mut items, &mut |s| {
                s.revision = format!("{}:{revision}", s.revision)
            });
        }
        let start = page_start(&items, &safe, true)?;
        let has_more = !exhausted || start > 0;
        let mut anchors: Vec<String> = match (raw.first(), raw.last()) {
            (Some(first), Some(last)) => vec![id(first).to_owned(), id(last).to_owned()],
            _ => Vec::new(),
        };
        anchors.extend(before_anchor.as_ref().map(|a| id(a).to_owned()));
        for anchor in &anchors {
            deps.anchor(session, anchor, outlined)?;
        }
        let selected: Vec<Item> = items.split_off(start);
        let next_cursor = selected
            .first()
            .filter(|_| has_more)
            .map(|item| first_id(item).to_owned());
        log::debug!(target: "localDb", "history page {} items, {} rows scanned, more {has_more}", selected.len(), scan.rows);
        Ok(Page {
            items: selected,
            has_more,
            next_cursor,
        })
    }
}
