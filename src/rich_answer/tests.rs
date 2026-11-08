use super::{Part, split, text};

fn expected() -> serde_json::Value {
    let raw = include_str!("../../tests/fixtures/preview/expected.json");
    serde_json::from_str(raw).expect("golden JSON")
}

#[test]
fn writes_rich_answers_as_rich_answer_text_does() {
    let golden = expected();
    let cases = golden["rich"].as_array().expect("rich cases");
    assert!(cases.len() >= 20);
    for case in cases {
        let source = case[0].as_str().unwrap();
        assert_eq!(text(source), case[1].as_str().unwrap(), "{source:?}");
    }
}

#[test]
fn splits_only_top_level_cindy_ui_fences() {
    let nested = "````md\n```cindy-ui\nx\n```\n````\n";
    assert_eq!(split(nested), vec![Part::Markdown(nested)]);
    assert_eq!(
        split("a\n```cindy-ui\nops\n```\nb"),
        vec![
            Part::Markdown("a\n"),
            Part::Rich("ops\n"),
            Part::Markdown("b")
        ]
    );
    assert_eq!(split("```cindy-ui\nops"), vec![Part::Rich("ops")]);
    assert!(split("").is_empty());
}

#[test]
fn parses_nine_hundred_thousand_levels_as_v8_json_parse_does() {
    let depth = 900_000;
    let deep = format!("{}{}", "[".repeat(depth), "]".repeat(depth));
    let line =
        format!(r#"{{"seq":1,"op":"create","id":"d","kind":"text","props":"top","x":{deep}}}"#);
    let parsed = super::js::parse(&line).expect("V8 parses it");
    assert_eq!(super::js::stringify_len(&parsed), line.len());
    drop(parsed.clone());
    assert_eq!(text(&format!("```cindy-ui\n{line}\n```")), "top");
}
