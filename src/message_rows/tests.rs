use super::*;

static NULL: Js = Js::Null;

/// The golden read as `JSON.parse` reads it, so objects keep their key order.
pub fn golden() -> Js {
    js::parse(include_str!(
        "../../tests/fixtures/history-rows/expected.json"
    ))
    .unwrap()
}

/// Each golden case of `name`: its first argument and the expected result.
pub fn cases<'a>(golden: &'a Js, name: &str) -> Vec<(&'a Js, &'a Js)> {
    let list = golden.obj().unwrap()[name].arr().unwrap();
    assert!(!list.is_empty(), "{name} has cases");
    list.iter()
        .map(|c| (&c.arr().unwrap()[0].arr().unwrap()[0], &c.arr().unwrap()[1]))
        .collect()
}

pub fn field<'a>(value: &'a Js, key: &str) -> Option<&'a Js> {
    value.obj().and_then(|o| o.get(key))
}

pub fn shown(text: &str) -> String {
    js::stringify(&js::parse(text).unwrap(), "")
}

/// Fields a live `ChatMessage` carries and a stored row never does.
fn live_only(message: &Js) -> bool {
    ["delivery", "systemCardType", "isContinuationTrigger"]
        .iter()
        .any(|key| field(message, key).is_some())
}

#[test]
fn text_checks_match_cindys() {
    let golden = golden();
    for (name, check) in [
        (
            "isDeliveryProseText",
            is_delivery_prose_text as fn(&str) -> bool,
        ),
        ("isOrcaCommunicationTool", is_orca_communication_tool),
        ("isAgentTaskToolName", is_agent_task_tool_name),
        ("isAgentPlanToolName", is_agent_plan_tool_name),
        ("hasRichAnswer", rich_answer::has),
    ] {
        for (input, expected) in cases(&golden, name) {
            let text = input.str().unwrap();
            assert_eq!(&Js::Bool(check(text)), expected, "{name} {text:?}");
        }
    }
}

#[test]
fn previews_match_cindys() {
    let golden = golden();
    for (input, expected) in cases(&golden, "messageContentToPreview") {
        assert_eq!(
            expected.str(),
            Some(content_to_preview(input).as_str()),
            "{input:?}"
        );
    }
}

#[test]
fn tool_uses_match_cindys() {
    let golden = golden();
    for (message, expected) in cases(&golden, "parseMessageToolUse") {
        let content = field(message, "content").unwrap_or(&NULL);
        let parsed = tool_use(field(message, "toolUseId").and_then(Js::str), content);
        let mut shown = Obj::new();
        if let Some(id) = parsed.tool_use_id {
            shown.insert("toolUseId".into(), Js::Str(id));
        }
        shown.insert("toolName".into(), Js::Str(parsed.tool_name));
        shown.insert("input".into(), parsed.input);
        assert_eq!(&Js::Obj(shown), expected, "{message:?}");
    }
}

#[test]
fn row_flags_match_cindys() {
    let golden = golden();
    let mut checked = 0;
    for (message, expected) in cases(&golden, "isContinuationMessage") {
        if live_only(message) {
            continue;
        }
        let role = field(message, "role").and_then(Js::str).unwrap_or("");
        let content = field(message, "content").unwrap_or(&NULL);
        let got = is_continuation_message(role, content, field(message, "agentMeta"));
        assert_eq!(&Js::Bool(got), expected, "{message:?}");
        checked += 1;
    }
    for (message, expected) in cases(&golden, "isSteerUserRow") {
        if live_only(message) {
            continue;
        }
        let got = is_steer_user_row(field(message, "agentMeta"));
        assert_eq!(&Js::Bool(got), expected, "{message:?}");
        checked += 1;
    }
    assert!(checked > 40, "stored-row cases ran: {checked}");
}

#[test]
fn visible_results_match_cindys() {
    let golden = golden();
    for (input, expected) in cases(&golden, "hasVisibleHistoryResult") {
        assert_eq!(
            &Js::Bool(has_visible_history_result(input)),
            expected,
            "{input:?}"
        );
    }
}

#[test]
fn stringify_writes_as_json_stringify() {
    let value = js::parse(r#"{"b":[1,{},[]],"2":"x","1":null,"n":-0}"#).unwrap();
    assert_eq!(
        js::stringify(&value, "  "),
        "{\n  \"1\": null,\n  \"2\": \"x\",\n  \"b\": [\n    1,\n    {},\n    []\n  ],\n  \"n\": 0\n}"
    );
    assert_eq!(
        shown(r#"{"b":[1,{},[]],"2":"x","1":null}"#),
        r#"{"1":null,"2":"x","b":[1,{},[]]}"#
    );
}
