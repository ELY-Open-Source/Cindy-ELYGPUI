use super::*;

fn work(view: &View<Row>, index: usize) -> Summary {
    match &view.snapshot().items[index] {
        Item::Work { summary, .. } => summary.clone(),
        Item::Messages { .. } => panic!("a work group"),
    }
}

#[gpui::test]
async fn the_detail_walk_reads_every_page(_cx: &mut TestAppContext) {
    let cursors: RefCell<Vec<Option<String>>> = RefCell::default();
    let mut received = Vec::new();
    let read = |cursor: Option<String>| {
        cursors.borrow_mut().push(cursor.clone());
        let first = cursor.is_none();
        let n = if first { 1 } else { 2 };
        ready(Ok(detail(
            vec![row(n, "thinking", "detail")],
            first.then_some("1"),
        )))
    };
    continuity::work_details(read, || true, |rows| received.extend(rows))
        .await
        .unwrap();
    assert_eq!(*cursors.borrow(), [None, Some("1".into())]);
    assert_eq!(fake_ids(&received), ["1", "2"]);
}

fn fake_ids(rows: &[Row]) -> Vec<String> {
    rows.iter().map(|r| r.id().to_owned()).collect()
}

#[gpui::test]
async fn the_detail_walk_publishes_no_late_page(_cx: &mut TestAppContext) {
    let current = Cell::new(true);
    let mut received = Vec::new();
    let read = |_| {
        current.set(false);
        ready(Ok(detail(vec![row(1, "thinking", "late")], None)))
    };
    continuity::work_details(read, || current.get(), |rows| received.push(rows))
        .await
        .unwrap();
    assert!(received.is_empty());
}

#[gpui::test]
async fn a_host_downgrade_clears_the_projection_and_late_details(cx: &mut TestAppContext) {
    let fake = Fake::fixed(project(&[row(1, "thinking", "old")], true, false));
    let (send, reply) = gate();
    fake.details.borrow_mut().push_back(reply);
    let view = view(&fake, cx);
    view.refresh(false, false).await;
    let key = view.snapshot().items[0].key().to_owned();
    view.set_expanded(&key, true);
    fake.pages
        .borrow_mut()
        .push_back(ready(Err("timeout".into())));
    view.refresh(false, false).await;
    assert!(view.snapshot().ready);
    fake.pages
        .borrow_mut()
        .push_back(ready(Err("[CHANNEL_NOT_ALLOWED] old Host".into())));
    view.refresh(false, false).await;
    let _ = send.send(Ok(detail(vec![row(1, "thinking", "late")], None)));
    cx.run_until_parked();
    let s = view.snapshot();
    assert!(!s.ready && s.items.is_empty() && !s.has_more && s.next_cursor.is_none());
    assert!(s.details.is_empty() && s.expanded.is_empty());
}

#[gpui::test]
async fn late_details_for_a_removed_range_are_dropped(cx: &mut TestAppContext) {
    let rows = Rc::new(RefCell::new(vec![row(1, "thinking", "old")]));
    let source = rows.clone();
    let fake = Fake::new(move |_| {
        ready(Ok(page(
            project(&source.borrow(), false, false),
            false,
            None,
        )))
    });
    let (send, reply) = gate();
    fake.details.borrow_mut().push_back(reply);
    let view = view(&fake, cx);
    view.refresh(false, false).await;
    let group = work(&view, 0);
    view.set_expanded(&group.key, true);
    let pending = view.load_details(group, false);
    *rows.borrow_mut() = vec![row(1, "assistant", "updated")];
    view.refresh(false, false).await;
    assert!(summaries(&view.snapshot().items).is_empty());
    let _ = send.send(Ok(detail(vec![row(1, "thinking", "late body")], None)));
    pending.await;
    assert!(view.snapshot().details.is_empty());
}

#[gpui::test]
async fn a_collapse_mid_read_releases_a_large_partial_body(cx: &mut TestAppContext) {
    let body = row(1, "thinking", &"x".repeat(1024 * 1024));
    let fake = Fake::fixed(project(std::slice::from_ref(&body), false, false));
    let (finish, reply) = gate();
    fake.details
        .borrow_mut()
        .push_back(ready(Ok(detail(vec![body], Some("next")))));
    fake.details.borrow_mut().push_back(reply);
    let view = view(&fake, cx);
    view.refresh(false, false).await;
    let group = work(&view, 0);
    view.set_expanded(&group.key, true);
    let pending = view.load_details(group.clone(), false);
    cx.run_until_parked();
    assert_eq!(view.snapshot().details[&group.key].messages.len(), 1);
    view.set_expanded(&group.key, false);
    assert!(view.snapshot().details.is_empty());
    let _ = finish.send(Ok(detail(Vec::new(), None)));
    pending.await;
    assert!(view.snapshot().details.is_empty());
}

