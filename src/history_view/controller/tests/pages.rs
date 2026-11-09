use super::*;

/// `makePage` of `historyViewLoading.test.ts`: one user row per id.
fn ids_page(ids: &[usize], has_more: bool, same_time: bool) -> Page<Row> {
    let items = ids
        .iter()
        .map(|&id| {
            let mut message = row(id, "user", &id.to_string());
            if let Js::Obj(o) = &mut message {
                o.insert("clientId".into(), Js::Str(id.to_string()));
                let at = 1_700_000_000_000 + if same_time { 0 } else { id as i64 * 1000 };
                o.insert(
                    "createdAt".into(),
                    Js::Str(crate::js_date::iso(at).unwrap()),
                );
            }
            Item::Messages {
                key: id.to_string(),
                messages: vec![message],
                deferred: None,
            }
        })
        .collect();
    let next = has_more.then(|| ids[0].to_string());
    page(items, has_more, next.as_deref())
}

fn item_keys(view: &View<Row>) -> Vec<String> {
    view.snapshot()
        .items
        .iter()
        .map(|i| i.key().to_owned())
        .collect()
}

#[gpui::test]
async fn the_cached_window_stays_until_a_disconnected_refresh_settles(cx: &mut TestAppContext) {
    for outcome in ["join", "same-time", "failure", "blur", "repeated-cursor"] {
        let same = outcome == "same-time";
        let fake = Fake::fixed(Vec::new());
        let (finish, reply) = gate();
        fake.pages
            .borrow_mut()
            .push_back(ready(Ok(ids_page(&[1, 2, 3], false, same))));
        fake.pages
            .borrow_mut()
            .push_back(ready(Ok(ids_page(&[7, 8], true, same))));
        fake.pages.borrow_mut().push_back(reply);
        let view = view(&fake, cx);
        view.refresh(false, false).await;
        let visible: Rc<RefCell<Vec<Vec<String>>>> = Rc::default();
        let (seen, watched) = (visible.clone(), view.clone());
        let _stop = view.subscribe(move || seen.borrow_mut().push(item_keys(&watched)));
        let pending = view.refresh(false, false);
        cx.run_until_parked();
        assert_eq!(fake.page_calls.borrow().len(), 3, "{outcome}");
        assert_eq!(item_keys(&view), ["1", "2", "3"]);
        if outcome == "blur" {
            view.set_active(false);
        }
        let _ = finish.send(match outcome {
            "failure" => Err("temporary read failure".into()),
            "repeated-cursor" => Ok(ids_page(&[7, 8], true, same)),
            _ => Ok(ids_page(&[3, 4, 5, 6], true, same)),
        });
        pending.await;
        let want: Vec<String> = match outcome {
            "join" | "same-time" => (1..=8).map(|i| i.to_string()).collect(),
            _ => vec!["1".into(), "2".into(), "3".into()],
        };
        assert_eq!(item_keys(&view), want, "{outcome}");
        assert!(visible.borrow().iter().all(|keys| keys[0] == "1"));
        assert!(!view.snapshot().has_more);
        if matches!(outcome, "failure" | "repeated-cursor") {
            assert!(view.snapshot().error.is_some(), "{outcome}");
        }
    }
}

#[gpui::test]
async fn the_latest_read_reports_its_start_and_flight(cx: &mut TestAppContext) {
    let fake = Fake::new(|_| ready(Ok(page(Vec::new(), true, Some("older")))));
    let view = view(&fake, cx);
    assert_eq!(view.latest_read_started_at(), 0.0);
    assert!(!view.is_reading_latest());
    let (finish, reply) = gate();
    fake.pages.borrow_mut().push_back(reply);
    let before = crate::js_date::now();
    let latest = view.refresh(false, false);
    assert!(view.is_reading_latest());
    assert!(view.latest_read_started_at() >= before);
    let _ = finish.send(Ok(page(Vec::new(), true, Some("older"))));
    latest.await;
    assert!(!view.is_reading_latest());
    let started = view.latest_read_started_at();
    let (finish, reply) = gate();
    fake.pages.borrow_mut().push_back(reply);
    let older = view.refresh(true, false);
    assert!(!view.is_reading_latest());
    assert_eq!(view.latest_read_started_at(), started);
    let _ = finish.send(Ok(page(Vec::new(), true, Some("older"))));
    older.await;
}

