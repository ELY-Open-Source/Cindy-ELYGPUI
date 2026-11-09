use serde_json::Value;

use super::normalize;

#[test]
fn launchers_unwrap_as_cindys_do() {
    let golden: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/command-intent/expected.json"
    ))
    .unwrap();
    for case in golden["display"].as_array().unwrap() {
        let command = case[0].as_str().unwrap();
        assert_eq!(
            normalize(command).as_deref(),
            case[1].as_str(),
            "{command:?}"
        );
    }
}
