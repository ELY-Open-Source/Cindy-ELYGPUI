use serde_json::Value;

use super::activity;
use crate::{i18n::Translator, locale, rich_answer::js};

#[test]
fn activity_lines_match_cindys_in_every_locale() {
    let cases: Value =
        serde_json::from_str(include_str!("../../tests/fixtures/tool-use/cases.json")).unwrap();
    let golden: Value =
        serde_json::from_str(include_str!("../../tests/fixtures/tool-use/activity.json")).unwrap();
    let cases = cases["island"].as_array().unwrap();
    for entry in golden.as_array().unwrap() {
        let tag = entry[0].as_str().unwrap();
        let tag = locale::SUPPORTED
            .iter()
            .find(|t| **t == tag)
            .expect("a port locale");
        let t = Translator::new(tag).unwrap();
        for (case, want) in cases.iter().zip(entry[1].as_array().unwrap()) {
            let name = case[0].as_str().unwrap();
            let input = (!case[1].is_null()).then(|| js::parse(&case[1].to_string()).unwrap());
            let got = activity(&t, name, input.as_ref());
            assert_eq!(got, want.as_str().unwrap(), "{tag} {name} {}", case[1]);
        }
    }
}
