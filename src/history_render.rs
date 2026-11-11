//! `renderHistoryView` as `buildMessageStreamRenderProjection` calls it for a ready view.

use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

use indexmap::{IndexMap, IndexSet};

use crate::{
    chat_message::merge::{Fields, Message, get, strict_field},
    history_view::{
        Item as Leaf, Summary,
        controller::{Detail, Message as _, Snapshot, View, leaves, summaries},
    },
    js_date,
    markdown::image_targets,
    render_items::{self, Deferred, Item, Options},
    rich_answer::js::{Js, truthy},
    work_groups,
};

mod bind;

const LIVE: &str = "history-live:";

/// What the stream's projection reads besides the view.
pub struct Input<'a> {
    pub snapshot: &'a Snapshot<Message>,
    pub live: &'a [Message],
    pub streaming: bool,
    /// `handoff.pending`, in observation order.
    pub pending: &'a IndexSet<String>,
    /// `handoff.tail`.
    pub tail: &'a IndexSet<String>,
    pub working_dir: &'a str,
    /// `markdownImageTargetCache`, which the stream's mount holds.
    pub image_targets: Option<&'a image_targets::Cache>,
}

/// The items with the rows they index.
#[derive(Debug)]
pub struct Projection {
    pub rows: Vec<Message>,
    pub items: Vec<Rc<Item>>,
}

fn is_true(row: &Fields, key: &str) -> bool {
    get(row, key) == Some(&Js::Bool(true))
}

fn role(row: &Fields) -> &str {
    get(row, "role").and_then(Js::str).unwrap_or_default()
}

/// The stream's `isLocalUser` or `isLocalMessage`.
fn local(row: &Fields) -> bool {
    let waits = is_true(row, "isPendingPersist")
        || truthy(get(row, "blockedByGhost"))
        || truthy(get(row, "localSendPrecedingClientIds"));
    (role(row) == "user" && waits) || is_true(row, "isLocalSystemCard")
}

/// `Math.max`, where one `NaN` makes `NaN`.
fn max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        return f64::NAN;
    }
    a.max(b)
}

fn created(row: &Message) -> f64 {
    js_date::parse(row.created_at())
}

/// `liveContentWithHistoryOrder`.
fn in_history_order(live: &Message, history: &Message) -> Message {
    if strict_field(live, history, "createdAt") && strict_field(live, history, "rowid") {
        return live.clone();
    }
    let mut row = (**live).clone();
    for key in ["createdAt", "rowid"] {
        row.insert(key.to_owned(), history.get(key).cloned().flatten());
    }
    Rc::new(row)
}

/// The stream's `placeholder`: a redacted thinking row that stands for an unread range.
fn placeholder(summary: &Summary) -> Message {
    // The projection writes a time `Date.parse` gave, or 0.
    let started = js_date::iso(summary.started_at_ms as i64).expect("a time in range");
    let took = max(0.0, summary.ended_at_ms - summary.started_at_ms);
    let text = |text: &str| Js::Str(text.to_owned());
    let fields = [
        ("id", text(&summary.first_message_id)),
        ("clientId", text(&summary.anchor_client_id)),
        ("role", text("thinking")),
        ("content", text("")),
        ("createdAt", text(&started)),
        ("thinkingDurationMs", Js::Num(took)),
        ("thinkingRedacted", Js::Bool(true)),
        ("isStreaming", Js::Bool(summary.is_streaming)),
    ];
    let fields = fields.map(|(key, value)| (key.to_owned(), Some(Rc::new(value))));
    Rc::new(Fields::from_iter(fields))
}

/// A cached range from its first to its last message; to the end when the last is not there.
fn range<'a>(cached: &'a [Message], first: &str, last: &str) -> &'a [Message] {
    let at = |id: &str| {
        let live = id.strip_prefix(LIVE);
        cached
            .iter()
            .position(|row| row.id() == id || live == Some(row.client_id()))
    };
    match (at(first), at(last)) {
        (None, _) => &[],
        (Some(first), Some(last)) if last >= first => &cached[first..=last],
        (Some(first), _) => &cached[first..],
    }
}

