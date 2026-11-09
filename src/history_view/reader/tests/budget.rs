use super::*;

fn calls(fake: &Fake) -> usize {
    fake.lists + fake.outlines
}

#[test]
fn completed_deliveries_page_without_rescanning_the_whole_run() {
    for (outlined, streaming) in [(false, false), (true, false), (true, true)] {
        let mut n = 0;
        let mut next = || {
            n += 1;
            n - 1
        };
        let mut source = vec![row(next(), "user")];
        let say = |at: usize, text: &str| {
            let r = with(row(at, "assistant"), "content", Js::Str(text.into()));
            with(r, "agentMeta", turn_meta())
        };
        for _ in 0..90 {
            for _ in 0..50 {
                source.push(with(
                    row(next(), "thinking"),
                    "content",
                    Js::Str("detail ".repeat(300)),
                ));
            }
            source.push(say(next(), "Continuing"));
            source.push(row(next(), "thinking"));
            source.push(say(next(), "# Result\nDelivered"));
        }
        source.push(row(next(), "user"));
        if streaming {
            source.push(row(next(), "thinking"));
        }
        assert!(js::stringify(&Js::Arr(source.clone()), "").len() > 8 * 1024 * 1024);
        let want = without_revision(&crate::history_view::project(&source, streaming, outlined));
        let mut fake = if outlined {
            Fake::outlined(source)
        } else {
            Fake::new(source)
        };
        fake.running = streaming;
        let mut reader = Reader::default();
        let mut received: Vec<Item> = Vec::new();
        let mut before: Option<String> = None;
        let mut pages = 0;
        loop {
            let read = calls(&fake);
            let page = reader
                .page(&mut fake, "s", before.as_deref(), outlined)
                .unwrap();
            assert!(!page.items.is_empty());
            assert!(calls(&fake) - read <= if outlined { 2 } else { 10 });
            assert_ne!(page.next_cursor, before);
            received.splice(0..0, page.items);
            before = page.next_cursor;
            pages += 1;
            assert!(pages < 20);
            if before.is_none() {
                break;
            }
        }
        assert_eq!(without_revision(&received), want, "{outlined}/{streaming}");
        let continuing = messages(&received).iter().any(|m| {
            crate::history_view::field(m, "content") == Some(&Js::Str("Continuing".into()))
        });
        assert!(!continuing);
    }
}

#[test]
fn delivered_bytes_fill_a_page_before_twenty_items() {
    let mut n = 0;
    let mut source = vec![row(n, "user")];
    for _ in 0..40 {
        for _ in 0..99 {
            n += 1;
            source.push(row(n, "thinking"));
        }
        n += 1;
        let report = with(
            row(n, "assistant"),
            "content",
            Js::Str(format!("# Report\n{}", "x".repeat(200_000))),
        );
        source.push(with(report, "agentMeta", turn_meta()));
    }
    let mut fake = Fake::outlined(source);
    let page = Reader::default().page(&mut fake, "s", None, true).unwrap();
    assert_eq!(calls(&fake), 1);
    assert!(page.items.len() > 1 && page.items.len() < 20);
    assert!(page.has_more);
    let shown = Js::Arr(page.items.iter().map(json::item).collect());
    assert!(js::stringify(&shown, "").len() < 1024 * 1024);
}

#[test]
fn work_never_splits_at_a_progress_or_subagent_seal() {
    for kind in ["progress", "subagent"] {
        let source = (0..2500)
            .map(|n| match n % 50 {
                0 => {
                    let text = if kind == "progress" {
                        "Continuing"
                    } else {
                        "# Child report"
                    };
                    let meta = match kind {
                        "progress" => turn_meta(),
                        _ => js(r#"{"turnCompleted":true,"parentUuid":"child"}"#),
                    };
                    with(
                        with(row(n, "assistant"), "content", Js::Str(text.into())),
                        "agentMeta",
                        meta,
                    )
                }
                _ => row(n, "thinking"),
            })
            .collect();
        let refused = code(Reader::default().page(&mut Fake::new(source), "s", None, false));
        assert_eq!(refused.0, "UNSUPPORTED_CAPABILITY", "{kind}");
    }
}

#[test]
fn a_full_visible_page_comes_before_an_older_oversized_group() {
    let mut source: Vec<Row> = (0..2500).map(|i| row(i, "thinking")).collect();
    let latest: Vec<Row> = (0..20).map(|i| row(2500 + i, "user")).collect();
    source.extend(latest.iter().cloned());
    let mut fake = Fake::new(source);
    let mut reader = Reader::default();
    let page = reader.page(&mut fake, "s", None, false).unwrap();
    let shown: Vec<String> = messages(&page.items)
        .iter()
        .map(|m| text(m, "id").to_owned())
        .collect();
    assert_eq!(shown, ids(&latest));
    assert!(page.has_more);
    assert_eq!(page.next_cursor.as_deref(), Some("2500"));
    assert_eq!(fake.lists, 1);
    let older = reader.page(&mut fake, "s", Some("2500"), false);
    assert_eq!(code(older).0, "UNSUPPORTED_CAPABILITY");
}

#[test]
fn a_boundary_free_history_stops_before_a_twenty_first_query() {
    let mut fake = Fake::new((0..2500).map(|i| row(i, "thinking")).collect());
    let refused = code(Reader::default().page(&mut fake, "s", None, false));
    assert_eq!(refused.0, "UNSUPPORTED_CAPABILITY");
    assert_eq!(fake.lists, 20);
}

#[test]
fn retained_bytes_stop_before_a_large_batch_projects() {
    let source = (0..150)
        .map(|i| with(row(i, "thinking"), "content", Js::Str("x".repeat(100_000))))
        .collect();
    let mut fake = Fake::new(source);
    let refused = code(Reader::default().page(&mut fake, "s", None, false));
    assert_eq!(refused.0, "UNSUPPORTED_CAPABILITY");
    assert_eq!(fake.lists, 1);
}

#[test]
fn in_flight_content_shares_the_budget() {
    let mut fake = Fake::new(vec![row(0, "user")]);
    fake.running = true;
    fake.live = vec![with(
        row(1, "thinking"),
        "content",
        Js::Str("x".repeat(8 * 1024 * 1024)),
    )];
    let refused = code(Reader::default().page(&mut fake, "s", None, false));
    assert_eq!(refused.0, "UNSUPPORTED_CAPABILITY");
    assert_eq!(fake.lists, 1);
}