#[gpui::test]
async fn an_expanded_read_joins_through_its_latest_revision(cx: &mut TestAppContext) {
    for changed in [false, true] {
        let rows = Rc::new(RefCell::new(vec![row(1, "thinking", "old")]));
        let source = rows.clone();
        let fake = Fake::new(move |_| {
            ready(Ok(page(
                project(&source.borrow(), true, false),
                false,
                None,
            )))
        });
        let replies: Senders<DetailPage<Row>> = Rc::default();
        let sends = replies.clone();
        *fake.detail_default.borrow_mut() = Box::new(move |_, _| {
            let (send, reply) = gate();
            sends.borrow_mut().push(send);
            reply
        });
        let view = view(&fake, cx);
        view.refresh(false, false).await;
        let group = work(&view, 0);
        view.set_expanded(&group.key, true);
        let completed = Rc::new(Cell::new(false));
        let (flag, joined) = (completed.clone(), view.load_details(group.clone(), false));
        let watched = joined.clone();
        cx.foreground_executor()
            .spawn(async move {
                watched.await;
                flag.set(true)
            })
            .detach();
        cx.run_until_parked();
        assert!(!completed.get());
        assert_eq!(fake.detail_calls.borrow().len(), 1);
        if changed {
            *rows.borrow_mut() = vec![
                row(1, "thinking", "corrected"),
                row(2, "thinking", "appended"),
            ];
            view.refresh(false, false).await;
            assert_eq!(fake.detail_calls.borrow().len(), 1);
        }
        let first = replies.borrow_mut().remove(0);
        let _ = first.send(Ok(detail(vec![row(1, "thinking", "old")], None)));
        if changed {
            cx.run_until_parked();
            assert!(!completed.get());
            assert_eq!(fake.detail_calls.borrow().len(), 2);
            let second = replies.borrow_mut().remove(0);
            let _ = second.send(Ok(detail(rows.borrow().clone(), None)));
        }
        joined.await;
        let d = view.snapshot().details[&group.key].clone();
        assert!(d.complete && !d.loading);
        assert_eq!(d.messages, *rows.borrow());
        view.load_details(work(&view, 0), false).await;
        assert_eq!(
            fake.detail_calls.borrow().len(),
            if changed { 2 } else { 1 }
        );
    }
}

#[gpui::test]
async fn new_rows_in_the_same_revision_reload_an_edited_prefix(cx: &mut TestAppContext) {
    let rows = Rc::new(RefCell::new(vec![
        row(1, "thinking", "old"),
        row(2, "thinking", "tail"),
    ]));
    let (source, detail_rows) = (rows.clone(), rows.clone());
    let fake = Fake::new(move |_| {
        ready(Ok(page(
            project(&source.borrow(), false, false),
            false,
            None,
        )))
    });
    *fake.detail_default.borrow_mut() = Box::new(move |_, after| {
        let after: Option<usize> = after.map(|a| a.parse().unwrap());
        let kept = detail_rows
            .borrow()
            .iter()
            .filter(|r| after.is_none_or(|a| r.id().parse::<usize>().unwrap() > a))
            .cloned()
            .collect();
        ready(Ok(detail(kept, None)))
    });
    let view = view(&fake, cx);
    view.refresh(false, false).await;
    let key = view.snapshot().items[0].key().to_owned();
    view.set_expanded(&key, true);
    cx.run_until_parked();
    let tail = rows.borrow()[1].clone();
    *rows.borrow_mut() = vec![
        row(1, "thinking", "corrected"),
        tail,
        row(3, "thinking", "appended"),
    ];
    view.refresh(false, false).await;
    cx.run_until_parked();
    assert_eq!(*fake.detail_calls.borrow(), [None, None]);
    let contents: Vec<String> = view.snapshot().details[&key]
        .messages
        .iter()
        .map(|m| crate::history_view::text(m, "content").to_owned())
        .collect();
    assert_eq!(contents, ["corrected", "tail", "appended"]);
    view.refresh(false, false).await;
    assert_eq!(fake.detail_calls.borrow().len(), 2);
}