fn cached<'a>(details: &'a IndexMap<String, Detail<Message>>, key: &str) -> &'a [Message] {
    details.get(key).map_or(&[], |state| &state.messages)
}

/// The rows the builder reads, with what `bind` needs to tie groups to their ranges.
struct Rows<'a> {
    rows: Vec<Message>,
    placeholders: HashSet<String>,
    references: HashMap<String, &'a Summary>,
}

struct Live<'a> {
    input: &'a Input<'a>,
}

impl Live<'_> {
    fn pending(&self, row: &Message) -> bool {
        role(row) == "assistant" && self.input.pending.contains(row.client_id())
    }

    fn stale(&self, row: &Message) -> bool {
        self.input.tail.contains(row.client_id()) && !local(row)
    }

    fn is(&self, row: &Message) -> bool {
        is_true(row, "isStreaming") || self.pending(row) || self.stale(row)
    }
}

fn rows<'a>(input: &'a Input<'a>) -> Rows<'a> {
    let Input { snapshot, .. } = input;
    let is = Live { input };
    let live: HashMap<&str, &Message> = input
        .live
        .iter()
        .filter(|row| is.is(row))
        .map(|row| (row.client_id(), row))
        .collect();
    let leaves = leaves(&snapshot.items);
    let page: HashSet<&str> = leaves
        .iter()
        .flat_map(|leaf| match leaf {
            Leaf::Messages { messages, .. } => messages.as_slice(),
            Leaf::Work { .. } => &[],
        })
        .map(|row| row.client_id())
        .collect();
    let mut out = Rows {
        rows: Vec::new(),
        placeholders: HashSet::new(),
        references: HashMap::new(),
    };
    let mut seen: HashSet<String> = HashSet::new();
    let mut end_ms = 0.0;
    for leaf in leaves {
        match leaf {
            Leaf::Messages {
                messages, deferred, ..
            } => {
                for row in messages {
                    let current = live.get(row.client_id());
                    out.rows.push(
                        current.map_or_else(|| row.clone(), |live| in_history_order(live, row)),
                    );
                    seen.insert(row.client_id().to_owned());
                    end_ms = max(end_ms, created(row));
                }
                let Some(summary) = deferred else { continue };
                out.references
                    .insert(summary.anchor_client_id.clone(), summary);
                let body = range(
                    cached(&snapshot.details, &summary.key),
                    &summary.first_message_id,
                    &summary.last_message_id,
                );
                for row in body {
                    if !page.contains(row.client_id()) && seen.insert(row.client_id().to_owned()) {
                        out.rows.push(row.clone());
                    }
                }
            }
            Leaf::Work { summary, .. } => {
                let full = snapshot.details.get(&summary.key);
                let read =
                    full.is_some_and(|full| full.complete && full.revision == summary.revision);
                let preview = summary
                    .preview
                    .as_deref()
                    .filter(|preview| !read && snapshot.expanded.contains(&preview.key));
                let shown = preview.unwrap_or(summary);
                let cached = cached(&snapshot.details, &shown.key);
                let ends = |row: &Message| {
                    let id = shown.last_message_id.as_str();
                    row.id() == id || id.strip_prefix(LIVE) == Some(row.client_id())
                };
                let sliding = preview.is_some() && !cached.iter().any(ends);
                let body = match sliding {
                    true => cached,
                    false => range(cached, &shown.first_message_id, &shown.last_message_id),
                };
                let body = body.iter().filter(|row| !page.contains(row.client_id()));
                let stand_in = placeholder(summary);
                let id = stand_in.client_id().to_owned();
                if !body.clone().any(|row| row.client_id() == id) {
                    out.rows.push(stand_in);
                    out.placeholders.insert(id.clone());
                }
                out.references.insert(id, summary);
                for row in body {
                    out.rows.push(row.clone());
                    out.references.insert(row.client_id().to_owned(), summary);
                    seen.insert(row.client_id().to_owned());
                }
                end_ms = max(end_ms, summary.ended_at_ms);
            }
        }
    }
    let waits = |row: &Message| is.pending(row) && !seen.contains(row.client_id());
    let mut tail = input.pending.iter().filter_map(|id| {
        let row = live.get(id.as_str())?;
        (role(row) == "assistant" && !seen.contains(id)).then_some(*row)
    });
    let mut next = |source: &'a Message| match waits(source) {
        true => tail.next().expect("one live row per pending id"),
        false => source,
    };
    let ordered: Vec<&Message> = input.live.iter().map(&mut next).collect();
    for row in &ordered {
        if !is.is(row) || seen.contains(row.client_id()) {
            continue;
        }
        let at = created(row);
        if is.stale(row) && !is.pending(row) && at < end_ms {
            let before = out.rows.iter().position(|known| created(known) > at);
            out.rows
                .insert(before.unwrap_or(out.rows.len()), (*row).clone());
        } else if is.stale(row) || at >= end_ms || is.pending(row) {
            out.rows.push((*row).clone());
        }
    }
    let mut rendered: HashSet<String> = out.rows.iter().map(|r| r.client_id().to_owned()).collect();
    let mut before: Option<&str> = None;
    for row in ordered.iter().rev() {
        let id = row.client_id();
        if rendered.contains(id) {
            before = Some(id);
        } else if local(row) {
            let at =
                before.and_then(|before| out.rows.iter().position(|r| r.client_id() == before));
            out.rows
                .insert(at.unwrap_or(out.rows.len()), (*row).clone());
            rendered.insert(id.to_owned());
            before = Some(id);
        }
    }
    out
}

