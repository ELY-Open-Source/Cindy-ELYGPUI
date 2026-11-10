use super::{Insertion, Source, Status, Todo, insertions, is_plan_user_boundary, latest_state};
use crate::{
    chat_message::merge::Message,
    generated_files::tests::{refs, rows},
    rich_answer::js::{self, Js, Obj},
};

fn text(value: &str) -> Js {
    Js::Str(value.to_owned())
}

fn object(fields: Vec<(&str, Option<Js>)>) -> Js {
    let kept = fields
        .into_iter()
        .filter_map(|(k, v)| Some((k.to_owned(), v?)));
    Js::Obj(Obj::from_iter(kept))
}

pub fn todos(list: &[Todo]) -> Js {
    let todo = |t: &Todo| {
        let status = match t.status {
            Status::Pending => "pending",
            Status::InProgress => "in_progress",
            Status::Completed => "completed",
        };
        object(vec![
            ("content", Some(text(&t.content))),
            ("status", Some(text(status))),
            ("activeForm", t.active_form.as_deref().map(text)),
        ])
    };
    Js::Arr(list.iter().map(todo).collect())
}

/// The insertion as `JSON.stringify` writes Cindy's.
fn insertion(i: &Insertion) -> Js {
    let source = match i.source {
        Source::Todo => "todo",
        Source::Codex => "codex",
        Source::Task => "task",
    };
    object(vec![
        ("key", Some(text(&i.key))),
        ("todos", Some(todos(&i.todos))),
        (
            "sourceClientIds",
            Some(Js::Arr(
                i.source_client_ids.iter().map(|id| text(id)).collect(),
            )),
        ),
        ("createdAt", i.created_at.as_deref().map(text)),
        ("updatedAtMs", i.updated_at_ms.map(Js::Num)),
        ("source", Some(text(source))),
        ("sealed", i.sealed.then_some(Js::Bool(true))),
        ("sealedAtMs", i.sealed_at_ms.map(Js::Num)),
        ("turnFailed", i.turn_failed.then_some(Js::Bool(true))),
    ])
}

/// `messages` and the options object's `keyPrefix` and `taskHistoryMayBeIncomplete`.
fn arguments(args: &Js) -> (Vec<Message>, &str, bool) {
    let args = args.arr().unwrap();
    let options = args.get(1).and_then(Js::obj);
    let prefix = options.and_then(|o| o.get("keyPrefix")).and_then(Js::str);
    let incomplete = options.and_then(|o| o.get("taskHistoryMayBeIncomplete"));
    (
        rows(&args[0]),
        prefix.unwrap_or("todo"),
        incomplete == Some(&Js::Bool(true)),
    )
}

fn golden(name: &str, run: impl Fn(&Js) -> Js) {
    let all = js::parse(include_str!(
        "../../tests/fixtures/plan-timeline/expected.json"
    ))
    .unwrap();
    let cases = all.obj().unwrap()[name].arr().unwrap();
    assert!(!cases.is_empty(), "{name}");
    for case in cases {
        let [args, expected] = case.arr().unwrap() else {
            panic!("case shape")
        };
        assert_eq!(
            js::stringify(&run(args), ""),
            js::stringify(expected, ""),
            "{name} {}",
            js::stringify(args, "")
        );
    }
}

/// A throw, as the golden records its message.
fn thrown(result: Result<Js, String>) -> Js {
    result.unwrap_or_else(|message| object(vec![("throws", Some(text(&message)))]))
}

#[test]
fn insertions_match_cindys() {
    golden("findMessageTodoInsertions", |args| {
        let (messages, prefix, _) = arguments(args);
        let entry =
            |(index, i): (usize, Insertion)| Js::Arr(vec![Js::Num(index as f64), insertion(&i)]);
        thrown(
            insertions(&refs(&messages), prefix)
                .map(|all| Js::Arr(all.into_iter().map(entry).collect())),
        )
    });
}

#[test]
fn the_latest_state_matches_cindys() {
    golden("getLatestMessageTodoState", |args| {
        let (messages, prefix, incomplete) = arguments(args);
        thrown(
            latest_state(&refs(&messages), prefix, incomplete).map(|state| {
                object(vec![
                    (
                        "insertion",
                        Some(state.insertion.as_ref().map_or(Js::Null, insertion)),
                    ),
                    ("hasPlanEvent", Some(Js::Bool(state.has_plan_event))),
                    ("isResolved", Some(Js::Bool(state.is_resolved))),
                    (
                        "latestPlanIndex",
                        Some(Js::Num(state.latest_plan_index as f64)),
                    ),
                    (
                        "latestInsertionIndex",
                        Some(Js::Num(state.latest_insertion_index as f64)),
                    ),
                ])
            }),
        )
    });
    golden("findLatestMessageTodoInsertion", |args| {
        let (messages, prefix, incomplete) = arguments(args);
        let state = latest_state(&refs(&messages), prefix, incomplete);
        thrown(state.map(|state| state.insertion.as_ref().map_or(Js::Null, insertion)))
    });
}

#[test]
fn plan_todos_match_cindys() {
    golden("extractPlanTodos", |args| {
        let args = args.arr().unwrap();
        let name = args[0].str().unwrap_or("");
        let input = args.get(1).filter(|_| args.len() > 1);
        let found = super::extract_plan_todos(name, input);
        thrown(found.map(|found| found.map_or(Js::Null, |list| todos(&list))))
    });
}

#[test]
fn user_boundaries_match_cindys() {
    golden("isPlanUserBoundary", |args| {
        Js::Bool(is_plan_user_boundary(&rows(args)[0]))
    });
}
