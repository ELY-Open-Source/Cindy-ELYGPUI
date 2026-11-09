//! `reader.details`: a work range's rows, a page at a time.

use std::collections::HashMap;

use super::{
    DETAIL_PAGE_BYTES, Deps, DetailPage, MAX_SCAN_BYTES, MAX_SCAN_ROWS, Opts, Reader, Ref, Result,
    Row, bytes, compare, id, ipc, is_live, rows_bytes, subagent_scopes, text,
};
use crate::history_view::created_ms;

fn page(messages: Vec<Row>, next_cursor: Option<String>) -> DetailPage {
    DetailPage {
        has_more: next_cursor.is_some(),
        messages,
        next_cursor,
    }
}

fn changed() -> super::Error {
    ipc("NOT_FOUND", "History range changed")
}

fn after(cursor: &str, limit: usize) -> Opts {
    Opts {
        limit: Some(limit as f64),
        after: Some(cursor.to_owned()),
        ..Opts::default()
    }
}

/// `finalize`: every exit rechecks the range's ends and rows.
fn finalize(deps: &mut dyn Deps, session: &str, r: &Ref, page: DetailPage) -> Result<DetailPage> {
    let mut ids = vec![r.first.clone(), r.last.clone()];
    ids.extend(page.messages.iter().map(|m| id(m).to_owned()));
    let mut seen = std::collections::HashSet::new();
    ids.retain(|i| seen.insert(i.clone()));
    match deps.validate(session, &ids) {
        Some(result) => result?,
        None => {
            for id in &ids {
                deps.anchor(session, id, true)?;
            }
        }
    }
    Ok(page)
}

impl Reader {
    /// `reader.details`.
    pub fn details(
        &mut self,
        deps: &mut dyn Deps,
        session: &str,
        r: &Ref,
        cursor: Option<&str>,
    ) -> Result<DetailPage> {
        if let Some(parent) = &r.parent {
            let page = self.subagent(deps, session, r, parent, cursor)?;
            return finalize(deps, session, r, page);
        }
        if !r.live.is_empty() {
            return self.live(deps, session, r, cursor);
        }
        let page = plain(deps, session, r, cursor)?;
        finalize(deps, session, r, page)
    }

    /// The subagent branch: its scoped headers once per revision, then hydrated pages.
    fn subagent(
        &mut self,
        deps: &mut dyn Deps,
        session: &str,
        r: &Ref,
        parent: &str,
        cursor: Option<&str>,
    ) -> Result<DetailPage> {
        let outlined = deps.outlines();
        let first = deps.anchor(session, &r.first, outlined)?;
        let last = deps.anchor(session, &r.last, outlined)?;
        if compare(&first, &last) > 0.0 {
            return Err(ipc("INVALID_PARAMS", "Invalid history range"));
        }
        let cursor_anchor = cursor
            .map(|c| deps.anchor(session, c, outlined))
            .transpose()?;
        if let Some(anchor) = &cursor_anchor
            && (compare(anchor, &first) < 0.0 || compare(anchor, &last) > 0.0)
        {
            return Err(ipc("INVALID_PARAMS", "Invalid detail cursor"));
        }
        let revision = match outlined && r.live.is_empty() {
            true => deps.revision().transpose()?,
            false => None,
        };
        if revision != self.detail_revision {
            self.detail_ranges.clear();
            self.detail_bytes = 0;
            self.detail_revision = revision.clone();
        }
        let key = (
            session.to_owned(),
            parent.to_owned(),
            id(&first).to_owned(),
            id(&last).to_owned(),
            r.last_stored.clone(),
        );
        let cached = revision
            .as_ref()
            .and_then(|_| self.detail_ranges.shift_remove(&key));
        let scoped = match cached {
            Some(hit) => {
                let rows = hit.0.clone();
                self.detail_ranges.insert(key, hit);
                rows
            }
            None => {
                let scoped = scope(deps, session, r, parent, &first, &last, outlined)?;
                if let Some(revision) = &revision
                    && deps.revision().transpose()?.as_ref() == Some(revision)
                    && self.detail_revision.as_ref() == Some(revision)
                {
                    self.remember(key, &scoped);
                }
                scoped
            }
        };
        let candidates: Vec<&Row> = scoped
            .iter()
            .filter(|row| cursor_anchor.as_ref().is_none_or(|c| compare(row, c) > 0.0))
            .collect();
        let mut collected: Vec<Row> = Vec::new();
        let mut total = 1024;
        for batch in candidates.chunks(100) {
            let stored: Vec<String> = batch
                .iter()
                .map(|r| id(r).to_owned())
                .filter(|i| !is_live(i))
                .collect();
            let full: HashMap<String, Row> = match outlined {
                true => deps.hydrate(session, &stored)?,
                false => batch
                    .iter()
                    .filter(|r| !is_live(id(r)))
                    .map(|r| (*r).clone())
                    .collect(),
            }
            .into_iter()
            .map(|row| (id(&row).to_owned(), row))
            .collect();
            for header in batch {
                let row = full.get(id(header)).unwrap_or(header).clone();
                let size = bytes(&row);
                if !collected.is_empty() && total + size > DETAIL_PAGE_BYTES {
                    let next = id(collected.last().expect("a collected row")).to_owned();
                    return Ok(page(collected, Some(next)));
                }
                collected.push(row);
                total += size;
            }
        }
        Ok(page(collected, None))
    }

