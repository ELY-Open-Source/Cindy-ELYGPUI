use super::*;

#[gpui::test]
async fn locate_reads_only_the_target_range_without_opening_it(cx: &mut TestAppContext) {
    for mode in ["mounted", "older-page", "inactive", "changed"] {
        let target = row(1, "thinking", "target");
        let items = Rc::new(RefCell::new(project(
            std::slice::from_ref(&target),
            false,
            false,
        )));
        let pages = Rc::new(Cell::new(0));
        let (source, count) = (items.clone(), pages.clone());
        let fake = Fake::new(move |_| {
            let n = count.get();
            count.set(n + 1);
            let empty = mode == "older-page" && n == 0;
            let shown = if empty {
                Vec::new()
            } else {
                source.borrow().clone()
            };
            ready(Ok(page(
                shown,
                mode == "older-page" && n == 0,
                Some("older"),
            )))
        });
        let (finish, reply) = gate();
        fake.details.borrow_mut().push_back(reply);
        let view = view(&fake, cx);
        view.refresh(false, false).await;
        let pending = {
            let (view, target) = (view.clone(), target.clone());
            cx.foreground_executor()
                .spawn(async move { view.locate(target.client_id(), target.created_at()).await })
        };
        cx.run_until_parked();
        let key = items.borrow()[0].key().to_owned();
        view.set_expanded(&key, false);
        if mode == "inactive" {
            view.set_active(false);
        }
        if mode == "changed" {
            let mut changed = view.snapshot().items;
            if let Item::Work { summary, .. } = &mut changed[0] {
                summary.revision = "changed".into();
            }
            view.publish(
                Patch {
                    items: Some(changed),
                    ..Patch::default()
                },
                None,
            );
        }
        let _ = finish.send(Ok(detail(vec![target.clone()], None)));
        let found = pending.await.unwrap();
        let lost = matches!(mode, "inactive" | "changed");
        assert_eq!(found, (!lost).then(|| target.clone()), "{mode}");
        let s = view.snapshot();
        assert!(s.expanded.is_empty());
        match lost {
            true => assert!(s.details.is_empty(), "{mode}"),
            false => assert!(s.details[&key].complete, "{mode}"),
        }
    }
}

#[gpui::test]
async fn a_located_range_with_a_running_preview_keeps_the_preview_open(cx: &mut TestAppContext) {
    let rows: Vec<Row> = (0..8)
        .map(|i| row(i + 1, "thinking", &format!("detail-{i}")))
        .collect();
    let mut items = project(&rows, true, false);
    if let Item::Work { summary, .. } = &mut items[0] {
        let mut preview = summary.clone();
        preview.key = "preview".into();
        preview.first_message_id = rows[3].id().into();
        summary.preview = Some(Box::new(preview));
    }
    let fake = Fake::fixed(items);
    let all = rows.clone();
    *fake.detail_default.borrow_mut() = Box::new(move |r, _| {
        let messages = if r.key == "preview" {
            all[3..].to_vec()
        } else {
            all.clone()
        };
        ready(Ok(detail(messages, None)))
    });
    let view = view(&fake, cx);
    view.refresh(false, false).await;
    view.set_expanded("preview", true);
    cx.run_until_parked();
    let found = view
        .locate(rows[0].client_id(), rows[0].created_at())
        .await
        .unwrap();
    assert_eq!(found.as_ref(), Some(&rows[0]));
    assert_eq!(
        view.snapshot().expanded.iter().collect::<Vec<_>>(),
        ["preview"]
    );
}

