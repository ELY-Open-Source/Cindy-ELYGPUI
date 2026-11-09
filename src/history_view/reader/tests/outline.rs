use super::*;
use crate::rich_answer::js::Obj;

fn tool(n: usize, id: &str, name: &str, input: &str) -> Row {
    let call = format!(r#"{{"toolName":"{name}","input":{input}}}"#);
    with(
        with(row(n, "tool_use"), "toolUseId", Js::Str(id.into())),
        "content",
        js(&call),
    )
}

fn child(r: Row, parent: &str) -> Row {
    with(
        r,
        "agentMeta",
        js(&format!(r#"{{"parentUuid":"{parent}"}}"#)),
    )
}

fn delivered(n: usize, body: &str) -> Row {
    let r = with(
        row(n, "assistant"),
        "content",
        Js::Str(format!("# Result\n{body}")),
    );
    with(r, "agentMeta", turn_meta())
}

fn card<'a>(items: &'a [Item], parent: &str) -> &'a Summary {
    items
        .iter()
        .find_map(|i| match i {
            Item::Messages {
                deferred: Some(d), ..
            } if d.parent_tool_use_id.as_deref() == Some(parent) => Some(d),
            _ => None,
        })
        .expect("a lazy subagent card")
}

#[test]
fn a_main_task_result_pages_apart_from_its_call() {
    let mut source = vec![row(0, "user"), tool(1, "main-call", "Bash", "{}")];
    source.extend((0..30).map(|i| row(i + 2, "user")));
    source.push(with(
        row(32, "tool_result"),
        "toolUseId",
        Js::Str("main-call".into()),
    ));
    let page = Reader::default()
        .page(&mut Fake::new(source), "s", None, true)
        .unwrap();
    assert!(page.has_more);
    assert!(
        matches!(page.items.last(), Some(Item::Work { summary, .. }) if summary.last_message_id == "32")
    );
}

#[test]
fn an_agent_spanning_deliveries_stays_whole() {
    for outlined in [false, true] {
        let mut n = 3;
        let mut source = vec![
            row(0, "user"),
            tool(1, "a", "Agent", "{}"),
            child(tool(2, "b", "Agent", "{}"), "a"),
        ];
        for i in 0..1200 {
            if i % 300 == 0 {
                source.push(delivered(n, "Delivered"));
                n += 1;
            }
            source.push(child(
                row(n, "thinking"),
                if i % 2 == 1 { "a" } else { "b" },
            ));
            n += 1;
        }
        let want = without_revision(&crate::history_view::project(&source, false, true));
        let mut fake = if outlined {
            Fake::outlined(source)
        } else {
            Fake::new(source)
        };
        let page = Reader::default().page(&mut fake, "s", None, true).unwrap();
        assert_eq!(without_revision(&page.items), want, "{outlined}");
        assert!(!page.has_more);
        assert_eq!(card(&page.items, "a").message_count, 1201);
    }
}

/// The outline as the test gives it: tool inputs and answers left out.
fn compact(r: Row) -> Row {
    match text(&r, "role") {
        "tool_use" => {
            let mut call = crate::history_view::content(&r).clone();
            if let Js::Obj(fields) = &mut call {
                fields.insert("input".into(), Js::Obj(Obj::new()));
            }
            with(r, "content", call)
        }
        "assistant" => with(r, "content", Js::Str("# Result\nOutline".into())),
        _ => r,
    }
}

#[derive(Default)]
struct Case {
    lazy: bool,
    outlined: bool,
    large: bool,
    result_only: bool,
    live_tail: bool,
    newer_tail: bool,
}

fn leaves(items: &[Item]) -> Vec<&Item> {
    items
        .iter()
        .flat_map(|i| match i {
            Item::Work {
                children: Some(c), ..
            } => leaves(c),
            _ => vec![i],
        })
        .collect()
}

fn intact_agent(case: &Case) {
    let mut n = 2;
    let mut next = || {
        n += 1;
        n - 1
    };
    let description = r#"{"description":"Full agent description"}"#;
    let mut source = vec![row(0, "user"), tool(1, "a", "Agent", description)];
    if case.result_only {
        source.push(child(tool(next(), "child-call", "Bash", "{}"), "a"));
    }
    for _ in 0..(if case.large { 6 } else { 75 }) {
        let step = row(next(), "thinking");
        source.push(if case.result_only || case.live_tail {
            step
        } else {
            child(step, "a")
        });
        let body = "x".repeat(if case.large { 200_000 } else { 10 });
        source.push(delivered(next(), &body));
    }
    let tail = match case.result_only {
        true => with(
            row(next(), "tool_result"),
            "toolUseId",
            Js::Str("child-call".into()),
        ),
        false => child(row(next(), "thinking"), "a"),
    };
    let mut live = Vec::new();
    match case.live_tail {
        true => live.push(with(
            tail.clone(),
            "id",
            Js::Str(format!("{LIVE}{}", text(&tail, "clientId"))),
        )),
        false => source.push(tail),
    }
    if case.newer_tail {
        source.push(row(next(), "user"));
    }
    let mut fake = if case.outlined {
        Fake::outlined(source.clone())
    } else {
        Fake::new(source.clone())
    };
    fake.limit_cap = Some(100);
    fake.outline_map = compact;
    fake.running = case.live_tail;
    fake.live = live.clone();
    let mut reader = Reader::default();
    if case.large {
        let mut before = None;
        if case.newer_tail {
            let page = reader.page(&mut fake, "s", None, case.lazy).unwrap();
            assert_eq!(page.items.len(), 1);
            assert!(
                matches!(&page.items[0], Item::Messages { messages, .. } if messages[..] == source[source.len() - 1..])
            );
            assert!(page.has_more);
            before = page.next_cursor;
        }
        let refused = reader.page(&mut fake, "s", before.as_deref(), case.lazy);
        assert_eq!(code(refused).0, "UNSUPPORTED_CAPABILITY");
        return;
    }
    let mut seen: Vec<String> = Vec::new();
    let mut before: Option<String> = None;
    loop {
        let page = reader
            .page(&mut fake, "s", before.as_deref(), case.lazy)
            .unwrap();
        for item in leaves(&page.items) {
            if let Item::Messages { messages, .. } = item {
                seen.extend(ids(messages));
                for m in messages {
                    let stored = source.iter().find(|r| text(r, "id") == text(m, "id"));
                    assert_eq!(
                        stored.map(crate::history_view::content),
                        Some(crate::history_view::content(m))
                    );
                }
            }
            let reference = match item {
                Item::Work { summary, .. } => Some(summary),
                Item::Messages { deferred, .. } => deferred.as_ref(),
            };
            if let Some(summary) = reference {
                seen.extend(drain(&mut reader, &mut fake, &summary.into()));
            }
        }
        assert_ne!(page.next_cursor, before);
        before = page.next_cursor;
        if before.is_none() {
            break;
        }
    }
    seen.sort();
    let mut all = ids(&source);
    all.extend(ids(&live));
    all.sort();
    assert_eq!(seen, all);
}

#[test]
fn an_intact_agent_pages_across_the_item_target() {
    let cases = [
        Case::default(),
        Case {
            lazy: true,
            ..Case::default()
        },
        Case {
            lazy: true,
            outlined: true,
            ..Case::default()
        },
        Case {
            lazy: true,
            outlined: true,
            large: true,
            ..Case::default()
        },
        Case {
            lazy: true,
            outlined: true,
            large: true,
            newer_tail: true,
            ..Case::default()
        },
        Case {
            lazy: true,
            outlined: true,
            result_only: true,
            ..Case::default()
        },
        Case {
            lazy: true,
            outlined: true,
            live_tail: true,
            ..Case::default()
        },
    ];
    for case in &cases {
        intact_agent(case);
    }
}

#[test]
fn visible_bodies_come_first_and_only_the_expanded_agent_pages() {
    let mut source = vec![
        row(0, "user"),
        tool(1, "a", "Agent", r#"{"description":"A"}"#),
        tool(2, "b", "Agent", r#"{"description":"B"}"#),
    ];
    source.extend(
        (0..3000).map(|i| child(row(i + 3, "thinking"), if i % 2 == 1 { "b" } else { "a" })),
    );
    source.push(row(3003, "assistant"));
    let expected: Vec<String> = source
        .iter()
        .filter(|r| {
            crate::history_view::meta(r).and_then(|m| m.get("parentUuid"))
                == Some(&Js::Str("a".into()))
        })
        .map(|r| text(r, "id").to_owned())
        .collect();
    let mut fake = Fake::outlined(source);
    fake.outline_map = |r| match text(&r, "role") {
        "thinking" => with(r, "content", js(r#"{"isRedacted":true}"#)),
        _ => r,
    };
    fake.running = true;
    fake.revision = Some("stable".into());
    let mut reader = Reader::default();
    let page = reader.page(&mut fake, "s", None, true).unwrap();
    assert!(!page.has_more);
    let shown: Vec<&str> = messages(&page.items)
        .iter()
        .map(|m| text(m, "id"))
        .collect();
    assert_eq!(shown, ["0", "1", "2", "3003"]);
    assert_eq!(fake.hydrated, ["0", "1", "2", "3003"]);
    assert_eq!(fake.outlines, 4);
    let lazy: Ref = card(&page.items, "a").into();
    fake.hydrated.clear();
    fake.outlines = 0;
    assert_eq!(drain(&mut reader, &mut fake, &lazy), expected);
    assert_eq!(fake.outlines, 3);
    assert!(fake.hydrated.iter().all(|i| expected.contains(i)));
}

#[test]
fn database_revisions_keep_snapshots_and_catch_hidden_edits() {
    let mut fake = Fake::outlined(vec![
        row(0, "user"),
        row(1, "thinking"),
        row(2, "assistant"),
    ]);
    fake.revision = Some("first".into());
    let mut reader = Reader::default();
    let initial = reader.page(&mut fake, "s", None, true).unwrap();
    assert_eq!(reader.page(&mut fake, "s", None, true).unwrap(), initial);
    fake.revision = Some("edited".into());
    fake.source[1] = with(fake.source[1].clone(), "content", Js::Str("changed".into()));
    assert_ne!(reader.page(&mut fake, "s", None, true).unwrap(), initial);
}

#[test]
fn a_revision_drops_cached_membership_and_cleared_anchors_still_fail() {
    let source = (1..=4)
        .map(|n| {
            child(
                with(row(n, "thinking"), "content", Js::Str("x".repeat(200_000))),
                "a",
            )
        })
        .collect();
    let mut fake = Fake::outlined(source);
    let redact: fn(Row) -> Row = |r| with(r, "content", js(r#"{"isRedacted":true}"#));
    fake.outline_map = redact;
    fake.anchor_map = |r, _| with(r, "content", js(r#"{"isRedacted":true}"#));
    fake.revision = Some("one".into());
    let r = Ref {
        parent: Some("a".into()),
        ..range("1", "4")
    };
    let mut reader = Reader::default();
    assert_eq!(
        reader
            .details(&mut fake, "s", &r, None)
            .unwrap()
            .next_cursor
            .as_deref(),
        Some("1")
    );
    assert_eq!(
        ids(&reader
            .details(&mut fake, "s", &r, Some("1"))
            .unwrap()
            .messages),
        ["2"]
    );
    assert_eq!(fake.outlines, 1);
    fake.source[1] = child(fake.source[1].clone(), "b");
    fake.revision = Some("two".into());
    assert_eq!(
        ids(&reader
            .details(&mut fake, "s", &r, Some("1"))
            .unwrap()
            .messages),
        ["3"]
    );
    assert_eq!(fake.outlines, 2);
    fake.source.clear();
    assert_eq!(
        code(reader.details(&mut fake, "s", &r, Some("3"))).0,
        "NOT_FOUND"
    );
}