/// The view's items: its rows built, grouped, tied to their ranges, one per key.
pub fn render(input: &Input) -> Result<Projection, String> {
    let Rows {
        rows,
        placeholders,
        references,
    } = rows(input);
    let artifacts: Vec<Js> = summaries(&input.snapshot.items)
        .into_iter()
        .flat_map(|summary| summary.artifacts.iter().flatten().cloned())
        .collect();
    let options = Options {
        history_window_incomplete: true,
        working_dir: input.working_dir,
        history_artifacts: &artifacts,
        image_targets: input.image_targets,
    };
    let built = render_items::build(&rows, options)?;
    let binder = bind::Binder {
        rows: &rows,
        placeholders: &placeholders,
        references: &references,
        snapshot: input.snapshot,
    };
    let mut keys = HashSet::new();
    let items = work_groups::group(built, &rows, input.streaming)
        .iter()
        .map(|item| binder.bind(item))
        .filter(|item| keys.insert(item.key().to_owned()))
        .collect();
    Ok(Projection { rows, items })
}

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "the work group block of 9.11 calls them")
)]
impl Deferred {
    /// The keys `setVisible` holds open.
    fn visible(&self, full: bool, preview: bool) -> Vec<String> {
        let tail = |summary: &Summary| {
            let preview = summary.preview.as_deref()?;
            (self.live_preview && summary.is_streaming).then(|| preview.key.clone())
        };
        let keys = self.summaries.iter().filter_map(|summary| match full {
            true => Some(summary.key.clone()),
            false => tail(summary).filter(|_| preview),
        });
        keys.collect()
    }

    /// The ranges `retry` reads: a preview while it is open.
    fn retried(&self, expanded: &IndexSet<String>) -> Vec<&Summary> {
        let open = |preview: &&Summary| expanded.contains(&preview.key);
        let shown = self
            .summaries
            .iter()
            .map(|summary| summary.preview.as_deref().filter(open).unwrap_or(summary));
        shown.collect()
    }

    /// `setVisible`: holds the ranges open; the result lets go.
    pub fn set_visible(
        &self,
        view: &View<Message>,
        full: bool,
        preview: bool,
    ) -> impl FnOnce() + use<> {
        view.retain_expanded(self.visible(full, preview))
    }

    pub fn toggle(&self, view: &View<Message>) {
        for summary in &self.summaries {
            view.set_expanded(&summary.key, !self.expanded);
        }
    }

    pub fn retry(&self, view: &View<Message>) {
        for summary in self.retried(&view.snapshot().expanded) {
            drop(view.load_details(summary.clone(), false));
        }
    }
}

#[cfg(test)]
mod tests;