#[gpui::test]
async fn an_in_flight_expansion_never_replaces_a_located_range(cx: &mut TestAppContext) {
    let rows = vec![row(1, "thinking", "one"), row(2, "thinking", "two")];
    let items = project(&rows, false, false);
    let fake = Fake::fixed(items.clone());
    let (finish, reply) = gate();
    fake.details.borrow_mut().push_back(reply);
    let all = rows.clone();
    *fake.detail_default.borrow_mut() = Box::new(move |_, _| ready(Ok(detail(all.clone(), None))));
    let view = view(&fake, cx);
    view.refresh(false, false).await;
    let key = items[0].key().to_owned();
    view.set_expanded(&key, true);
    let found = view
        .locate(rows[1].client_id(), rows[1].created_at())
        .await
        .unwrap();
    assert_eq!(found.as_ref(), Some(&rows[1]));
    let observed: Rc<RefCell<Vec<usize>>> = Rc::default();
    let (seen, watched, watched_key) = (observed.clone(), view.clone(), key.clone());
    let stop = view.subscribe(move || {
        let count = watched
            .snapshot()
            .details
            .get(&watched_key)
            .map_or(0, |d| d.messages.len());
        seen.borrow_mut().push(count);
    });
    let _ = finish.send(Ok(detail(vec![rows[0].clone()], Some("1"))));
    cx.run_until_parked();
    assert!(observed.borrow().iter().all(|c| *c == 2));
    stop();
}

#[gpui::test]
async fn only_a_current_locate_reports_its_error(cx: &mut TestAppContext) {
    for inactive in [false, true] {
        let target = row(1, "thinking", "target");
        let fake = Fake::fixed(project(std::slice::from_ref(&target), false, false));
        let (fail, reply) = gate();
        fake.details.borrow_mut().push_back(reply);
        let view = view(&fake, cx);
        view.refresh(false, false).await;
        let request = {
            let (view, target) = (view.clone(), target.clone());
            cx.foreground_executor()
                .spawn(async move { view.locate(target.client_id(), target.created_at()).await })
        };
        cx.run_until_parked();
        if inactive {
            view.set_active(false);
        }
        let _ = fail.send(Err("read failed".into()));
        let result = request.await;
        match inactive {
            true => assert_eq!(result, Ok(None)),
            false => assert_eq!(result, Err("read failed".into())),
        }
    }
}

#[gpui::test]
async fn a_warm_body_of_an_older_revision_is_read_again(cx: &mut TestAppContext) {
    let target = row(1, "thinking", "target");
    let items = Rc::new(RefCell::new(project(
        std::slice::from_ref(&target),
        false,
        false,
    )));
    let source = items.clone();
    let fake = Fake::new(move |_| ready(Ok(page(source.borrow().clone(), false, None))));
    let reply = target.clone();
    *fake.detail_default.borrow_mut() =
        Box::new(move |_, _| ready(Ok(detail(vec![reply.clone()], None))));
    let view = view(&fake, cx);
    view.refresh(false, false).await;
    let key = items.borrow()[0].key().to_owned();
    view.set_expanded(&key, true);
    cx.run_until_parked();
    view.set_expanded(&key, false);
    if let Item::Work { summary, .. } = &mut items.borrow_mut()[0] {
        summary.revision = "edited".into();
    }
    view.refresh(false, false).await;
    assert!(view.snapshot().details[&key].complete);
    let found = view
        .locate(target.client_id(), target.created_at())
        .await
        .unwrap();
    assert_eq!(found, Some(target));
    assert_eq!(fake.detail_calls.borrow().len(), 2);
    assert_eq!(view.snapshot().details[&key].revision, "edited");
}

#[gpui::test]
async fn a_failed_older_page_ends_the_search_with_its_error(cx: &mut TestAppContext) {
    let fake = Fake::fixed(Vec::new());
    fake.pages
        .borrow_mut()
        .push_back(ready(Ok(page(Vec::new(), true, Some("older")))));
    fake.pages
        .borrow_mut()
        .push_back(ready(Err("older read failed".into())));
    let view = view(&fake, cx);
    view.refresh(false, false).await;
    let target = row(1, "thinking", "target");
    let result = view.locate(target.client_id(), target.created_at()).await;
    assert_eq!(result, Err("older read failed".into()));
}
