use super::{
    ForkOrigin, Rows, assistant_has_following_user_boundary, contains_client_id,
    first_user_message_client_id, group, insert_fork_origin, last_user_message_client_id,
    turn_final_assistant_client_ids,
};
use crate::{
    generated_files::tests::{self, object, refs, text},
    render_items::{Item, Options, build, created_ms},
    rich_answer::js::{self, Js},
};

fn render() -> Js {
    js::parse(include_str!(
        "../../tests/fixtures/render-items/expected.json"
    ))
    .unwrap()
}

fn cases() -> Js {
    js::parse(include_str!("../../tests/fixtures/work-groups/cases.json")).unwrap()
}

fn expected() -> Js {
    js::parse(include_str!(
        "../../tests/fixtures/work-groups/expected.json"
    ))
    .unwrap()
}

fn options(opts: Option<&Js>) -> Options<'_> {
    let opt = |key: &str| opts.and_then(Js::obj).and_then(|o| o.get(key));
    Options {
        history_window_incomplete: opt("historyWindowIncomplete") == Some(&Js::Bool(true)),
        working_dir: opt("workingDir").and_then(Js::str).unwrap_or_default(),
        history_artifacts: opt("historyArtifacts")
            .and_then(Js::arr)
            .unwrap_or_default(),
        image_targets: None,
    }
}

fn ms(value: Option<f64>) -> Option<Js> {
    Some(value.map_or(Js::Null, Js::Num))
}

/// The item as the golden's projection writes Cindy's.
fn shown(item: &Item, rows: &Rows) -> Js {
    let (kind, key) = match item {
        Item::Message { key, .. } => ("message", key),
        Item::AgentPlan { key, .. } => ("agent_plan", key),
        Item::ToolSegment { key, .. } => ("tool_segment", key),
        Item::AgentTask { key, .. } => ("agent_task", key),
        Item::ToolMedia { key, .. } => ("tool_media", key),
        Item::GeneratedFiles { key, .. } => ("generated_files", key),
        Item::WorkGroup { key, .. } => ("work_group", key),
        Item::ForkOrigin { key, .. } => ("fork_origin", key),
    };
    let mut fields = vec![
        ("type", Some(text(kind))),
        ("key", Some(text(key))),
        ("startMs", ms(rows.start_ms(item))),
        ("endMs", ms(rows.end_ms(item))),
    ];
    if let Item::WorkGroup {
        children,
        duration_ms,
        is_streaming,
        started_at_ms,
        ..
    } = item
    {
        let children = children.iter().map(|child| shown(child, rows));
        fields.extend([
            ("durationMs", ms(*duration_ms)),
            ("isStreaming", Some(Js::Bool(*is_streaming))),
            ("startedAtMs", ms(*started_at_ms)),
            ("children", Some(Js::Arr(children.collect()))),
        ]);
    }
    object(fields)
}

fn same(left: &Js, right: &Js, what: &str) {
    assert_eq!(js::stringify(left, ""), js::stringify(right, ""), "{what}");
}

fn both(run: impl Fn(bool) -> Js) -> Js {
    Js::Arr(vec![run(false), run(true)])
}

fn id(value: Option<&Js>) -> Js {
    value.cloned().unwrap_or(Js::Null)
}

#[test]
fn groups_match_cindys() {
    let (render, cases, expected) = (render(), cases(), expected());
    let render = render.obj().unwrap()["buildRenderItems"].arr().unwrap();
    let own = cases.obj().unwrap()["rows"].arr().unwrap();
    let all = expected.obj().unwrap()["groups"].arr().unwrap();
    assert!(all.len() > render.len() / 2 + own.len());
    for case in all {
        let [reference, groups, finals, users] = case.arr().unwrap() else {
            panic!("case shape")
        };
        let (rows, opts) = match reference {
            Js::Num(index) => {
                let args = render[*index as usize].arr().unwrap()[0].arr().unwrap();
                (&args[0], args.get(3))
            }
            Js::Arr(index) => {
                let Js::Num(index) = index[0] else {
                    panic!("reference shape")
                };
                let args = own[index as usize].arr().unwrap();
                (&args[0], args.get(1))
            }
            _ => panic!("reference shape"),
        };
        let what = js::stringify(reference, "");
        let rows = &tests::rows(rows);
        let grouped = both(|streaming| {
            let items = build(rows, options(opts)).unwrap();
            let grouped = group(items, rows, streaming);
            Js::Arr(
                grouped
                    .iter()
                    .map(|item| shown(item, &Rows(rows)))
                    .collect(),
            )
        });
        same(&grouped, groups, &what);
        let scan = refs(rows);
        let ids = turn_final_assistant_client_ids(&scan);
        same(
            &Js::Arr(
                ids.iter()
                    .map(|id| id.as_deref().map_or(Js::Null, text))
                    .collect(),
            ),
            finals,
            &what,
        );
        let found = Js::Arr(vec![
            id(first_user_message_client_id(&scan, false)),
            id(first_user_message_client_id(&scan, true)),
            id(last_user_message_client_id(&scan)),
        ]);
        same(&found, users, &what);
    }
}

