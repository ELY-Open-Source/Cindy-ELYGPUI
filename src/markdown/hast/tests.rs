use serde_json::{Map, Value, json};

use super::{Hast, Prop, Urls, render};

fn bare(node: &Hast) -> Value {
    match node {
        Hast::Text(value) => json!(value),
        Hast::Raw => panic!("html reached a renderer"),
        Hast::Element(element) => {
            let mut props = Map::new();
            for (name, value) in &element.props {
                let value = match value {
                    Prop::Text(text) => json!(text),
                    Prop::Flag(flag) => json!(flag),
                    Prop::Number(number) => json!(number),
                    Prop::Words(words) => json!(words),
                };
                assert!(
                    props.insert((*name).to_owned(), value).is_none(),
                    "{name} twice"
                );
            }
            let children: Vec<Value> = element.children.iter().map(bare).collect();
            json!({"tag": element.tag, "props": props, "children": children})
        }
    }
}

/// react-markdown's own run with Cindy's plugins and transforms, by `sync-golden.mjs`.
#[test]
fn the_elements_match_cindys() {
    let golden = include_str!("../../../tests/fixtures/markdown-hast/expected.json");
    let cases: Vec<(String, Value, Value, Value)> = serde_json::from_str(golden).unwrap();
    assert!(cases.len() > 6400);
    let mut wrong = 0;
    for (text, trusted, preview, allowlisted) in &cases {
        // A face that matches the trusted one is recorded as null.
        let faces = [
            (true, Urls::Trusted, trusted),
            (false, Urls::Preview, preview),
            (false, Urls::Allowlisted, allowlisted),
        ];
        for (privileged, urls, cindy) in faces {
            let cindy = if cindy.is_null() { trusted } else { cindy };
            let port = std::panic::catch_unwind(|| {
                Value::Array(render(text, privileged, urls).iter().map(bare).collect())
            });
            if port.as_ref().ok() != Some(cindy) {
                wrong += 1;
                eprintln!("{text:?} {urls:?}\n  cindy {cindy}\n  port  {port:?}");
            }
        }
    }
    assert_eq!(wrong, 0, "of {} sources", cases.len());
}
