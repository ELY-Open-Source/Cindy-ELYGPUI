//! In-memory dependencies, as `historyViewReader.test.ts` builds them.

use super::{Deps, Error, Opts, Result, Row, id, text};
use crate::rich_answer::js::{Js, Obj};

type Hook = Box<dyn FnMut(&mut Vec<Row>)>;
type AnchorHook = Box<dyn FnMut(&mut Vec<Row>, &str)>;

/// `row(n, role)`: a 1000-byte body at `1700000000000 + n` ms.
pub fn row(n: usize, role: &str) -> Row {
    let mut out = Obj::new();
    out.insert("id".into(), Js::Str(n.to_string()));
    out.insert("clientId".into(), Js::Str(format!("c{n}")));
    out.insert("rowid".into(), Js::Num(n as f64));
    out.insert("role".into(), Js::Str(role.into()));
    out.insert("content".into(), Js::Str("body ".repeat(200)));
    let at = crate::js_date::iso(1_700_000_000_000 + n as i64).expect("a time");
    out.insert("createdAt".into(), Js::Str(at));
    Js::Obj(out)
}

/// `{ ...row, key: value }`.
pub fn with(mut row: Row, key: &str, value: Js) -> Row {
    if let Js::Obj(fields) = &mut row {
        fields.insert(key.into(), value);
    }
    row
}

pub fn js(text: &str) -> Js {
    crate::rich_answer::js::parse(text).expect("test JSON")
}

pub struct Fake {
    pub source: Vec<Row>,
    pub live: Vec<Row>,
    /// `outline` and `hydrate` given.
    pub outlined: bool,
    pub outline_map: fn(Row) -> Row,
    pub anchor_map: fn(Row, bool) -> Row,
    /// The anchor of an id not in the source; else `NOT_FOUND`.
    pub anchor_fallback: Option<fn(&[Row], &str) -> Row>,
    pub revision: Option<String>,
    /// The revision a write during the next read leaves.
    pub revision_after_read: Option<String>,
    /// `validate` given: every id still in the source.
    pub validates: bool,
    pub running: bool,
    /// `readMessagesList`'s clamp on raw reads.
    pub limit_cap: Option<usize>,
    /// `mockResolvedValue`: every list returns these.
    pub list_value: Option<Vec<Row>>,
    pub on_read: Option<Hook>,
    pub once: bool,
    pub on_hydrate: Option<Hook>,
    pub on_anchor: Option<AnchorHook>,
    pub lists: usize,
    pub outlines: usize,
    pub hydrated: Vec<String>,
    pub anchors: Vec<String>,
}

impl Fake {
    pub fn new(source: Vec<Row>) -> Self {
        Fake {
            source,
            live: Vec::new(),
            outlined: false,
            outline_map: |r| r,
            anchor_map: |r, _| r,
            anchor_fallback: None,
            revision: None,
            revision_after_read: None,
            validates: false,
            running: false,
            limit_cap: None,
            list_value: None,
            on_read: None,
            once: false,
            on_hydrate: None,
            on_anchor: None,
            lists: 0,
            outlines: 0,
            hydrated: Vec::new(),
            anchors: Vec::new(),
        }
    }

    pub fn outlined(source: Vec<Row>) -> Self {
        Fake {
            outlined: true,
            ..Fake::new(source)
        }
    }

    /// The test's `list`: newest first, `before` exclusive, `after` oldest first then reversed.
    fn read(&mut self, opts: &Opts, cap: Option<usize>) -> Vec<Row> {
        if let Some(value) = &self.list_value {
            return value.clone();
        }
        let limit = opts.limit.map_or(50, |l| l as usize);
        let limit = cap.map_or(limit, |c| limit.min(c));
        let find = |c: &str| self.source.iter().position(|r| id(r) == c);
        let start = opts
            .after
            .as_deref()
            .map_or(0, |a| find(a).map_or(0, |i| i + 1));
        // `findIndex` of -1 slices to the last row, as `slice(start, -1)` does.
        let end = opts.before.as_deref().map_or(self.source.len(), |b| {
            find(b).unwrap_or(self.source.len().saturating_sub(1))
        });
        let selected = &self.source[start.min(end)..end];
        let mut out = match opts.after {
            Some(_) => selected[..limit.min(selected.len())].to_vec(),
            None => selected[selected.len().saturating_sub(limit)..].to_vec(),
        };
        out.reverse();
        if let Some(next) = self.revision_after_read.take() {
            self.revision = Some(next);
        }
        if let Some(hook) = self.on_read.as_mut() {
            hook(&mut self.source);
            if self.once {
                self.on_read = None;
            }
        }
        out
    }
}

impl Deps for Fake {
    fn list(&mut self, _session: &str, opts: &Opts) -> Result<Vec<Row>> {
        self.lists += 1;
        Ok(self.read(opts, self.limit_cap))
    }

    fn outlines(&self) -> bool {
        self.outlined
    }

    fn outline(&mut self, _session: &str, opts: &Opts) -> Result<Vec<Row>> {
        self.outlines += 1;
        let map = self.outline_map;
        Ok(self.read(opts, None).into_iter().map(map).collect())
    }

    fn hydrate(&mut self, _session: &str, ids: &[String]) -> Result<Vec<Row>> {
        self.hydrated.extend(ids.iter().cloned());
        let rows = self
            .source
            .iter()
            .filter(|r| ids.iter().any(|i| i == id(r)))
            .cloned()
            .collect();
        if let Some(hook) = self.on_hydrate.as_mut() {
            hook(&mut self.source);
        }
        Ok(rows)
    }

    fn revision(&mut self) -> Option<Result<String>> {
        self.revision.clone().map(Ok)
    }

    fn validate(&mut self, _session: &str, ids: &[String]) -> Option<Result<()>> {
        let known = |i: &String| self.source.iter().chain(&self.live).any(|r| id(r) == i);
        self.validates.then(|| match ids.iter().all(known) {
            true => Ok(()),
            false => Err(Error::Ipc("NOT_FOUND", "History range changed")),
        })
    }

    fn anchor(&mut self, _session: &str, wanted: &str, outline: bool) -> Result<Row> {
        self.anchors.push(wanted.to_owned());
        let found = self
            .source
            .iter()
            .chain(&self.live)
            .find(|r| id(r) == wanted)
            .cloned()
            .or_else(|| self.anchor_fallback.map(|f| f(&self.source, wanted)));
        if let Some(hook) = self.on_anchor.as_mut() {
            hook(&mut self.source, wanted);
        }
        let row = found.ok_or(Error::Ipc("NOT_FOUND", "History range changed"))?;
        Ok((self.anchor_map)(row, outline))
    }

    fn running(&self, _session: &str) -> bool {
        self.running
    }

    fn live(&self, _session: &str) -> Vec<Row> {
        self.live.clone()
    }
}

pub fn ids(rows: &[Row]) -> Vec<String> {
    rows.iter().map(|r| text(r, "id").to_owned()).collect()
}
