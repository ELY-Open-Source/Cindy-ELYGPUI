//! The store's window helpers: the message timeline, raw pages and the loaded islands.

use std::cmp::Ordering;

use crate::{
    chat_message::{
        self,
        merge::{self, Message, RowsOrder},
    },
    message_rows::UI_ACTION_TRIGGER_PREFIX,
    rich_answer::js::{self, Js, Obj},
};

/// `LoadedWindowIsland`.
#[derive(Clone, Debug, PartialEq)]
pub struct Island {
    pub oldest_client_id: String,
    pub newest_client_id: String,
}

/// The two fields `compareMessageTimeline` reads.
#[derive(Clone, Copy)]
pub struct Stamp<'a> {
    created_at: Option<&'a Js>,
    rowid: Option<&'a Js>,
}

impl<'a> Stamp<'a> {
    pub fn raw(row: &'a Obj) -> Self {
        Stamp {
            created_at: row.get("createdAt"),
            rowid: row.get("rowid"),
        }
    }

    pub fn message(row: &'a Message) -> Self {
        Stamp {
            created_at: merge::get(row, "createdAt"),
            rowid: merge::get(row, "rowid"),
        }
    }

    /// `messageTime`: a missing or unreadable time is the end of the line.
    fn time(self) -> f64 {
        if !js::truthy(self.created_at) {
            return f64::INFINITY;
        }
        let ms = chat_message::time(self.created_at);
        if ms.is_finite() { ms } else { f64::INFINITY }
    }

    /// `messageRowid`.
    fn rowid(self) -> f64 {
        match self.rowid {
            Some(Js::Num(n)) if n.is_finite() => *n,
            _ => f64::NAN,
        }
    }
}

/// `compareMessageTimeline`'s first two keys; `None` when neither decides.
fn decide(a: Stamp, b: Stamp) -> Option<f64> {
    [a.time() - b.time(), a.rowid() - b.rowid()]
        .into_iter()
        .find(|d| !d.is_nan() && *d != 0.0)
}

/// `compareMessageTimeline`.
pub fn compare(a: Stamp, b: Stamp) -> Ordering {
    decide(a, b).map_or(Ordering::Equal, |d| d.total_cmp(&0.0))
}

/// `oldestMessageRow`: ties go to the earlier row for `OldestFirst`, the later otherwise.
pub fn oldest_row(rows: &[Js], order: RowsOrder) -> Option<&Obj> {
    let mut ranked: Vec<(usize, &Obj)> = rows.iter().filter_map(Js::obj).enumerate().collect();
    crate::js_sort::sort(&mut ranked, |(ai, a), (bi, b)| {
        let index = match order {
            RowsOrder::OldestFirst => ai as f64 - bi as f64,
            RowsOrder::NewestFirst => bi as f64 - ai as f64,
        };
        decide(Stamp::raw(a), Stamp::raw(b)).unwrap_or(index) < 0.0
    });
    ranked.first().map(|(_, row)| *row)
}

/// `newestMessageRowForWindow`.
pub fn newest_row(rows: &[Js]) -> Option<&Obj> {
    let mut newest: Option<&Obj> = None;
    for row in rows.iter().filter_map(Js::obj) {
        if !Stamp::raw(row).time().is_finite() {
            continue;
        }
        if newest.is_none_or(|n| compare(Stamp::raw(row), Stamp::raw(n)).is_gt()) {
            newest = Some(row);
        }
    }
    newest
}

/// `serverMessagePageHasMore` with its page of 50.
pub fn page_has_more(rows: &[Js]) -> bool {
    rows.len() >= 50 || rows.iter().any(trimmed)
}

/// A device-link page cut short: `agentMeta.remoteRowsTrimmed`.
pub fn trimmed(row: &Js) -> bool {
    let meta = row.obj().and_then(|r| r.get("agentMeta")).and_then(Js::obj);
    meta.and_then(|m| m.get("remoteRowsTrimmed")) == Some(&Js::Bool(true))
}

/// `isSyntheticTriggerRow`.
pub fn synthetic_trigger(row: &Obj) -> bool {
    if row.get("role").and_then(Js::str) != Some("user") {
        return false;
    }
    let text = match row.get("content") {
        Some(Js::Str(s)) => Some(s.as_str()),
        Some(Js::Obj(c)) => c.get("text").and_then(Js::str),
        _ => None,
    };
    text.is_some_and(|t| t.starts_with(UI_ACTION_TRIGGER_PREFIX))
}

pub fn text<'a>(row: &'a Message, key: &str) -> Option<&'a str> {
    merge::get(row, key).and_then(Js::str)
}

fn index_of(messages: &[Message], client: &str) -> Option<usize> {
    messages
        .iter()
        .position(|m| text(m, "clientId") == Some(client))
}

/// `mainContiguousRunStartIndex`.
fn main_run_start(messages: &[Message], islands: &[Island]) -> usize {
    let Some(newest) = islands.last() else {
        return 0;
    };
    index_of(messages, &newest.newest_client_id).map_or(messages.len(), |i| i + 1)
}

/// `oldestMessageOfMainContiguousRun`.
pub fn main_run_oldest<'a>(messages: &'a [Message], islands: &[Island]) -> Option<&'a Message> {
    messages.get(main_run_start(messages, islands))
}

/// `conservativeIslandsFor`: the whole window counts as detached.
pub fn conservative(messages: &[Message]) -> Vec<Island> {
    let newest = messages.last().and_then(|m| text(m, "clientId"));
    newest
        .map(|id| Island {
            oldest_client_id: id.into(),
            newest_client_id: id.into(),
        })
        .into_iter()
        .collect()
}

/// `absorbIslandsCrossedByPaging`; a missing island edge before the boundary throws, as in Cindy.
pub fn absorb(
    islands: &[Island],
    messages: &[Message],
    boundary: Option<&str>,
) -> Result<Vec<Island>, String> {
    let Some(boundary) = boundary.filter(|b| !islands.is_empty() && !b.is_empty()) else {
        return Ok(islands.to_vec());
    };
    let matches =
        |m: &Message| text(m, "clientId") == Some(boundary) || text(m, "id") == Some(boundary);
    let Some(at) = messages.iter().position(matches) else {
        return Ok(islands.to_vec());
    };
    let mut next = Vec::new();
    for island in islands {
        // `findIndex`'s -1 sorts before every boundary.
        let index = |id: &str| index_of(messages, id).map_or(-1, |i| i as isize);
        if index(&island.oldest_client_id) >= at as isize {
            continue;
        }
        if index(&island.newest_client_id) >= at as isize {
            let Some(before) = at.checked_sub(1).map(|i| &messages[i]) else {
                return Err("Cannot read properties of undefined (reading 'clientId')".into());
            };
            let newest = text(before, "clientId").unwrap_or_default();
            next.push(Island {
                oldest_client_id: island.oldest_client_id.clone(),
                newest_client_id: newest.into(),
            });
            continue;
        }
        next.push(island.clone());
    }
    Ok(next)
}
