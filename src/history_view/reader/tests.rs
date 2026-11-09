use super::{
    db::Db,
    fake::{Fake, ids, js, row, with},
    *,
};
use crate::history_view::artifacts::with_artifacts;

mod budget;
mod db;
mod edges;
mod live;
mod outline;

pub fn kinds(items: &[Item]) -> Vec<&'static str> {
    items
        .iter()
        .map(|i| match i {
            Item::Messages { .. } => "messages",
            Item::Work { .. } => "work",
        })
        .collect()
}

pub fn code<T: std::fmt::Debug>(result: Result<T>) -> (&'static str, String) {
    match result.expect_err("a refused read") {
        error @ Error::Ipc(code, _) => (code, error.to_string()),
        Error::Db(error) => panic!("database error {error}"),
    }
}

pub fn summary(item: &Item) -> &Summary {
    match item {
        Item::Work { summary, .. } => summary,
        Item::Messages { deferred, .. } => deferred.as_ref().expect("a deferred card"),
    }
}

/// Every detail page of a range.
pub fn drain(reader: &mut Reader, deps: &mut dyn Deps, r: &Ref) -> Vec<String> {
    let mut received = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let page = reader.details(deps, "s", r, after.as_deref()).unwrap();
        received.extend(ids(&page.messages));
        after = page.next_cursor;
        if after.is_none() {
            return received;
        }
    }
}

pub fn messages(items: &[Item]) -> Vec<&Row> {
    items
        .iter()
        .flat_map(|i| match i {
            Item::Messages { messages, .. } => messages.iter().collect(),
            Item::Work { .. } => Vec::new(),
        })
        .collect()
}

/// Items' JSON with every `revision` key dropped.
pub fn without_revision(items: &[Item]) -> String {
    fn strip(value: &mut Js) {
        match value {
            Js::Obj(fields) => {
                fields.shift_remove("revision");
                fields.values_mut().for_each(strip);
            }
            Js::Arr(values) => values.iter_mut().for_each(strip),
            _ => {}
        }
    }
    let mut all = Js::Arr(items.iter().map(json::item).collect());
    strip(&mut all);
    js::stringify(&all, "")
}

