use serde_json::Value;

use super::{TrustLabels, project};
use crate::{
    i18n::Translator,
    rich_answer::js::{self, Js},
};

fn parsed(value: &Value) -> Js {
    js::parse(&value.to_string()).unwrap()
}

fn labels() -> TrustLabels {
    TrustLabels::new(&Translator::new("en").unwrap())
}

#[test]
fn rows_project_as_project_mcp_app_content_does() {
    let raw = |s: &str| serde_json::from_str::<Value>(s).unwrap();
    let cases = raw(include_str!("../../../../tests/fixtures/search/cases.json"));
    let expected = raw(include_str!(
        "../../../../tests/fixtures/search/expected.json"
    ));
    let (cases, expected) = (
        cases["mcpApp"].as_array().unwrap(),
        expected["mcpApp"].as_array().unwrap(),
    );
    assert_eq!(cases.len(), expected.len());
    for (case, want) in cases.iter().zip(expected) {
        let got = project(parsed(&case[0]), case[1].as_bool().unwrap(), &labels());
        assert_eq!(got, parsed(want), "{case}");
    }
}

fn card(trust: &str) -> Js {
    parsed(&serde_json::json!({ "mcpApp": {
        "version": 1, "serverId": "s", "serverRevision": "r", "resourceUri": "ui://x",
        "tool": { "name": "chart", "title": "Chart", "inputSchema": { "type": "object" },
                  "_meta": { "cindy/plugin": { "name": "Charts", "trust": trust } } },
        "arguments": {}, "result": { "content": [{ "type": "text", "text": "body" }] }
    }}))
}

fn title(content: Js) -> String {
    let projected = project(content, true, &labels());
    let history = projected.obj().unwrap()["mcpAppHistory"].obj().unwrap();
    history["title"].str().unwrap().to_owned()
}

#[test]
fn a_known_plugin_trust_names_the_plugin_in_the_title() {
    assert_eq!(title(card("reviewed")), "Charts · Reviewed");
    assert_eq!(title(card("cindy-official")), "Charts · Cindy official");
    assert_eq!(
        title(card("forged")),
        "Chart",
        "an unknown trust keeps the tool title"
    );
    assert_eq!(title(Js::Str("{broken".into())), "");
}
