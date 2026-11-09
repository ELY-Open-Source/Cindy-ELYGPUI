//! Branches Cindy's tests leave open, each pinned to Cindy's source.

use super::*;

fn say(n: usize, text: &str, meta: &str) -> Row {
    let r = with(row(n, "assistant"), "content", Js::Str(text.into()));
    with(r, "agentMeta", js(meta))
}

fn agent(n: usize, id: &str) -> Row {
    let call = with(row(n, "tool_use"), "toolUseId", Js::Str(id.into()));
    with(call, "content", js(r#"{"toolName":"Agent","input":{}}"#))
}

fn child(r: Row, parent: &str) -> Row {
    with(
        r,
        "agentMeta",
        js(&format!(r#"{{"parentUuid":"{parent}"}}"#)),
    )
}

fn sized(n: usize, size: usize) -> Row {
    with(row(n, "thinking"), "content", Js::Str("x".repeat(size)))
}

fn messages_item(content: usize) -> Item {
    Item::Messages {
        key: "k".into(),
        messages: vec![sized(0, content)],
        deferred: None,
    }
}

#[test]
fn page_boundaries_are_users_systems_and_delivered_answers() {
    let done = r#"{"turnCompleted":true}"#;
    assert!(is_boundary(&row(0, "user")));
    assert!(is_boundary(&row(0, "system")));
    assert!(is_boundary(&say(0, "# Result\nDelivered", done)));
    assert!(!is_boundary(&say(
        0,
        "# Result\nDelivered",
        r#"{"turnCompleted":true,"parentUuid":"a"}"#
    )));
    assert!(!is_boundary(&say(0, "# Result\nDelivered", "{}")));
    assert!(!is_boundary(&say(0, "Continuing", done)));
    assert!(!is_boundary(&with(say(0, "", done), "content", js("{}"))));
    assert!(!is_boundary(&row(0, "thinking")));
}

#[test]
fn a_suffix_cuts_tool_context_without_its_header() {
    let result = with(row(1, "tool_result"), "toolUseId", Js::Str("x".into()));
    assert!(cuts_tool_context(std::slice::from_ref(&result), 0));
    let call = with(
        with(row(0, "tool_use"), "toolUseId", Js::Str("x".into())),
        "content",
        js(r#"{"toolName":"Bash"}"#),
    );
    assert!(!cuts_tool_context(&[call.clone(), result.clone()], 0));
    assert!(cuts_tool_context(&[call, result], 1));
    assert!(cuts_tool_context(&[child(row(0, "thinking"), "y")], 0));
    let stray = with(row(0, "thinking"), "toolUseId", Js::Str("z".into()));
    assert!(!cuts_tool_context(&[stray], 0));
}

#[test]
fn safe_starts_keep_every_child_after_its_earliest_header() {
    let rows = [
        row(0, "user"),
        agent(1, "A"),
        row(2, "user"),
        agent(3, "B"),
        child(row(4, "thinking"), "B"),
        child(row(5, "thinking"), "A"),
    ];
    let mut safe: Vec<String> = safe_starts(&rows).into_iter().collect();
    safe.sort();
    assert_eq!(safe, ["0", "1"]);
}

/// The JSON overhead of a one-row messages item.
fn overhead() -> usize {
    bytes(&json::item(&messages_item(0)))
}

#[test]
fn one_oversized_item_still_starts_a_page() {
    let items = [messages_item(2 * PAGE_BYTES)];
    let safe = HashSet::from(["0".to_owned()]);
    assert_eq!(page_start(&items, &safe, true).unwrap(), 0);
}

#[test]
fn the_page_cut_counts_a_kilobyte_of_framing() {
    let each = (PAGE_BYTES - 512) / 2 - overhead();
    let items = [messages_item(each), messages_item(each)];
    let safe = HashSet::from(["0".to_owned()]);
    assert_eq!(page_start(&items, &safe, true).unwrap(), 1);
    let near = [messages_item(PAGE_BYTES - 1024 - 2 - overhead())];
    assert!(super::super::page::full(&near));
    let under = [messages_item(PAGE_BYTES - 1024 - 3 - overhead())];
    assert!(!super::super::page::full(&under));
}

#[test]
fn outline_revisions_reach_previews_and_deferred_cards() {
    let mut source = vec![
        row(0, "user"),
        agent(1, "a"),
        child(row(2, "thinking"), "a"),
    ];
    source.extend((3..9).map(|n| row(n, "thinking")));
    let mut fake = Fake::outlined(source);
    fake.running = true;
    fake.revision = Some("r".into());
    let mut reader = Reader::default();
    let page = reader.page(&mut fake, "s", None, true).unwrap();
    let suffix = format!(":{}:r", reader.epoch);
    let mut seen = 0;
    for item in &page.items {
        match item {
            Item::Work { summary, .. } => {
                let preview = summary.preview.as_ref().expect("a running preview");
                assert!(preview.revision.ends_with(&suffix));
                seen += 1;
            }
            Item::Messages {
                deferred: Some(d), ..
            } => {
                assert!(d.revision.ends_with(&suffix));
                seen += 1;
            }
            Item::Messages { .. } => {}
        }
    }
    assert_eq!(seen, 2);
}

#[test]
fn a_stored_copy_of_a_live_row_is_not_shown_twice() {
    let mut fake = Fake::new(vec![row(0, "user"), row(1, "thinking")]);
    fake.running = true;
    fake.live = vec![with(row(1, "thinking"), "id", Js::Str(format!("{LIVE}c1")))];
    let page = Reader::default().page(&mut fake, "s", None, false).unwrap();
    let work = summary(&page.items[1]);
    assert_eq!(work.message_count, 1);
    assert!(work.stored.is_none());
}

#[test]
fn a_live_cursor_reads_from_the_newest_row() {
    let mut fake = Fake::new(vec![row(0, "user"), row(1, "thinking"), row(2, "user")]);
    fake.live = vec![with(row(9, "thinking"), "id", Js::Str(format!("{LIVE}c9")))];
    let page = Reader::default()
        .page(&mut fake, "s", Some(&format!("{LIVE}c9")), false)
        .unwrap();
    assert!(messages(&page.items).iter().any(|m| text(m, "id") == "2"));
}

#[test]
fn an_unsealed_cursor_answer_gives_no_context() {
    let source = vec![
        row(0, "user"),
        row(1, "thinking"),
        say(2, "Progress", r#"{"turnCompleted":true}"#),
        row(3, "thinking"),
        say(4, "Still building", "{}"),
    ];
    let want = crate::history_view::project(&source[..4], false, false);
    let mut fake = Fake::new(source);
    let page = Reader::default()
        .page(&mut fake, "s", Some("4"), false)
        .unwrap();
    assert_eq!(without_revision(&page.items), without_revision(&want));
}

#[test]
fn an_older_page_never_streams() {
    let mut fake = Fake::new(vec![row(0, "user"), row(1, "thinking"), row(2, "user")]);
    fake.running = true;
    let page = Reader::default()
        .page(&mut fake, "s", Some("2"), false)
        .unwrap();
    assert!(!summary(&page.items[1]).is_streaming);
}

#[test]
fn hydration_covers_the_item_target_not_the_byte_cut() {
    let mut source = vec![row(0, "user")];
    for t in 0..6 {
        source.push(row(1 + t * 2, "thinking"));
        source.push(say(
            2 + t * 2,
            &format!("# Report\n{}", "x".repeat(300_000)),
            r#"{"turnCompleted":true}"#,
        ));
    }
    let mut fake = Fake::outlined(source);
    let page = Reader::default().page(&mut fake, "s", None, true).unwrap();
    assert!(page.has_more);
    let shown: Vec<&str> = messages(&page.items)
        .iter()
        .map(|m| text(m, "id"))
        .collect();
    assert!(fake.hydrated.iter().any(|h| !shown.contains(&h.as_str())));
}

#[test]
fn a_validating_source_refuses_a_range_cleared_mid_batch() {
    let mut fake = Fake::new((1..=3).map(|n| sized(n, 1000)).collect());
    fake.validates = true;
    fake.on_read = Some(Box::new(|source| source.truncate(1)));
    fake.once = true;
    let refused = code(Reader::default().details(&mut fake, "s", &range("1", "3"), None));
    assert_eq!(refused.0, "NOT_FOUND");
}

fn agent_ref(first: &str, last: &str) -> Ref {
    Ref {
        parent: Some("a".into()),
        ..range(first, last)
    }
}

fn agent_rows(size: usize) -> Vec<Row> {
    (1..=4).map(|n| child(sized(n, size), "a")).collect()
}

#[test]
fn subagent_ranges_and_cursors_are_checked() {
    let mut fake = Fake::outlined(agent_rows(10));
    let mut reader = Reader::default();
    let backwards = code(reader.details(&mut fake, "s", &agent_ref("3", "1"), None));
    assert_eq!(backwards.1, "[INVALID_PARAMS] Invalid history range");
    let early = code(reader.details(&mut fake, "s", &agent_ref("2", "4"), Some("1")));
    assert_eq!(early.1, "[INVALID_PARAMS] Invalid detail cursor");
    let early = code(reader.details(&mut fake, "s", &range("2", "4"), Some("1")));
    assert_eq!(early.1, "[INVALID_PARAMS] Invalid detail cursor");
}

#[test]
fn a_revision_moving_during_the_scan_caches_nothing() {
    let mut fake = Fake::outlined(agent_rows(10));
    fake.revision = Some("a".into());
    fake.revision_after_read = Some("b".into());
    let mut reader = Reader::default();
    reader
        .details(&mut fake, "s", &agent_ref("1", "4"), None)
        .unwrap();
    fake.revision = Some("a".into());
    reader
        .details(&mut fake, "s", &agent_ref("1", "4"), None)
        .unwrap();
    assert_eq!(fake.outlines, 2);
}

#[test]
fn the_detail_cache_holds_two_ranges() {
    let mut fake = Fake::outlined(agent_rows(10));
    fake.revision = Some("r".into());
    let mut reader = Reader::default();
    for first in ["1", "2", "3", "1"] {
        reader
            .details(&mut fake, "s", &agent_ref(first, "4"), None)
            .unwrap();
    }
    assert_eq!(fake.outlines, 4);
    reader
        .details(&mut fake, "s", &agent_ref("3", "4"), None)
        .unwrap();
    assert_eq!(fake.outlines, 4);
}

#[test]
fn one_oversized_subagent_row_is_its_own_page() {
    let mut fake = Fake::outlined(agent_rows(300_000));
    let page = Reader::default()
        .details(&mut fake, "s", &agent_ref("1", "4"), None)
        .unwrap();
    assert_eq!(ids(&page.messages), ["1"]);
    assert_eq!(page.next_cursor.as_deref(), Some("1"));
}

#[test]
fn a_row_past_the_last_rowid_in_its_millisecond_is_a_changed_range() {
    let at = crate::history_view::field(&row(2, "thinking"), "createdAt")
        .cloned()
        .unwrap();
    let mut fake = Fake::new(vec![row(1, "thinking"), row(2, "thinking")]);
    fake.list_value = Some(vec![with(row(3, "thinking"), "createdAt", at)]);
    let refused = code(Reader::default().details(&mut fake, "s", &range("1", "2"), None));
    assert_eq!(refused.0, "NOT_FOUND");
}

#[test]
fn a_live_cursor_skips_the_stored_part() {
    let mut fake = Fake::new((1..=3).map(|n| sized(n, 10)).collect());
    let live: Vec<String> = (4..=6).map(|n| format!("{LIVE}c{n}")).collect();
    fake.anchor_fallback = Some(|_, id| {
        let n = id.trim_start_matches("history-live:c");
        with(
            with(
                row(n.parse().unwrap(), "thinking"),
                "id",
                Js::Str(id.into()),
            ),
            "content",
            Js::Str("x".into()),
        )
    });
    let r = Ref {
        key: "w".into(),
        first: "1".into(),
        last: live[2].clone(),
        first_stored: Some("1".into()),
        last_stored: Some("3".into()),
        live: live.clone(),
        parent: None,
    };
    let page = Reader::default()
        .details(&mut fake, "s", &r, Some(&live[0]))
        .unwrap();
    assert_eq!(ids(&page.messages), &live[1..]);
}

fn live_copy(_: &[Row], id: &str) -> Row {
    let n: usize = id.trim_start_matches("history-live:c").parse().unwrap();
    child(with(sized(n, 10), "id", Js::Str(id.into())), "a")
}

#[test]
fn a_live_row_already_stored_is_read_once() {
    let mut fake = Fake::new((1..=3).map(|n| sized(n, 10)).collect());
    fake.anchor_fallback = Some(live_copy);
    let r = Ref {
        first_stored: Some("1".into()),
        last_stored: Some("3".into()),
        live: vec![format!("{LIVE}c3")],
        ..range("1", &format!("{LIVE}c3"))
    };
    let page = Reader::default().details(&mut fake, "s", &r, None).unwrap();
    assert_eq!(ids(&page.messages), ["1", "2", "3"]);
}

#[test]
fn a_subagent_live_row_already_stored_is_scoped_once() {
    let mut fake = Fake::new(agent_rows(10));
    fake.anchor_fallback = Some(live_copy);
    let r = Ref {
        live: vec![format!("{LIVE}c4")],
        ..agent_ref("1", "4")
    };
    let page = Reader::default().details(&mut fake, "s", &r, None).unwrap();
    assert_eq!(ids(&page.messages), ["1", "2", "3", "4"]);
}