fn turn_meta() -> Js {
    js(r#"{"turnCompleted":true}"#)
}

#[test]
fn a_long_run_reads_through_outlines_and_keeps_sdk_metadata_in_details() {
    let mut source = vec![row(0, "user")];
    for i in 0..1100 {
        let meta = format!(
            r#"{{"uuid":"sdk-{i}","usage":{{"details":"{}"}}}}"#,
            "x".repeat(9000)
        );
        let r = with(
            row(i + 1, "thinking"),
            "content",
            Js::Str("thinking".into()),
        );
        source.push(with(r, "agentMeta", js(&meta)));
    }
    source.push(with(row(1101, "assistant"), "agentMeta", turn_meta()));
    let meta = crate::history_view::field(&source[1], "agentMeta").cloned();
    let mut fake = Fake::outlined(source);
    fake.outline_map = with_artifacts;
    fake.anchor_map = |r, outline| if outline { with_artifacts(r) } else { r };
    let mut reader = Reader::default();
    let page = reader.page(&mut fake, "s", None, true).unwrap();
    assert!(!page.has_more);
    assert_eq!(kinds(&page.items), ["messages", "work", "messages"]);
    let details = reader
        .details(&mut fake, "s", &summary(&page.items[1]).into(), None)
        .unwrap();
    assert_eq!(
        crate::history_view::field(&details.messages[0], "agentMeta").cloned(),
        meta
    );
    assert!(details.has_more);
}

#[test]
fn settled_plugin_work_pages_by_visible_groups_and_keeps_every_call() {
    let mut source = vec![row(0, "user")];
    for n in 1..=80 {
        let call = format!(
            r#"{{"toolName":"mcp:cindy:ghost_call","toolUseId":"t{n}","input":{{"ghost_id":"simulator","tool":"tap"}}}}"#
        );
        source.push(with(row(n * 2 - 1, "tool_use"), "content", js(&call)));
        let result = with(
            row(n * 2, "tool_result"),
            "toolUseId",
            Js::Str(format!("t{n}")),
        );
        source.push(with(result, "content", Js::Str("ok".into())));
    }
    let done = with(row(161, "assistant"), "content", Js::Str("Done".into()));
    source.push(with(done, "agentMeta", turn_meta()));
    let middle = ids(&source[1..source.len() - 1]);
    let mut fake = Fake::outlined(source);
    let mut reader = Reader::default();
    let page = reader.page(&mut fake, "s", None, true).unwrap();
    assert_eq!(kinds(&page.items), ["messages", "work", "messages"]);
    assert!(!page.has_more);
    assert_eq!(
        drain(&mut reader, &mut fake, &summary(&page.items[1]).into()),
        middle
    );
}

#[test]
fn a_long_hidden_run_pages_without_splitting_or_sending_bodies() {
    let mut source = vec![row(0, "user")];
    source.extend((1..=600).map(|i| row(i, "thinking")));
    source.push(row(601, "assistant"));
    let total = js::stringify(&Js::Arr(source.clone()), "").len();
    let middle = ids(&source[1..source.len() - 1]);
    let mut fake = Fake::new(source);
    let mut reader = Reader::default();
    let page = reader.page(&mut fake, "s", None, false).unwrap();
    assert_eq!(kinds(&page.items), ["messages", "work", "messages"]);
    assert!(!page.has_more);
    assert_eq!(fake.lists, 7);
    let shown = Js::Arr(page.items.iter().map(json::item).collect());
    assert!(js::stringify(&shown, "").len() < total / 100);
    assert_eq!(
        drain(&mut reader, &mut fake, &summary(&page.items[1]).into()),
        middle
    );
}

#[test]
fn visible_pages_walk_every_source_range_once() {
    let source: Vec<Row> = (0..45)
        .flat_map(|t| {
            [
                row(t * 3, "user"),
                row(t * 3 + 1, "thinking"),
                row(t * 3 + 2, "assistant"),
            ]
        })
        .collect();
    let all = ids(&source);
    let mut fake = Fake::new(source);
    let mut reader = Reader::default();
    let mut seen: Vec<String> = Vec::new();
    let mut before: Option<String> = None;
    loop {
        let page = reader
            .page(&mut fake, "s", before.as_deref(), false)
            .unwrap();
        assert!(page.items.len() <= 20);
        for item in &page.items {
            match item {
                Item::Messages { messages, .. } => seen.extend(ids(messages)),
                Item::Work { summary, .. } => {
                    seen.extend(drain(&mut reader, &mut fake, &summary.into()))
                }
            }
        }
        before = page.next_cursor;
        if before.is_none() {
            break;
        }
    }
    seen.sort_by_key(|i| i.parse::<usize>().unwrap());
    assert_eq!(seen, all);
}

#[test]
fn an_interrupted_turn_stays_folded_when_its_stamped_answer_starts_the_newer_page() {
    let mut n = 0;
    let mut next = || {
        n += 1;
        n - 1
    };
    let mut source = vec![row(next(), "user")];
    let say = |at: usize, text: &str| with(row(at, "assistant"), "content", Js::Str(text.into()));
    for i in 0..30 {
        source.push(say(next(), &format!("Still building {i}")));
        source.push(row(next(), "thinking"));
    }
    source.push(with(
        row(next(), "user"),
        "agentMeta",
        js(r#"{"delivery":"steer"}"#),
    ));
    source.push(row(next(), "thinking"));
    let cost = r#"{"turnCompleted":false,"turnCost":{"amount":5,"currency":"USD"}}"#;
    let answer = with(say(next(), "Checking CI"), "agentMeta", js(cost));
    let answer_id = text(&answer, "id").to_owned();
    source.push(answer);
    source.push(row(next(), "thinking"));
    for _ in 0..9 {
        source.push(row(next(), "user"));
        source.push(with(say(next(), "Done"), "agentMeta", turn_meta()));
    }
    let mut fake = Fake::new(source);
    let mut reader = Reader::default();
    let latest = reader.page(&mut fake, "s", None, false).unwrap();
    assert_eq!(latest.items.len(), 20);
    assert_eq!(latest.next_cursor.as_deref(), Some(answer_id.as_str()));
    let older = reader
        .page(&mut fake, "s", Some(&answer_id), false)
        .unwrap();
    let shape: Vec<&str> = older
        .items
        .iter()
        .map(|i| match i {
            Item::Work { .. } => "work",
            Item::Messages { messages, .. } => text(&messages[0], "role"),
        })
        .collect();
    assert_eq!(shape, ["user", "work", "user", "work"]);
    assert!(!older.has_more);
}

fn range(first: &str, last: &str) -> Ref {
    Ref {
        key: "w".into(),
        first: first.into(),
        last: last.into(),
        ..Ref::default()
    }
}

#[test]
fn same_millisecond_backward_ranges_and_cursors_past_the_rowid_are_refused() {
    let at = crate::history_view::field(&row(0, "user"), "createdAt")
        .cloned()
        .unwrap();
    let source = (1..=3)
        .map(|n| with(row(n, "thinking"), "createdAt", at.clone()))
        .collect();
    let mut fake = Fake::new(source);
    let mut reader = Reader::default();
    let refused = code(reader.details(&mut fake, "s", &range("2", "1"), None));
    assert_eq!(
        refused,
        (
            "INVALID_PARAMS",
            "[INVALID_PARAMS] Invalid history range".into()
        )
    );
    let refused = code(reader.details(&mut fake, "s", &range("1", "2"), Some("3")));
    assert_eq!(
        refused,
        (
            "INVALID_PARAMS",
            "[INVALID_PARAMS] Invalid detail cursor".into()
        )
    );
    let page = reader
        .details(&mut fake, "s", &range("1", "2"), None)
        .unwrap();
    assert_eq!(ids(&page.messages), ["1", "2"]);
}

#[test]
fn a_clear_during_the_scan_publishes_nothing() {
    let mut fake = Fake::new(vec![
        row(0, "user"),
        row(1, "thinking"),
        row(2, "assistant"),
    ]);
    fake.on_read = Some(Box::new(|source| source.clear()));
    fake.once = true;
    let refused = code(Reader::default().page(&mut fake, "s", None, false));
    assert_eq!(
        refused,
        ("NOT_FOUND", "[NOT_FOUND] History range changed".into())
    );
}

#[test]
fn a_cursor_answer_rewound_mid_read_is_refused() {
    for phase in ["raw scan", "outline scan", "hydrate"] {
        let continuing = with(row(2, "assistant"), "content", Js::Str("Continuing".into()));
        let result = with(
            row(4, "assistant"),
            "content",
            Js::Str("# Result\nDelivered".into()),
        );
        let source = vec![
            row(0, "user"),
            row(1, "thinking"),
            with(continuing, "agentMeta", turn_meta()),
            row(3, "thinking"),
            with(result, "agentMeta", turn_meta()),
        ];
        let mut fake = Fake::outlined(source);
        let pop: fn(&mut Vec<Row>) = |source| {
            source.pop();
        };
        match phase {
            "hydrate" => fake.on_hydrate = Some(Box::new(pop)),
            _ => fake.on_read = Some(Box::new(pop)),
        }
        let refused = code(Reader::default().page(&mut fake, "s", Some("4"), phase != "raw scan"));
        assert_eq!(
            refused,
            ("NOT_FOUND", "[NOT_FOUND] History range changed".into()),
            "{phase}"
        );
        assert_eq!(ids(&fake.source), ["0", "1", "2", "3"], "{phase}");
    }
}

#[test]
fn details_cleared_or_rewound_mid_batch_are_refused() {
    for size in [1000, 200_000] {
        for keep in [0, 1] {
            let source = (1..=3)
                .map(|n| with(row(n, "thinking"), "content", Js::Str("x".repeat(size))))
                .collect();
            let mut fake = Fake::new(source);
            fake.on_read = Some(Box::new(move |source| source.truncate(keep)));
            fake.once = true;
            let refused = code(Reader::default().details(&mut fake, "s", &range("1", "3"), None));
            assert_eq!(
                refused,
                ("NOT_FOUND", "[NOT_FOUND] History range changed".into())
            );
        }
    }
}

#[test]
fn a_scan_that_cannot_finish_keeps_its_ipc_code() {
    for kind in ["page", "detail", "empty", "overshoot"] {
        let mut fake = Fake::new(vec![row(1, "thinking"), row(2, "thinking")]);
        fake.list_value = Some(match kind {
            "empty" => Vec::new(),
            "overshoot" => vec![row(3, "thinking")],
            _ => vec![row(1, "thinking")],
        });
        let mut reader = Reader::default();
        let (got, message) = match kind {
            "page" => code(reader.page(&mut fake, "s", Some("1"), false)),
            _ => code(reader.details(&mut fake, "s", &range("1", "2"), None)),
        };
        let want = if matches!(kind, "page" | "detail") {
            "INTERNAL"
        } else {
            "NOT_FOUND"
        };
        assert_eq!(got, want, "{kind}");
        assert!(message.contains(&format!("[{want}]")));
    }
}
