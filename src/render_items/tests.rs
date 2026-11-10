use super::{Item, Options, build, turn_end::is_turn_sealed};
use crate::{
    generated_files::tests::{file, object, text},
    plan_todos::tests::todos,
    rich_answer::js::{self, Js},
    tool_media::tests::shown as media,
};

fn golden(name: &str, run: impl Fn(&[Js]) -> Js) {
    let all = js::parse(include_str!(
        "../../tests/fixtures/render-items/expected.json"
    ))
    .unwrap();
    let cases = all.obj().unwrap()[name].arr().unwrap();
    assert!(!cases.is_empty(), "{name}");
    for case in cases {
        let [args, expected] = case.arr().unwrap() else {
            panic!("case shape")
        };
        assert_eq!(
            js::stringify(&run(args.arr().unwrap()), ""),
            js::stringify(expected, ""),
            "{name} {}",
            js::stringify(args, "")
        );
    }
}

fn number(n: usize) -> Js {
    Js::Num(n as f64)
}

fn strings<'a>(all: impl IntoIterator<Item = &'a String>) -> Js {
    Js::Arr(all.into_iter().map(|s| text(s)).collect())
}

/// The item as the golden's projection writes Cindy's.
fn shown(item: &Item) -> Js {
    let pair = |key: &String, value: Js| Js::Arr(vec![text(key), value]);
    let ms = |ms: &Option<f64>| Some(ms.map_or(Js::Null, Js::Num));
    match item {
        Item::Message { key, message } => object(vec![
            ("type", Some(text("message"))),
            ("key", Some(text(key))),
            ("message", Some(number(*message))),
        ]),
        Item::AgentPlan {
            key,
            todos: list,
            source_client_ids,
            created_at,
        } => object(vec![
            ("type", Some(text("agent_plan"))),
            ("key", Some(text(key))),
            ("todos", Some(todos(list))),
            ("sourceClientIds", Some(strings(source_client_ids))),
            ("createdAt", created_at.as_deref().map(text)),
        ]),
        Item::ToolSegment {
            key,
            tool_calls,
            result_map,
            result_ts_map,
            settled_ids,
        } => {
            let results = result_map.iter().map(|(k, v)| pair(k, text(v)));
            let times = result_ts_map.iter().map(|(k, v)| pair(k, Js::Num(*v)));
            object(vec![
                ("type", Some(text("tool_segment"))),
                ("key", Some(text(key))),
                (
                    "toolCalls",
                    Some(Js::Arr(tool_calls.iter().map(|c| number(*c)).collect())),
                ),
                ("resultMap", Some(Js::Arr(results.collect()))),
                ("resultTsMap", Some(Js::Arr(times.collect()))),
                ("settledIds", Some(strings(settled_ids))),
            ])
        }
        Item::AgentTask {
            key,
            tool_call,
            persisted_status,
            result,
            result_ts_ms,
        } => object(vec![
            ("type", Some(text("agent_task"))),
            ("key", Some(text(key))),
            ("persistedStatus", persisted_status.as_deref().map(text)),
            ("result", result.as_deref().map(text)),
            ("resultTsMs", result_ts_ms.map(Js::Num)),
            ("toolCall", Some(number(*tool_call))),
        ]),
        Item::ToolMedia { key, items } => object(vec![
            ("type", Some(text("tool_media"))),
            ("key", Some(text(key))),
            ("items", Some(media(items))),
        ]),
        Item::GeneratedFiles {
            key,
            files,
            turn_start_ms,
            turn_end_ms,
            turn_sealed,
        } => object(vec![
            ("type", Some(text("generated_files"))),
            ("key", Some(text(key))),
            ("turnStartMs", ms(turn_start_ms)),
            ("turnEndMs", ms(turn_end_ms)),
            ("turnSealed", Some(Js::Bool(*turn_sealed))),
            ("files", Some(Js::Arr(files.iter().map(file).collect()))),
        ]),
    }
}

#[test]
fn render_items_match_cindys() {
    golden("buildRenderItems", |args| {
        let opts = args.get(3).and_then(Js::obj);
        let opt = |key: &str| opts.and_then(|o| o.get(key));
        let options = Options {
            history_window_incomplete: opt("historyWindowIncomplete") == Some(&Js::Bool(true)),
            working_dir: opt("workingDir").and_then(Js::str).unwrap_or_default(),
            history_artifacts: opt("historyArtifacts")
                .and_then(Js::arr)
                .unwrap_or_default(),
        };
        match build(args[0].arr().unwrap(), options) {
            Ok(items) => Js::Arr(items.iter().map(shown).collect()),
            Err(message) => object(vec![("throws", Some(text(&message)))]),
        }
    });
}

#[test]
fn sealed_turns_match_cindys() {
    golden("isGeneratedFilesTurnSealed", |args| {
        let slice: Vec<&Js> = args[0].arr().unwrap().iter().collect();
        Js::Bool(is_turn_sealed(&slice, args.get(1) == Some(&Js::Bool(true))))
    });
}
