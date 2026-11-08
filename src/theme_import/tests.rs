use std::{fs, path::Path};

use serde_json::{Value, json};

use super::color::{
    Rgb, contrast_ratio, is_dark_background, mix, parse_css_color, shade, to_hex, to_hsl_triplet,
    to_rgba_string,
};
use super::{convert_vscode_theme, vscode::strip_json_comments};
use crate::theme::Kind;

fn rgb(r: f64, g: f64, b: f64) -> Rgb {
    Rgb { r, g, b }
}

/// Cindy's own converter wrote `expected/` from `input/`.
#[test]
fn matches_cindy_converter_output() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/theme-import");
    let mut inputs: Vec<_> = fs::read_dir(dir.join("input"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    inputs.sort();
    assert_eq!(inputs.len(), 6);
    for input in inputs {
        let stem = input.file_stem().unwrap().to_str().unwrap();
        let raw = fs::read_to_string(&input).unwrap();
        let expected: Value = serde_json::from_str(
            &fs::read_to_string(dir.join(format!("expected/{stem}.json"))).unwrap(),
        )
        .unwrap();
        let actual = match convert_vscode_theme(&raw, stem).unwrap() {
            None => Value::Null,
            Some(result) => json!({
                "themes": result.themes.iter().map(|t| json!({
                    "name": t.name,
                    "type": if t.kind == Kind::Dark { "dark" } else { "light" },
                    "colors": t.colors.iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
                "report": {
                    "source": "vscode",
                    "resolvedRoles": result.report.resolved_roles,
                    "derivedRoles": result.report.derived_roles,
                    "unresolved": result.report.unresolved,
                    "skippedProtected": result.report.skipped_protected,
                },
            }),
        };
        assert_eq!(actual, expected, "{stem}");
    }
}

#[test]
fn parses_cindy_color_cases() {
    let cases = [
        ("#282c34", Some(rgb(40.0, 44.0, 52.0))),
        ("282c34", Some(rgb(40.0, 44.0, 52.0))),
        ("#333", Some(rgb(51.0, 51.0, 51.0))),
        ("#30363d80", Some(rgb(48.0, 54.0, 61.0))),
        ("#333a", Some(rgb(51.0, 51.0, 51.0))),
        ("rgba(97, 175, 239, 0.5)", Some(rgb(97.0, 175.0, 239.0))),
        ("rgb(50%, 50%, 50%)", Some(rgb(128.0, 128.0, 128.0))),
        ("hsl(120 100% 50%)", Some(rgb(0.0, 255.0, 0.0))),
        ("color-mix(in srgb, #000 12%, transparent)", None),
        ("var(--undefined-var)", None),
        ("transparent", None),
        ("currentColor", None),
        ("   ", None),
        ("rgb(1\u{a0}2\u{a0}3)", Some(rgb(1.0, 2.0, 3.0))),
        ("hsl(120\u{a0}100%\u{2003}50%)", Some(rgb(0.0, 255.0, 0.0))),
        ("\u{feff}#fff\u{3000}", Some(rgb(255.0, 255.0, 255.0))),
        ("rgb(1\u{85}2\u{85}3)", None),
    ];
    for (input, expected) in cases {
        assert_eq!(parse_css_color(input), expected, "{input}");
    }
}

#[test]
fn formats_and_derivations_match_cindy() {
    for (hex, triplet) in [
        ("#282c34", "220 13% 18%"),
        ("#abb2bf", "219 14% 71%"),
        ("#000000", "0 0% 0%"),
    ] {
        assert_eq!(to_hsl_triplet(parse_css_color(hex).unwrap()), triplet);
    }
    assert_eq!(to_hex(rgb(40.0, 44.0, 52.0)), "#282c34");
    assert_eq!(
        to_rgba_string(rgb(97.0, 175.0, 239.0), 0.1),
        "rgba(97, 175, 239, 0.1)"
    );
    assert_eq!(shade(rgb(100.0, 100.0, 100.0), -1.0), rgb(0.0, 0.0, 0.0));
    assert_eq!(
        mix(rgb(0.0, 0.0, 0.0), rgb(100.0, 200.0, 50.0), 0.5),
        rgb(50.0, 100.0, 25.0)
    );
    assert!(is_dark_background(rgb(40.0, 44.0, 52.0)));
    assert!(!is_dark_background(rgb(253.0, 246.0, 227.0)));
    assert!((contrast_ratio(rgb(0.0, 0.0, 0.0), rgb(255.0, 255.0, 255.0)) - 21.0).abs() < 0.05);
}

#[test]
fn strips_jsonc_like_cindy() {
    let cases = [
        ("{\"a\": \"x, }y\"}", json!({"a": "x, }y"})),
        (
            "{\"a\": \"https://example.com/x\", \"b\": \"/* no */\"}",
            json!({"a": "https://example.com/x", "b": "/* no */"}),
        ),
        (
            "{\"a\": \"say \\\" // still\", \"b\": 1}",
            json!({"a": "say \" // still", "b": 1}),
        ),
        (
            "{\n \"a\": 1, // note\n \"b\": [1, 2,], /* block */\n}",
            json!({"a": 1, "b": [1, 2]}),
        ),
    ];
    for (input, expected) in cases {
        let parsed: Value = serde_json::from_str(&strip_json_comments(input)).unwrap();
        assert_eq!(parsed, expected, "{input}");
    }
    assert!(convert_vscode_theme("{\"foo\":1}", "x").unwrap().is_none());
    assert!(convert_vscode_theme("not json", "x").unwrap().is_none());
}