#[gpui::test]
async fn a_first_read_survives_repeated_activation(cx: &mut TestAppContext) {
    let (send, reply) = gate();
    let fake = Fake::fixed(Vec::new());
    fake.pages.borrow_mut().push_back(reply);
    *fake.detail_default.borrow_mut() =
        Box::new(|_, _| ready(Ok(detail(vec![row(1, "thinking", "body")], None))));
    let view = view(&fake, cx);
    let pending = view.refresh(false, false);
    view.set_active(true);
    view.set_active(true);
    let items = project(&[row(1, "thinking", "body")], true, false);
    let _ = send.send(Ok(page(items.clone(), false, None)));
    pending.await;
    assert!(view.snapshot().ready);
    view.set_expanded(items[0].key(), true);
    cx.run_until_parked();
    assert!(view.snapshot().details[items[0].key()].complete);
}

#[gpui::test]
async fn a_collapsed_range_loads_for_recovery_without_expanding(cx: &mut TestAppContext) {
    let target = row(1, "thinking", "authoritative terminal");
    let fake = Fake::fixed(project(std::slice::from_ref(&target), false, false));
    let reply = target.clone();
    *fake.detail_default.borrow_mut() =
        Box::new(move |_, _| ready(Ok(detail(vec![reply.clone()], None))));
    let view = view(&fake, cx);
    view.refresh(false, false).await;
    let group = work(&view, 0);
    view.load_details(group.clone(), true).await;
    assert_eq!(fake.detail_calls.borrow().len(), 1);
    let s = view.snapshot();
    assert!(s.expanded.is_empty());
    assert!(s.details[&group.key].complete);
    assert_eq!(s.details[&group.key].messages, [target]);
}

#[gpui::test]
async fn holds_keep_a_range_open_until_the_last_release(cx: &mut TestAppContext) {
    let items = project(&[row(1, "thinking", "body")], true, false);
    let fake = Fake::fixed(items.clone());
    let view = view(&fake, cx);
    view.refresh(false, false).await;
    let key = items[0].key().to_owned();
    let first = view.retain_expanded(vec![key.clone()]);
    let second = view.retain_expanded(vec![key.clone()]);
    first();
    assert!(view.snapshot().expanded.contains(&key));
    second();
    assert!(view.snapshot().expanded.is_empty());
}

#[gpui::test]
async fn an_unchanged_expansion_publishes_nothing(cx: &mut TestAppContext) {
    let items = project(&[row(1, "thinking", "body")], false, false);
    let fake = Fake::fixed(items.clone());
    let view = view(&fake, cx);
    view.refresh(false, false).await;
    let key = items[0].key().to_owned();
    view.set_expanded(&key, true);
    cx.run_until_parked();
    let published = Rc::new(Cell::new(0));
    let count = published.clone();
    let _stop = view.subscribe(move || count.set(count.get() + 1));
    view.set_expanded(&key, true);
    cx.run_until_parked();
    assert_eq!(published.get(), 0);
}

#[gpui::test]
async fn a_running_range_keeps_a_large_body_when_collapsed(cx: &mut TestAppContext) {
    let body = row(1, "thinking", &"x".repeat(1024 * 1024));
    let items = project(std::slice::from_ref(&body), true, false);
    let fake = Fake::fixed(items.clone());
    *fake.detail_default.borrow_mut() =
        Box::new(move |_, _| ready(Ok(detail(vec![body.clone()], None))));
    let view = view(&fake, cx);
    view.refresh(false, false).await;
    let key = items[0].key().to_owned();
    view.set_expanded(&key, true);
    cx.run_until_parked();
    view.set_expanded(&key, false);
    assert!(view.snapshot().details[&key].complete);
}

#[gpui::test]
async fn a_collapsed_range_reads_only_for_recovery(cx: &mut TestAppContext) {
    let fake = Fake::fixed(project(&[row(1, "thinking", "body")], false, false));
    let view = view(&fake, cx);
    view.refresh(false, false).await;
    view.load_details(work(&view, 0), false).await;
    assert!(fake.detail_calls.borrow().is_empty());
    assert!(view.snapshot().details.is_empty());
}

