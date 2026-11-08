use super::{html_image_alt, preview};

#[test]
fn projects_markdown_as_markdown_preview_text_does() {
    let raw = include_str!("../../tests/fixtures/preview/expected.json");
    let golden: serde_json::Value = serde_json::from_str(raw).expect("golden JSON");
    let cases = golden["markdown"].as_array().expect("markdown cases");
    assert!(cases.len() >= 20);
    for case in cases {
        let source = case[0].as_str().unwrap();
        assert_eq!(preview(source), case[1].as_str().unwrap(), "{source:?}");
    }
}

#[test]
fn reads_an_img_tag_as_html_img_to_image_node_does() {
    let alt = |html: &str| html_image_alt(html);
    assert_eq!(
        alt("<img src=\"https://a\" alt=\"x &lt;y&gt;\">").as_deref(),
        Some("x <y>")
    );
    assert_eq!(alt("<img src=\"xdt-image://a\">").as_deref(), Some(""));
    assert_eq!(alt("<img src=\"file:///a\" alt=\"no\">"), None);
    assert_eq!(alt("<img alt=\"no src\">"), None);
    assert_eq!(alt("<imgx src=\"https://a\">"), None);
    assert_eq!(
        alt("<img\u{e9} src=\"https://a\" alt=\"word edge\">").as_deref(),
        Some("word edge")
    );
}
