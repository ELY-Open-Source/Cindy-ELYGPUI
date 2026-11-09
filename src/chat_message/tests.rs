//! Golden `chat-message`: Cindy's `mapServerMessages` on `cases.json`.

use super::map_server_messages;
use crate::rich_answer::js::{self, Js};

#[test]
fn every_case_matches_cindy() {
    let expected = js::parse(include_str!(
        "../../tests/fixtures/chat-message/expected.json"
    ))
    .unwrap();
    let mut failures = Vec::new();
    for case in expected.arr().unwrap() {
        let case = case.obj().unwrap();
        let name = case["name"].str().unwrap();
        let got = match map_server_messages(case["rows"].arr().unwrap()) {
            Ok(rows) => {
                let json = Js::Arr(rows.iter().map(super::Out::json).collect());
                let undefined = Js::Arr(rows.iter().map(super::Out::undefined_keys).collect());
                format!(
                    "{}\n{}",
                    js::stringify(&json, " "),
                    js::stringify(&undefined, "")
                )
            }
            Err(error) => format!("error: {error}"),
        };
        let want = match case.get("error") {
            Some(error) => format!("error: {}", error.str().unwrap()),
            None => format!(
                "{}\n{}",
                js::stringify(&case["result"], " "),
                js::stringify(&case["undefinedKeys"], "")
            ),
        };
        if got != want {
            failures.push(format!("{name}\n got: {got}\nwant: {want}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
