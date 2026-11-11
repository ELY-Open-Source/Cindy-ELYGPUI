use std::rc::Rc;

use indexmap::{IndexMap, IndexSet};

use super::{Input, render};
use crate::{
    chat_message::{
        merge::Message,
        transport::{items, map_rows},
    },
    generated_files::tests::{object, text},
    history_view::{
        self,
        controller::{Detail, Snapshot},
    },
    render_items::{Deferred, Item},
    rich_answer::js::{self, Js},
};

fn strings(list: Option<&Js>) -> IndexSet<String> {
    let list = list.and_then(Js::arr).unwrap_or_default();
    list.iter().map(|id| id.str().unwrap().to_owned()).collect()
}

/// A source row under another id, as a detail read may store it.
fn stored(row: &Js, id: &Js) -> Js {
    let mut row = row.obj().unwrap().clone();
    row.insert("id".to_owned(), id.clone());
    Js::Obj(row)
}

fn details(list: &Js, source: &[Js]) -> IndexMap<String, Detail<Message>> {
    let detail = |entry: &Js| {
        let [key, state] = entry.arr().unwrap() else {
            panic!("detail shape")
        };
        let state = state.obj().unwrap();
        let picked = state["rows"].arr().unwrap().iter().map(|pick| {
            let [Js::Num(index), id] = pick.arr().unwrap() else {
                panic!("pick shape")
            };
            stored(&source[*index as usize], id)
        });
        let detail = Detail {
            messages: map_rows(&picked.collect::<Vec<_>>()).unwrap(),
            revision: state["revision"].str().unwrap().to_owned(),
            last_message_id: state["lastMessageId"].str().unwrap().to_owned(),
            loading: state["loading"] == Js::Bool(true),
            complete: state["complete"] == Js::Bool(true),
            error: state["error"].str().map(str::to_owned),
        };
        (key.str().unwrap().to_owned(), detail)
    };
    list.arr().unwrap().iter().map(detail).collect()
}

/// A store row with fields set over it.
fn live_row(spec: &Js) -> Message {
    let spec = spec.obj().unwrap();
    let mut row = (*map_rows(std::slice::from_ref(&spec["row"]))
        .unwrap()
        .remove(0))
    .clone();
    for (key, value) in spec["set"].obj().unwrap() {
        row.insert(key.clone(), Some(Rc::new(value.clone())));
    }
    Rc::new(row)
}

fn keys(keys: Vec<String>) -> Js {
    Js::Arr(keys.iter().map(|key| text(key)).collect())
}

fn deferred(work: &Deferred, expanded: &IndexSet<String>) -> Js {
    let visible = [(true, false), (true, true), (false, true), (false, false)]
        .map(|(full, preview)| keys(work.visible(full, preview)));
    let toggle = work
        .summaries
        .iter()
        .map(|s| Js::Arr(vec![text(&s.key), Js::Bool(!work.expanded)]));
    let retry = work.retried(expanded).into_iter().map(|s| s.key.clone());
    object(vec![
        ("key", Some(text(&work.key))),
        ("expanded", Some(Js::Bool(work.expanded))),
        ("revision", Some(text(&work.revision))),
        ("previewComplete", Some(Js::Bool(work.preview_complete))),
        ("lines", Some(Js::Bool(false))),
        ("livePreview", Some(Js::Bool(work.live_preview))),
        ("awaitingDetails", Some(Js::Bool(work.awaiting_details))),
        ("loading", Some(Js::Bool(work.loading))),
        ("failed", Some(Js::Bool(work.failed))),
        ("visible", Some(Js::Arr(visible.to_vec()))),
        ("toggle", Some(Js::Arr(toggle.collect()))),
        ("retry", Some(keys(retry.collect()))),
    ])
}

fn shown(item: &Item, rows: &[Message], expanded: &IndexSet<String>) -> Js {
    let id = |row: &usize| crate::render_items::field(&rows[*row], "clientId").cloned();
    let ms = |value: &Option<f64>| Some(value.map_or(Js::Null, Js::Num));
    let mut fields = vec![
        ("type", Some(text(item.kind()))),
        ("key", Some(text(item.key()))),
    ];
    match item {
        Item::Message { message, .. } => {
            let row = rows[*message].iter();
            let kept = row.filter_map(|(k, v)| Some((k.clone(), (**v.as_ref()?).clone())));
            fields.push(("message", Some(Js::Obj(kept.collect()))));
        }
        Item::ToolSegment { tool_calls, .. } => {
            let calls = tool_calls.iter().map(|call| id(call).unwrap_or(Js::Null));
            fields.push(("calls", Some(Js::Arr(calls.collect()))));
        }
        Item::AgentTask { tool_call, .. } => fields.push(("call", id(tool_call))),
        Item::WorkGroup {
            children,
            duration_ms,
            is_streaming,
            started_at_ms,
            deferred: work,
            ..
        } => {
            let children = children.iter().map(|child| shown(child, rows, expanded));
            let work = work.as_ref().map_or(Js::Null, |w| deferred(w, expanded));
            fields.extend([
                ("durationMs", ms(duration_ms)),
                ("isStreaming", Some(Js::Bool(*is_streaming))),
                ("startedAtMs", ms(started_at_ms)),
                ("deferred", Some(work)),
                ("children", Some(Js::Arr(children.collect()))),
            ]);
        }
        _ => {}
    }
    object(fields)
}

#[test]
fn the_view_renders_as_cindys() {
    let cases = js::parse(include_str!(
        "../../tests/fixtures/history-render/cases.json"
    ));
    let expected = js::parse(include_str!(
        "../../tests/fixtures/history-render/expected.json"
    ));
    let (cases, expected) = (cases.unwrap(), expected.unwrap());
    let sets = cases.obj().unwrap()["rows"].arr().unwrap();
    let (cases, expected) = (
        cases.obj().unwrap()["cases"].arr().unwrap(),
        expected.arr().unwrap(),
    );
    assert_eq!(cases.len(), expected.len());
    for (index, (case, expected)) in cases.iter().zip(expected).enumerate() {
        let case = case.obj().unwrap();
        let [resolved, want] = expected.arr().unwrap() else {
            panic!("case shape")
        };
        let resolved = resolved.obj().unwrap();
        let flag = |key: &str| case.get(key) == Some(&Js::Bool(true));
        let Js::Num(set) = case["rows"] else {
            panic!("rows shape")
        };
        let source = sets[set as usize].arr().unwrap();
        let page = history_view::project(source, flag("streaming"), true);
        let snapshot = Snapshot {
            items: items(page).unwrap(),
            details: details(&resolved["details"], source),
            expanded: strings(resolved.get("expanded")),
            ready: true,
            ..Snapshot::default()
        };
        let live = case.get("live").and_then(Js::arr).unwrap_or_default();
        let live: Vec<Message> = live.iter().map(live_row).collect();
        let input = Input {
            snapshot: &snapshot,
            live: &live,
            streaming: flag("sessionStreaming"),
            pending: &strings(case.get("pending")),
            tail: &strings(case.get("tail")),
            working_dir: case.get("workingDir").and_then(Js::str).unwrap_or_default(),
            image_targets: None,
        };
        let got = match render(&input) {
            Ok(projection) => {
                let shown = |item: &Rc<Item>| shown(item, &projection.rows, &snapshot.expanded);
                Js::Arr(projection.items.iter().map(shown).collect())
            }
            Err(message) => object(vec![("throws", Some(text(&message)))]),
        };
        assert_eq!(
            js::stringify(&got, ""),
            js::stringify(want, ""),
            "case {index}"
        );
    }
}