    /// A scoped range for this revision; at most two ranges and the scan's byte budget.
    fn remember(&mut self, key: super::DetailKey, scoped: &[Row]) {
        let size = rows_bytes(scoped);
        if size > MAX_SCAN_BYTES {
            return;
        }
        while !self.detail_ranges.is_empty()
            && (self.detail_ranges.len() >= 2 || self.detail_bytes + size > MAX_SCAN_BYTES)
        {
            let (_, (_, oldest)) = self.detail_ranges.shift_remove_index(0).expect("a range");
            self.detail_bytes -= oldest;
        }
        self.detail_bytes -= self.detail_ranges.get(&key).map_or(0, |e| e.1);
        self.detail_ranges.insert(key, (scoped.to_vec(), size));
        self.detail_bytes += size;
    }

    /// The live branch: the stored part first, then each live row by its anchor.
    fn live(
        &mut self,
        deps: &mut dyn Deps,
        session: &str,
        r: &Ref,
        cursor: Option<&str>,
    ) -> Result<DetailPage> {
        let live_cursor = cursor.and_then(|c| r.live.iter().position(|l| l == c));
        let stored = match (live_cursor, &r.first_stored, &r.last_stored) {
            (None, Some(first), Some(last)) => {
                let range = Ref {
                    key: r.key.clone(),
                    first: first.clone(),
                    last: last.clone(),
                    ..Ref::default()
                };
                self.details(deps, session, &range, cursor)?
            }
            _ => page(Vec::new(), None),
        };
        if stored.has_more {
            return finalize(deps, session, r, stored);
        }
        let mut collected = stored.messages;
        let mut total = rows_bytes(&collected) + 1024;
        let mut next = collected
            .last()
            .map(|m| id(m).to_owned())
            .or(cursor.map(str::to_owned))
            .or(r.last_stored.clone());
        for live in &r.live[live_cursor.map_or(0, |c| c + 1)..] {
            let row = deps.anchor(session, live, false)?;
            if collected
                .iter()
                .any(|c| text(c, "clientId") == text(&row, "clientId"))
            {
                continue;
            }
            let size = bytes(&row);
            if !collected.is_empty() && total + size > DETAIL_PAGE_BYTES {
                let more = DetailPage {
                    messages: collected,
                    has_more: true,
                    next_cursor: next,
                };
                return finalize(deps, session, r, more);
            }
            collected.push(row);
            total += size;
            next = Some(live.clone());
        }
        finalize(deps, session, r, page(collected, None))
    }
}