#[test]
fn client_ids_are_found_as_cindy_finds_them() {
    let (cases, expected) = (cases(), expected());
    let own = cases.obj().unwrap()["rows"].arr().unwrap();
    let all = expected.obj().unwrap()["ids"].arr().unwrap();
    assert_eq!(own.len(), all.len());
    for (index, (case, ids)) in own.iter().zip(all).enumerate() {
        let args = case.arr().unwrap();
        let rows = &tests::rows(&args[0]);
        let scan = refs(rows);
        let found = ids.arr().unwrap().iter().map(|entry| {
            let id = entry.arr().unwrap()[0].str().unwrap();
            let holds = both(|streaming| {
                let items = build(rows, options(args.get(1))).unwrap();
                let grouped = group(items, rows, streaming);
                let holds = grouped
                    .iter()
                    .map(|item| Js::Bool(contains_client_id(item, rows, id)));
                Js::Arr(holds.collect())
            });
            let follows = assistant_has_following_user_boundary(&scan, id);
            Js::Arr(vec![text(id), Js::Bool(follows), holds])
        });
        same(&Js::Arr(found.collect()), ids, &format!("case {index}"));
    }
}

#[test]
fn fork_origins_land_where_cindys_do() {
    let (cases, expected) = (cases(), expected());
    let own = cases.obj().unwrap()["forks"].arr().unwrap();
    let all = expected.obj().unwrap()["forks"].arr().unwrap();
    assert_eq!(own.len(), all.len());
    for (index, (case, keys)) in own.iter().zip(all).enumerate() {
        let args = case.arr().unwrap();
        let rows = &tests::rows(&args[0]);
        let field = |key: &str| args[2].obj().map(|fork| fork[key].str().unwrap());
        let fork = field("parentSessionId").map(|parent| ForkOrigin {
            parent_session_id: parent,
            forked_at_message_id: field("forkedAtMessageId").unwrap(),
            forked_session_created_at: field("forkedSessionCreatedAt").unwrap(),
        });
        let marked = both(|streaming| {
            let items = build(rows, options(args.get(1))).unwrap();
            let items = insert_fork_origin(group(items, rows, streaming), rows, fork.as_ref());
            let keys = items.iter().map(|item| match &**item {
                Item::ForkOrigin {
                    key,
                    parent_session_id,
                    forked_at_message_id,
                } => object(vec![
                    ("type", Some(text("fork_origin"))),
                    ("key", Some(text(key))),
                    ("parentSessionId", Some(text(parent_session_id))),
                    ("forkedAtMessageId", Some(text(forked_at_message_id))),
                ]),
                other => shown(other, &Rows(rows)).obj().unwrap()["key"].clone(),
            });
            Js::Arr(keys.collect())
        });
        same(&marked, keys, &format!("case {index}"));
    }
}

#[test]
fn message_times_match_cindys() {
    let (cases, expected) = (cases(), expected());
    let rows = cases.obj().unwrap()["messageTs"].arr().unwrap();
    let all = expected.obj().unwrap()["messageTs"].arr().unwrap();
    assert_eq!(rows.len(), all.len());
    for (row, expected) in rows.iter().zip(all) {
        same(
            &ms(created_ms(&tests::rows(&Js::Arr(vec![row.clone()]))[0])).unwrap(),
            expected,
            &js::stringify(row, ""),
        );
    }
}

#[test]
fn a_fork_origin_holds_no_message() {
    let marker = Item::ForkOrigin {
        key: "fork-origin-parent-m9".into(),
        parent_session_id: "parent".into(),
        forked_at_message_id: "m9".into(),
    };
    assert!(!contains_client_id(&marker, &[], "m9"));
}