#[gpui::test]
async fn only_an_applied_latest_read_is_certified(cx: &mut TestAppContext) {
    let fake = Fake::new(|_| ready(Ok(page(Vec::new(), true, Some("older")))));
    let view = view(&fake, cx);
    assert_eq!(view.latest_read_ok_started_at(), 0.0);
    fake.pages
        .borrow_mut()
        .push_back(ready(Err("link hiccup".into())));
    view.refresh(false, false).await;
    assert!(view.latest_read_started_at() > 0.0);
    assert_eq!(view.latest_read_ok_started_at(), 0.0);
    view.refresh(false, false).await;
    assert_eq!(
        view.latest_read_ok_started_at(),
        view.latest_read_started_at()
    );
}

#[gpui::test]
async fn an_older_read_needs_more_history(cx: &mut TestAppContext) {
    let fake = Fake::fixed(project(&[row(1, "user", "all")], false, false));
    let view = view(&fake, cx);
    view.refresh(false, false).await;
    view.refresh(true, false).await;
    assert_eq!(fake.page_calls.borrow().len(), 1);
}

#[gpui::test]
async fn an_older_page_after_a_blur_changes_nothing(cx: &mut TestAppContext) {
    let first = project(&[row(2, "user", "latest")], false, false);
    let fake = Fake::new({
        let first = first.clone();
        move |_| ready(Ok(page(first.clone(), true, Some("2"))))
    });
    let view = view(&fake, cx);
    view.refresh(false, false).await;
    let (finish, reply) = gate();
    fake.pages.borrow_mut().push_back(reply);
    let older = view.refresh(true, false);
    view.set_active(false);
    let _ = finish.send(Ok(page(
        project(&[row(1, "user", "older")], false, false),
        false,
        None,
    )));
    older.await;
    let s = view.snapshot();
    assert_eq!(s.items, first);
    assert!(s.has_more);
}

#[gpui::test]
async fn an_older_page_keeps_one_copy_of_each_item(cx: &mut TestAppContext) {
    let latest = project(&[row(2, "user", "latest")], false, false);
    let fake = Fake::new({
        let latest = latest.clone();
        move |_| ready(Ok(page(latest.clone(), true, Some("2"))))
    });
    let view = view(&fake, cx);
    view.refresh(false, false).await;
    let mut older = project(&[row(1, "user", "older")], false, false);
    older.extend(latest.clone());
    fake.pages
        .borrow_mut()
        .push_back(ready(Ok(page(older.clone(), false, None))));
    view.refresh(true, false).await;
    assert_eq!(view.snapshot().items, older);
}

#[gpui::test]
async fn a_blur_marks_ready_history_stale_until_the_next_latest_page(cx: &mut TestAppContext) {
    let fake = Fake::fixed(project(&[row(1, "user", "hello")], false, false));
    let view = view(&fake, cx);
    view.refresh(false, false).await;
    view.set_active(false);
    assert!(view.snapshot().stale);
    view.set_active(true);
    cx.run_until_parked();
    assert!(!view.snapshot().stale);
    assert_eq!(fake.page_calls.borrow().len(), 2);
}

#[gpui::test]
async fn activation_and_reset_read_on_their_own(cx: &mut TestAppContext) {
    let fake = Fake::fixed(project(&[row(1, "user", "hello")], false, false));
    let view = view(&fake, cx);
    view.set_active(true);
    cx.run_until_parked();
    assert!(view.snapshot().ready);
    view.reset();
    assert!(!view.snapshot().ready);
    cx.run_until_parked();
    assert!(view.snapshot().ready);
    assert_eq!(fake.page_calls.borrow().len(), 2);
}

#[gpui::test]
async fn a_downgrade_ends_a_queued_ack_read(cx: &mut TestAppContext) {
    let fake = Fake::fixed(Vec::new());
    let (finish, reply) = gate();
    fake.pages.borrow_mut().push_back(reply);
    let view = view(&fake, cx);
    let first = view.refresh(false, false);
    let ack = view.refresh(false, true);
    let _ = finish.send(Err("[CHANNEL_NOT_ALLOWED] old Host".into()));
    join_all([first, ack]).await;
    assert_eq!(fake.page_calls.borrow().len(), 1);
}

#[test]
fn a_complete_page_replaces_the_loaded_prefix() {
    let previous = Snapshot {
        items: project(&[row(1, "user", "a")], false, false)
            .into_iter()
            .chain(project(&[row(2, "user", "b")], false, false))
            .collect(),
        has_more: true,
        ..Snapshot::default()
    };
    let latest = || {
        page(
            project(&[row(2, "user", "b edited")], false, false),
            false,
            None,
        )
    };
    assert_eq!(continuity::retain_prefix(latest(), &previous), latest());
}
