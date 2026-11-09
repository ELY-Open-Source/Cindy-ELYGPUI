use super::*;

fn live_row(n: usize, content: String) -> Row {
    let r = with(row(n, "thinking"), "id", Js::Str(format!("{LIVE}c{n}")));
    with(r, "content", Js::Str(content))
}

#[test]
fn in_flight_blocks_continue_across_their_byte_boundary() {
    let mut fake = Fake::new(vec![row(0, "user")]);
    fake.live = (1..=2).map(|n| live_row(n, "x".repeat(200_000))).collect();
    fake.running = true;
    let mut reader = Reader::default();
    let page = reader.page(&mut fake, "s", None, false).unwrap();
    let work = page
        .items
        .iter()
        .find(|i| matches!(i, Item::Work { .. }))
        .unwrap();
    let r: Ref = summary(work).into();
    let first = reader.details(&mut fake, "s", &r, None).unwrap();
    let clients = |p: &DetailPage| {
        p.messages
            .iter()
            .map(|m| text(m, "clientId").to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(clients(&first), ["c1"]);
    assert!(first.has_more);
    let second = reader
        .details(&mut fake, "s", &r, first.next_cursor.as_deref())
        .unwrap();
    assert_eq!(clients(&second), ["c2"]);
    assert!(!second.has_more);
}

fn live_ref(ids: &[String]) -> Ref {
    Ref {
        key: "w".into(),
        first: ids[0].clone(),
        last: ids[ids.len() - 1].clone(),
        live: ids.to_vec(),
        ..Ref::default()
    }
}

#[test]
fn live_anchor_reads_stay_bounded_to_each_page() {
    let source: Vec<Row> = (0..2000)
        .map(|n| with(row(n, "thinking"), "content", Js::Str("x".repeat(4000))))
        .collect();
    let all = ids(&source);
    let live: Vec<String> = (0..2000).map(|n| format!("{LIVE}c{n}")).collect();
    let mut fake = Fake::new(source);
    fake.running = true;
    fake.anchor_fallback = Some(|source, id| {
        let n: usize = id.trim_start_matches("history-live:c").parse().unwrap();
        source[n].clone()
    });
    let mut reader = Reader::default();
    let r = live_ref(&live);
    let (mut received, mut after) = (Vec::new(), None::<String>);
    loop {
        fake.anchors.clear();
        let page = reader
            .details(&mut fake, "s", &r, after.as_deref())
            .unwrap();
        received.extend(ids(&page.messages));
        assert!(fake.anchors.len() <= page.messages.len() * 2 + 3);
        if page.has_more {
            let last = text(page.messages.last().unwrap(), "clientId");
            assert_eq!(page.next_cursor, Some(format!("{LIVE}{last}")));
        }
        after = page.next_cursor;
        if after.is_none() {
            break;
        }
    }
    assert_eq!(received, all);
}

#[test]
fn future_live_bodies_wait_while_the_stored_range_has_another_page() {
    let source: Vec<Row> = (1..=3)
        .map(|n| with(row(n, "thinking"), "content", Js::Str("x".repeat(200_000))))
        .collect();
    let live: Vec<String> = (4..=6).map(|n| format!("{LIVE}c{n}")).collect();
    let mut fake = Fake::new(source);
    fake.running = true;
    fake.anchor_fallback = Some(|_, id| with(row(6, "thinking"), "id", Js::Str(id.into())));
    let r = Ref {
        last: live[2].clone(),
        first_stored: Some("1".into()),
        last_stored: Some("3".into()),
        first: "1".into(),
        ..live_ref(&live)
    };
    let page = Reader::default().details(&mut fake, "s", &r, None).unwrap();
    assert_eq!(ids(&page.messages), ["1"]);
    assert_eq!(page.next_cursor.as_deref(), Some("1"));
    let read: Vec<&String> = fake.anchors.iter().filter(|a| live.contains(a)).collect();
    assert_eq!(read, [&live[2]]);
}

#[test]
fn a_live_page_invalidated_after_collection_is_refused() {
    for change in ["clear", "rewind", "middle"] {
        let live: Vec<String> = (1..=4).map(|n| format!("{LIVE}c{n}")).collect();
        let mut fake = Fake::new((1..=4).map(|n| live_row(n, "x".repeat(80_000))).collect());
        fake.running = true;
        let (last, middle) = (live[3].clone(), live[1].clone());
        fake.on_anchor = Some(Box::new(move |source, id| {
            if id == last {
                let gone = if change == "middle" { &middle } else { &last };
                match change {
                    "clear" => source.clear(),
                    _ => source.retain(|r| text(r, "id") != gone),
                }
            }
        }));
        let result = Reader::default().details(&mut fake, "s", &live_ref(&live), None);
        assert_eq!(code(result).0, "NOT_FOUND", "{change}");
    }
}
