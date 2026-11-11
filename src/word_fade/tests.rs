//! Golden `word-fade`: Cindy's `rehypeStreamWordFade.ts` in Cindy's Electron (`scripts/word-fade.js`).

use std::rc::Rc;

use serde_json::{Map, Value, json};

use super::{
    Fade,
    cache::{States, held, split_words},
    fade,
};
use crate::markdown::hast::{Element, Hast, Prop};

fn leaked(name: &str) -> &'static str {
    Box::leak(name.to_owned().into_boxed_str())
}

fn read(node: &Value) -> Hast {
    if let Some(text) = node.as_str() {
        return Hast::Text(text.to_owned());
    }
    let prop = |(name, value): (&String, &Value)| {
        let value = match value {
            Value::String(text) => Prop::Text(text.clone()),
            Value::Bool(flag) => Prop::Flag(*flag),
            Value::Array(words) => Prop::Words(serde_json::from_value(json!(words)).unwrap()),
            other => panic!("a property of {other}"),
        };
        (leaked(name), value)
    };
    Hast::Element(Element {
        tag: leaked(node["tag"].as_str().unwrap()),
        props: node["props"]
            .as_object()
            .unwrap()
            .iter()
            .map(prop)
            .collect(),
        children: node["children"]
            .as_array()
            .unwrap()
            .iter()
            .map(read)
            .collect(),
    })
}

fn bare(node: &Hast) -> Value {
    let Hast::Element(element) = node else {
        let Hast::Text(text) = node else {
            panic!("html in a renderer's tree");
        };
        return json!(text);
    };
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

#[test]
fn streams_fade_as_cindys() {
    let golden = include_str!("../../tests/fixtures/word-fade/expected.json");
    let golden: Value = serde_json::from_str(golden).unwrap();
    let scenarios = golden["scenarios"].as_array().unwrap();
    let (mut runs, mut schedules) = (0, 0);
    for (nth, steps) in scenarios.iter().enumerate() {
        let mut owner = Fade::default();
        for (at, step) in steps.as_array().unwrap().iter().enumerate() {
            let source = || step["source"].as_u64().unwrap() as usize;
            let now = || step["now"].as_f64().unwrap();
            match step["op"].as_str().unwrap() {
                "run" => {
                    let whole = step["whole"].as_bool().unwrap();
                    let mut candidate = match (owner.source(source()).candidate(), whole) {
                        (_, true) => owner.whole_candidate(source()),
                        (candidate, false) => candidate,
                    };
                    let tree = step["tree"].as_array().unwrap();
                    let mut tree: Vec<Hast> = tree.iter().map(read).collect();
                    fade(&mut tree, &mut candidate, now());
                    let out: Vec<Value> = tree.iter().map(bare).collect();
                    assert_eq!(json!(out), step["out"], "scenario {nth} step {at}");
                    if step["commit"].as_bool().unwrap() {
                        owner.source(source()).commit(&candidate);
                        if whole {
                            owner.retain_only(source());
                        }
                    }
                    assert_eq!(owner.snapshot(), step["state"], "scenario {nth} step {at}");
                    runs += 1;
                }
                "schedule" => {
                    let key = step["key"].as_str().unwrap();
                    let delay = owner.source(source()).schedule(key, now());
                    assert_eq!(
                        Some(delay),
                        step["delay"].as_f64(),
                        "scenario {nth} step {at}"
                    );
                    schedules += 1;
                }
                "settle" => owner.settle(step["key"].as_str().unwrap()),
                "end" => assert_eq!(owner.snapshot(), step["state"], "scenario {nth}"),
                other => panic!("a step of {other}"),
            }
        }
    }
    let kept = &scenarios.last().unwrap()[1]["out"][0]["children"][0]["props"];
    assert_eq!(kept["dataWfKey"], "wf-s0-0");
    assert!(runs > 600 && schedules > 2600);
}

#[test]
fn a_settled_key_reads_settled_from_every_chunk() {
    let mut owner = Fade::default();
    owner.source(40).settle("wf-s40-0");
    assert!(owner.source(0).settled("wf-s40-0"));
    assert!(!owner.settled("wf-s0-0"));
}

#[test]
fn the_sixty_fifth_message_drops_the_one_asked_for_longest_ago() {
    let mut states = States::default();
    let first = states.get(Some("m0"));
    let second = states.get(Some("m1"));
    assert!(Rc::ptr_eq(&first, &states.get(Some("m0"))));
    for nth in 2..=64 {
        states.get(Some(&format!("m{nth}")));
    }
    assert!(Rc::ptr_eq(&first, &states.get(Some("m0"))));
    assert!(!Rc::ptr_eq(&second, &states.get(Some("m1"))));
    states.release(Some("m0"));
    assert!(!Rc::ptr_eq(&first, &states.get(Some("m0"))));
    assert!(!Rc::ptr_eq(&states.get(None), &states.get(None)));
    assert!(!Rc::ptr_eq(&states.get(Some("")), &states.get(Some(""))));
}

#[test]
fn split_words_keeps_five_hundred_texts_and_two_megabytes() {
    assert_eq!(*split_words("a b"), ["a ", "b"]);
    assert_eq!(held(), (vec!["a b".to_owned()], 6 + 12 + 10));
    for nth in 0..499 {
        split_words(&format!("w{nth}"));
    }
    split_words("a b");
    split_words("one more");
    let (texts, _) = held();
    assert_eq!(texts.len(), 500);
    assert_eq!(texts[0], "w1");
    assert_eq!(texts[498], "a b");
    let long = "x".repeat(600_000);
    split_words(&long);
    assert!(!held().0.contains(&long));
    let half = "y".repeat(250_000);
    split_words(&half);
    split_words(&"z".repeat(250_000));
    let (texts, bytes) = held();
    assert!(texts.contains(&half) && bytes <= 2 * 1024 * 1024);
    split_words(&"v".repeat(250_000));
    let (texts, bytes) = held();
    assert!(!texts.contains(&half) && bytes <= 2 * 1024 * 1024);
}