#[gpui::test]
async fn a_read_from_before_a_collapse_never_lands(cx: &mut TestAppContext) {
    let items = project(&[row(1, "thinking", "body")], false, false);
    let fake = Fake::fixed(items.clone());
    let (old, first) = gate();
    let (_current, second) = gate();
    fake.details.borrow_mut().extend([first, second]);
    let view = view(&fake, cx);
    view.refresh(false, false).await;
    let key = items[0].key().to_owned();
    view.set_expanded(&key, true);
    view.set_expanded(&key, false);
    view.set_expanded(&key, true);
    let _ = old.send(Ok(detail(vec![row(1, "thinking", "late")], None)));
    cx.run_until_parked();
    let d = &view.snapshot().details[&key];
    assert!(d.messages.is_empty() && d.loading);
}

#[gpui::test]
async fn a_paginated_reread_keeps_the_previous_window_until_the_slid_tail_lands(
    cx: &mut TestAppContext,
) {
    let rows: Vec<Row> = (0..8)
        .map(|i| row(i + 1, "thinking", &format!("detail-{i}")))
        .collect();
    let slide = |items: &mut Vec<Item<Row>>, revision: &str, first: usize, last: usize| {
        if let Item::Work { summary, .. } = &mut items[0] {
            let mut preview = summary.clone();
            preview.key = "preview".into();
            preview.revision = revision.into();
            preview.first_message_id = rows[first].id().into();
            preview.last_message_id = rows[last].id().into();
            summary.preview = Some(Box::new(preview));
        }
    };
    let items = Rc::new(RefCell::new(project(&rows, true, false)));
    slide(&mut items.borrow_mut(), "r1", 0, 4);
    let source = items.clone();
    let fake = Fake::new(move |_| ready(Ok(page(source.borrow().clone(), false, None))));
    let reads: Senders<DetailPage<Row>> = Rc::default();
    let sends = reads.clone();
    *fake.detail_default.borrow_mut() = Box::new(move |_, _| {
        let (send, reply) = gate();
        sends.borrow_mut().push(send);
        reply
    });
    let view = view(&fake, cx);
    view.refresh(false, false).await;
    view.set_expanded("preview", true);
    let _ = reads
        .borrow_mut()
        .remove(0)
        .send(Ok(detail(rows[..5].to_vec(), None)));
    cx.run_until_parked();
    let window = |view: &View<Row>| view.snapshot().details["preview"].messages.clone();
    assert_eq!(window(&view), rows[..5]);
    slide(&mut items.borrow_mut(), "r2", 3, 7);
    view.refresh(false, false).await;
    cx.run_until_parked();
    assert_eq!(reads.borrow().len(), 1);
    let _ = reads
        .borrow_mut()
        .remove(0)
        .send(Ok(detail(vec![rows[3].clone()], Some(rows[3].id()))));
    cx.run_until_parked();
    assert_eq!(window(&view), rows[..5]);
    let _ = reads
        .borrow_mut()
        .remove(0)
        .send(Ok(detail(rows[4..].to_vec(), None)));
    cx.run_until_parked();
    assert_eq!(window(&view), rows[3..]);
}

#[gpui::test]
async fn a_recovery_read_over_a_new_revision_does_not_read_a_collapsed_range_again(
    cx: &mut TestAppContext,
) {
    let items = Rc::new(RefCell::new(project(
        &[row(1, "thinking", "old")],
        false,
        false,
    )));
    let source = items.clone();
    let fake = Fake::new(move |_| ready(Ok(page(source.borrow().clone(), false, None))));
    let (finish, reply) = gate();
    fake.details.borrow_mut().push_back(reply);
    let view = view(&fake, cx);
    view.refresh(false, false).await;
    let pending = view.load_details(work(&view, 0), true);
    if let Item::Work { summary, .. } = &mut items.borrow_mut()[0] {
        summary.revision = "edited".into();
    }
    view.refresh(false, false).await;
    let _ = finish.send(Ok(detail(vec![row(1, "thinking", "old")], None)));
    pending.await;
    assert_eq!(fake.detail_calls.borrow().len(), 1);
}

#[test]
fn characters_count_a_deep_tool_input_without_the_call_stack() {
    let mut input = Js::Str("deep".into());
    for _ in 0..50_000 {
        input = Js::Arr(vec![input]);
    }
    let mut message = row(1, "tool_use", "");
    if let Js::Obj(o) = &mut message {
        o.insert("content".into(), input);
    }
    let expected = ["1", "c1", "tool_use", "deep"]
        .iter()
        .map(|s| s.len())
        .sum::<usize>()
        + 24;
    assert_eq!(Message::characters(&message), expected);
}
