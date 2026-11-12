use std::{cell::RefCell, rc::Rc, time::Duration};

use gpui::TestAppContext;

use super::{Fading, walk};
use crate::{
    markdown::hast::{Element, Hast, Prop},
    motion::ReducedMotion,
    session_view::{
        assistant_row::Live,
        user_row::tests::{at, open_roles},
    },
    streamed_chunks::Chunks,
    word_fade::{Fade, cache::States},
};

fn tagged<'a>(nodes: &'a [Hast], tag: &str, out: &mut Vec<&'a Element>) {
    for node in nodes {
        if let Hast::Element(element) = node {
            if element.tag == tag {
                out.push(element);
            }
            tagged(&element.children, tag, out);
        }
    }
}

/// The opacity of each `tag` element of the tree at `now`.
fn shown(tree: &[Hast], tag: &str, fade: &Rc<RefCell<Fade>>, now: f64) -> Vec<Option<f32>> {
    let mut fading = Fading::default();
    walk(tree, &fade.borrow(), now, &mut fading);
    let mut found = Vec::new();
    tagged(tree, tag, &mut found);
    found.into_iter().map(|node| fading.of(node)).collect()
}

fn near(found: Option<f32>, wanted: f32) -> bool {
    found.is_some_and(|found| (found - wanted).abs() < 0.001)
}

#[test]
fn a_word_comes_in_along_blinks_curve_and_settles_at_its_end() {
    let fade: Rc<RefCell<Fade>> = Rc::default();
    let tree = Chunks::default().render("one two", Some(&fade), &[], 1000.0);
    // The second word starts 16ms on; before its start a word is not seen.
    assert_eq!(shown(&tree, "span", &fade, 1000.0), [Some(0.0), Some(0.0)]);
    // `logs/review/9.6j3c-blink-marker.log`.
    for (ms, opacity) in [
        (15.0, 0.494_391),
        (30.0, 0.752_126),
        (75.0, 0.971_779),
        (120.0, 0.998_891),
    ] {
        let found = shown(&tree, "span", &fade, 1000.0 + ms);
        assert!(near(found[0], opacity), "{ms}: {found:?}");
        assert!(near(shown(&tree, "span", &fade, 1016.0 + ms)[1], opacity));
    }
    assert!(!fade.borrow().settled("wf-s0-0"));
    let found = shown(&tree, "span", &fade, 1150.0);
    assert!(found[0].is_none() && found[1].is_some());
    assert!(fade.borrow().settled("wf-s0-0") && !fade.borrow().settled("wf-s0-1"));
    assert_eq!(shown(&tree, "span", &fade, 1166.0), [None, None]);
    assert!(fade.borrow().settled("wf-s0-1"));
}

#[test]
fn a_marker_comes_in_with_its_items_first_word() {
    let fade: Rc<RefCell<Fade>> = Rc::default();
    let tree = Chunks::default().render("- one", Some(&fade), &[], 1000.0);
    let marker = shown(&tree, "li", &fade, 1030.0);
    assert!(near(marker[0], 0.752_126), "{marker:?}");
    assert_eq!(marker, shown(&tree, "span", &fade, 1030.0));
    assert_eq!(shown(&tree, "li", &fade, 1150.0), [None]);
    assert!(fade.borrow().settled("wf-s0-0"));
}

#[test]
fn an_empty_items_marker_comes_in_and_settles_no_key() {
    let fade: Rc<RefCell<Fade>> = Rc::default();
    let tree = Chunks::default().render("-", Some(&fade), &[], 1000.0);
    assert!(near(shown(&tree, "li", &fade, 1015.0)[0], 0.494_391));
    assert_eq!(shown(&tree, "li", &fade, 1150.0), [None]);
    assert_eq!(fade.borrow().snapshot()["settled"], serde_json::json!([]));
}