/// The subagent's headers: stored rows through the range, then its live rows, scoped.
fn scope(
    deps: &mut dyn Deps,
    session: &str,
    r: &Ref,
    parent: &str,
    first: &Row,
    last: &Row,
    outlined: bool,
) -> Result<Vec<Row>> {
    let mut headers: Vec<Row> = Vec::new();
    let mut full_bytes = 0;
    let mut append = |headers: &mut Vec<Row>, rows: Vec<Row>| {
        if !outlined {
            full_bytes += rows_bytes(&rows);
        }
        let budget = if outlined { 100_000 } else { MAX_SCAN_ROWS };
        if headers.len() + rows.len() > budget || full_bytes > MAX_SCAN_BYTES {
            return Err(ipc("INVALID_PARAMS", "History detail range is too large"));
        }
        headers.extend(rows);
        Ok(())
    };
    if !is_live(id(first)) {
        append(&mut headers, vec![first.clone()])?;
        let mut cursor = id(first).to_owned();
        let limit = if outlined { 1000 } else { 100 };
        let end = r.last_stored.as_deref().unwrap_or(id(last)).to_owned();
        while cursor != end {
            let opts = after(&cursor, limit);
            let mut batch = match outlined {
                true => deps.outline(session, &opts)?,
                false => deps.list(session, &opts)?,
            };
            batch.reverse();
            let Some(tail) = batch.last() else { break };
            let next = id(tail).to_owned();
            let read = batch.len();
            let bounded: Vec<Row> = batch
                .into_iter()
                .filter(|row| compare(row, last) <= 0.0)
                .collect();
            let cut = bounded.len() < read;
            append(&mut headers, bounded)?;
            if next == cursor {
                return Err(ipc("INTERNAL", "History detail cursor did not advance"));
            }
            cursor = next;
            if cut || read < limit {
                break;
            }
        }
    }
    for live in &r.live {
        let row = deps.anchor(session, live, false)?;
        if !headers
            .iter()
            .any(|h| text(h, "clientId") == text(&row, "clientId"))
        {
            append(&mut headers, vec![row])?;
        }
    }
    let scopes = subagent_scopes(&headers, Some(parent));
    headers.retain(|row| scopes.get(id(row)).map(String::as_str) == Some(parent));
    Ok(headers)
}

/// The plain branch: rows after the cursor up to the range's last, which must hold still.
fn plain(deps: &mut dyn Deps, session: &str, r: &Ref, cursor: Option<&str>) -> Result<DetailPage> {
    let first = deps.anchor(session, &r.first, false)?;
    let last = deps.anchor(session, &r.last, false)?;
    if compare(&first, &last) > 0.0 {
        return Err(ipc("INVALID_PARAMS", "Invalid history range"));
    }
    let mut collected: Vec<Row> = Vec::new();
    let mut total = 1024;
    let mut cursor = match cursor {
        None => {
            total += bytes(&first);
            let first_id = id(&first).to_owned();
            collected.push(first);
            first_id
        }
        Some(c) => {
            let anchor = deps.anchor(session, c, false)?;
            if compare(&anchor, &first) < 0.0 || compare(&anchor, &last) > 0.0 {
                return Err(ipc("INVALID_PARAMS", "Invalid detail cursor"));
            }
            c.to_owned()
        }
    };
    if cursor == id(&last) {
        return Ok(page(collected, None));
    }
    loop {
        let mut rows = deps.list(session, &after(&cursor, 100))?;
        rows.reverse();
        if rows.is_empty() {
            return Err(changed());
        }
        for row in rows {
            if compare(&row, &last) > 0.0 {
                return Err(changed());
            }
            let size = bytes(&row);
            if !collected.is_empty() && total + size > DETAIL_PAGE_BYTES {
                return Ok(page(collected, Some(cursor)));
            }
            if id(&row) == cursor {
                return Err(ipc("INTERNAL", "History detail cursor did not advance"));
            }
            cursor = id(&row).to_owned();
            let past = created_ms(&row) > created_ms(&last);
            total += size;
            collected.push(row);
            if cursor == id(&last) {
                return Ok(page(collected, None));
            }
            if past {
                return Err(changed());
            }
        }
    }
}
