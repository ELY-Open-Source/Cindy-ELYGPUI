use std::time::Duration;

use gpui::TestAppContext;

use super::{ended, tree};
use crate::{
    markdown::hast::{self, Hast, Prop},
    motion::ReducedMotion,
    preferences::Preferences,
    session_view::tests::{go, install, serve},
    word_fade::cache::States,
};

/// The mounted content of `s1`, with the fade's globals.
fn open(cx: &mut TestAppContext) -> tempfile::TempDir {
    let dir = install(cx);
    cx.update(|cx| {
        cx.set_global(ReducedMotion(false));
        crate::stream_fade::install(cx);
    });
    serve(cx);
    go("/cc-agent/s1", cx);
    // A stream starts long after the app did.
    wait(1000, cx);
    dir
}

/// The tree's text, and the fade keys of its spans.
fn read(tree: &[Hast]) -> (String, Vec<String>) {
    fn keys(nodes: &[Hast], out: &mut Vec<String>) {
        for node in nodes {
            let Hast::Element(element) = node else {
                continue;
            };
            if let Some(Prop::Text(key)) = element.prop("dataWfKey") {
                out.push(key.clone());
            }
            keys(&element.children, out);
        }
    }
    let mut text = String::new();
    tree.iter().for_each(|node| hast::string(node, &mut text));
    let mut found = Vec::new();
    keys(tree, &mut found);
    (text, found)
}

fn shown(content: &str, cx: &mut TestAppContext) -> Option<(String, Vec<String>)> {
    cx.update(|cx| tree("m1", content, Some("c1"), &[], cx).map(|tree| read(&tree)))
}

fn wait(ms: u64, cx: &mut TestAppContext) {
    cx.executor().advance_clock(Duration::from_millis(ms));
    cx.run_until_parked();
}

#[gpui::test]
fn a_stream_shows_its_first_text_then_one_text_per_interval(cx: &mut TestAppContext) {
    let _dir = open(cx);
    let (text, keys) = shown("one", cx).unwrap();
    assert_eq!((text.as_str(), keys.len()), ("one", 1));
    wait(150, cx);
    // The interval has passed: the next text shows at once.
    assert_eq!(shown("one two", cx).unwrap().0, "one two");
    wait(30, cx);
    assert_eq!(shown("one two three", cx).unwrap().0, "one two");
    wait(60, cx);
    assert_eq!(shown("one two three", cx).unwrap().0, "one two");
    // The timer fires 100ms after the last emit, with the latest text.
    wait(10, cx);
    let (text, keys) = shown("one two three", cx).unwrap();
    assert_eq!(text, "one two three");
    assert_eq!(keys, ["wf-s0-0", "wf-s0-1", "wf-s0-2"]);
}

#[gpui::test]
fn a_message_back_in_view_keeps_its_keys_until_it_ends(cx: &mut TestAppContext) {
    let _dir = open(cx);
    shown("one two", cx).unwrap();
    go("/cc-agent/new", cx);
    go("/cc-agent/s1", cx);
    wait(200, cx);
    let (_, keys) = shown("one two three", cx).unwrap();
    assert_eq!(keys, ["wf-s0-0", "wf-s0-1", "wf-s0-2"]);
    cx.update(|cx| ended("m1", Some("c1"), cx));
    assert!(cx.update(|cx| cx.global::<States>().is_empty()));
    let (_, keys) = shown("one two three", cx).unwrap();
    assert_eq!(keys, ["wf-s0-0", "wf-s0-1", "wf-s0-2"]);
    assert!(!cx.update(|cx| cx.global::<States>().is_empty()));
}

#[gpui::test]
fn the_fade_is_left_out_under_reduced_motion_or_when_switched_off(cx: &mut TestAppContext) {
    let _dir = open(cx);
    cx.update(|cx| cx.set_global(ReducedMotion(true)));
    let (text, keys) = shown("one **tw", cx).unwrap();
    // With no fade the text is not repaired.
    assert_eq!((text.as_str(), keys.len()), ("one **tw", 0));
    cx.update(|cx| cx.set_global(ReducedMotion(false)));
    let (text, keys) = shown("one **tw", cx).unwrap();
    assert_eq!((text.as_str(), keys.len()), ("one tw", 2));
    let off = "chat.streamFadePreference";
    cx.update(|cx| cx.global_mut::<Preferences>().set(off, "off").unwrap());
    cx.update(crate::stream_fade::install);
    assert_eq!(shown("one **tw", cx).unwrap().1.len(), 0);
}

#[gpui::test]
fn a_rich_answer_draws_nothing_and_unmounts_the_renderer(cx: &mut TestAppContext) {
    let _dir = open(cx);
    let rich = "one\n\n```cindy-ui\n{}\n```\n";
    assert!(shown("one", cx).is_some());
    wait(30, cx);
    // The throttle that looks for a rich answer still shows the first text.
    assert!(shown(rich, cx).is_some());
    wait(70, cx);
    assert!(shown(rich, cx).is_none());
    wait(150, cx);
    // The renderer mounts anew: its throttle starts from the text it mounts with.
    assert_eq!(shown("one two", cx).unwrap().0, "one two");
}