#[gpui::test]
fn a_streaming_row_fades_where_the_stored_one_stands(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let text = "one `two`\n\n- three\n- four\n";
    let cx = open_roles(
        cx,
        dir.path(),
        2400.0,
        &[("user", "hi"), ("assistant", text)],
    );
    let paths = ["0", "1-0-0t", "1-0-marker", "1-1-0t", "1-1-marker"];
    let places = |cx: &mut gpui::VisualTestContext| {
        let place = |path: &&str| at(cx, &format!("msg-c1-md-{path}"));
        paths.iter().map(place).collect::<Vec<_>>()
    };
    // Each frame asks for the next while something moves.
    let frame = |cx: &mut gpui::VisualTestContext, ms: u64| {
        cx.executor().advance_clock(Duration::from_millis(ms));
        let asked = cx.update(|window, cx| window.simulate_next_frame(cx)) > 0;
        cx.run_until_parked();
        asked
    };
    // The window's own opening transitions end first.
    while frame(cx, 100) {}
    let stored = places(cx);
    assert!(stored.iter().all(Option::is_some));
    cx.update(|_, cx| {
        cx.set_global(ReducedMotion(false));
        crate::stream_fade::install(cx);
        cx.set_global(Live);
        cx.refresh_windows();
    });
    cx.run_until_parked();
    assert_eq!(places(cx), stored);
    let state = cx.update(|_, cx| cx.global_mut::<States>().get(Some("s1\0c1")));
    let settled = || state.borrow().snapshot()["settled"].clone();
    assert_eq!(settled(), serde_json::json!([]));
    assert!(frame(cx, 100));
    assert_eq!(settled(), serde_json::json!([]));
    assert!(frame(cx, 60));
    assert_eq!(settled(), serde_json::json!(["wf-s0-0"]));
    assert!(frame(cx, 60));
    let all = serde_json::json!(["wf-s0-0", "wf-s0-1", "wf-s0-2", "wf-s0-3"]);
    assert_eq!(settled(), all);
    // The last frame drew every word whole: no frame is asked for.
    assert!(!frame(cx, 60));
    assert_eq!(places(cx), stored);
}

/// Golden `stream-fade`: Cindy's chunk in Blink, each step read clear of every animation's end.
#[test]
fn markers_and_words_live_as_blink_runs_them() {
    use serde_json::{Value, json};
    let golden = include_str!("../../../tests/fixtures/stream-fade/expected.json");
    let golden: Value = serde_json::from_str(golden).unwrap();
    let mut count = 0;
    for (nth, scenario) in golden["scenarios"].as_array().unwrap().iter().enumerate() {
        let fade: Rc<RefCell<Fade>> = Rc::default();
        let mut chunks = Chunks::default();
        let mut tree = Rc::new(Vec::new());
        for (at, step) in scenario[1].as_array().unwrap().iter().enumerate() {
            let now = step["now"].as_f64().unwrap();
            // The frames since the last step, then the render.
            chunks.advance(now);
            walk(&tree, &fade.borrow(), now, &mut Fading::default());
            if let Some(text) = step["text"].as_str() {
                tree = chunks.render(text, Some(&fade), &[], now);
            }
            let mut fading = Fading::default();
            walk(&tree, &fade.borrow(), now, &mut fading);
            let mut spans = Vec::new();
            tagged(&tree, "span", &mut spans);
            let spans: Vec<Value> = (spans.iter())
                .filter_map(|span| match span.prop("dataWfKey") {
                    Some(Prop::Text(key)) => Some(json!([key, fading.of(*span).is_some()])),
                    _ => None,
                })
                .collect();
            let items = chunks.markers();
            let items: Vec<Value> = (items.iter())
                .map(|(path, (shown, animation, delay))| {
                    let delay = delay.map(crate::cindy_json::js_value);
                    json!([path, shown, animation, delay])
                })
                .collect();
            let found = json!({
                "spans": spans,
                "items": items,
                "settled": fade.borrow().snapshot()["settled"],
            });
            for part in ["spans", "items", "settled"] {
                let text = &scenario[0];
                assert_eq!(
                    found[part], step[part],
                    "scenario {nth} step {at} {part} {text}"
                );
            }
            count += 1;
        }
    }
    assert!(count > 350);
}
