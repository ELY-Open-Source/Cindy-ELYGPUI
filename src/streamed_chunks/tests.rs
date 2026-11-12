//! Golden `streamed-chunks`: Cindy's `StreamingMarkdownChunk` rows in jsdom, in Cindy's Electron.

use std::{cell::RefCell, rc::Rc};

use serde_json::{Value, json};

use super::{Chunks, replaced};
use crate::{
    markdown::hast::{self, Hast, Prop},
    word_fade::Fade,
};

/// Each faded span's key and text, as the DOM holds them: a replaced link's words are not there.
fn spans(nodes: &[Hast], out: &mut Vec<Value>) {
    for node in nodes {
        let Hast::Element(element) = node else {
            continue;
        };
        if element.tag == "a" && replaced(element, &[]) {
            continue;
        }
        if let ("span", Some(Prop::Text(key))) = (element.tag, element.prop("dataWfKey")) {
            let mut text = String::new();
            hast::string(node, &mut text);
            out.push(json!([key, text]));
        }
        spans(&element.children, out);
    }
}

#[test]
fn streams_mount_the_spans_cindys_rows_mount() {
    let golden = include_str!("../../tests/fixtures/streamed-chunks/expected.json");
    let golden: Value = serde_json::from_str(golden).unwrap();
    let mut count = 0;
    for (nth, stream) in golden["streams"].as_array().unwrap().iter().enumerate() {
        let mut now = stream[0].as_f64().unwrap();
        let fade: Rc<RefCell<Fade>> = Rc::default();
        let mut chunks = Chunks::default();
        let text = stream[1].as_str().unwrap();
        for (at, step) in stream[2].as_array().unwrap().iter().enumerate() {
            now += step[0].as_f64().unwrap();
            let shown = text.char_indices().nth(step[1].as_u64().unwrap() as usize);
            let shown = &text[..shown.map_or(text.len(), |(end, _)| end)];
            let tree = chunks.render(shown, Some(&fade), &[], now);
            let mut mounted = Vec::new();
            spans(&tree, &mut mounted);
            assert_eq!(json!(mounted), step[2], "stream {nth} step {at}");
            let state = fade.borrow().snapshot();
            for part in ["start", "settled", "next"] {
                assert_eq!(state[part], step[3][part], "stream {nth} step {at} {part}");
            }
            count += 1;
        }
    }
    assert!(count > 1200);
}

#[test]
fn a_stream_with_no_fade_is_parsed_as_it_stands() {
    let mut chunks = Chunks::default();
    let tree = chunks.render("one **tw\n\n- three", None, &[], 0.0);
    let whole = hast::render("one **tw\n\n", true, hast::Urls::Trusted);
    assert_eq!(tree[..whole.len()], whole[..]);
    assert!(Rc::ptr_eq(
        &tree,
        &chunks.render("one **tw\n\n- three", None, &[], 9.0)
    ));
}

#[test]
fn a_chips_words_take_no_place_and_an_item_follows_its_words() {
    let fade: Rc<RefCell<Fade>> = Rc::default();
    let text = "- [go task](cindy://session/task-id) after";
    Chunks::default().render(text, Some(&fade), &[], 1000.0);
    let state = fade.borrow().snapshot();
    // The item's marker borrows the chip's first word, which no span mounted.
    assert_eq!(
        state["start"],
        json!([["wf-s0-0", 1016], ["wf-s0-2", 1000]])
    );
    assert_eq!(state["next"], json!(1032));
}
