//! The handoff pill: shown by a card's navigation, its return and its dismissal.

use gpui::{KeyDownEvent, Keystroke, Modifiers, TestAppContext, VisualTestContext, px};

use crate::{
    handoff_card,
    i18n::Translator,
    router::{self, Handoff},
    session_view::{
        links::tests::{click, focus, key},
        user_row::tests::{at, open_roles, settle},
    },
};

const CARD: &str = "[x](cindy://session-card/old?wake=resumed&title=Plan&last_active=null)";
const PILL: &str = "handoff-pill";

/// `s1` with a card to `old`, which is then clicked.
fn handed<'a>(cx: &'a mut TestAppContext, dir: &std::path::Path) -> &'a mut VisualTestContext {
    let cx = open_roles(cx, dir, 2400.0, &[("user", "hi"), ("assistant", CARD)]);
    assert!(at(cx, PILL).is_none());
    click(cx, "msg-c1-card-a0", Modifiers::none());
    cx
}

fn path(cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| router::path(cx).to_owned())
}

#[gpui::test]
fn a_card_s_click_hands_its_task_on_and_the_pill_returns(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = handed(cx, dir.path());
    assert_eq!(path(cx), "/cc-agent/old");
    let from = cx.update(|_, cx| router::handoff(cx).cloned()).unwrap();
    assert_eq!(from.dispatcher, "s1");
    // `shrink-0 px-4 pt-3` over the viewport, `h-10`, at most the message width.
    let (view, pill) = (at(cx, "session-view").unwrap(), at(cx, PILL).unwrap());
    assert_eq!(pill.top() - view.top(), px(12.0));
    assert_eq!(pill.size.height, px(40.0));
    assert_eq!(pill.center().x, view.center().x);
    assert!(pill.size.width <= view.size.width - px(32.0));
    let messages = at(cx, "session-messages").unwrap();
    assert_eq!(messages.top(), pill.bottom());
    click(cx, "handoff-pill-return", Modifiers::none());
    assert_eq!(path(cx), "/cc-agent/s1");
    assert!(cx.update(|_, cx| router::handoff(cx).is_none()));
    assert!(at(cx, PILL).is_none());
}

#[gpui::test]
fn dismiss_hides_the_pill_until_the_task_changes(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = handed(cx, dir.path());
    click(cx, "handoff-pill-dismiss", Modifiers::none());
    assert!(at(cx, PILL).is_none());
    assert_eq!(path(cx), "/cc-agent/old");
    // The state stays with the location; a new task's content shows its own pill.
    let from = Handoff {
        dispatcher: "old".into(),
        title: None,
    };
    cx.update(|_, cx| router::navigate_from("/cc-agent/s1", from, cx));
    settle(cx);
    assert!(at(cx, PILL).is_some());
}

#[gpui::test]
fn enter_returns_on_key_down_and_the_keyboard_draws_the_ring(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = handed(cx, dir.path());
    focus(cx, "handoff-pill-dismiss");
    assert!(at(cx, "handoff-pill-dismiss-ring").is_none());
    key(cx, "shift");
    assert!(at(cx, "handoff-pill-dismiss-ring").is_some());
    focus(cx, "handoff-pill-return");
    cx.simulate_event(KeyDownEvent {
        keystroke: Keystroke::parse("enter").unwrap(),
        is_held: false,
        prefer_character_input: false,
    });
    settle(cx);
    assert_eq!(path(cx), "/cc-agent/s1");
}

#[gpui::test]
fn the_line_s_white_space_collapses(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = handed(cx, dir.path());
    let wide = |cx: &mut VisualTestContext, title: &str| {
        let from = Handoff {
            dispatcher: "s1".into(),
            title: Some(title.into()),
        };
        cx.update(|_, cx| router::navigate_from("/cc-agent/old", from, cx));
        settle(cx);
        // The state keeps the title as the card handed it on.
        let kept = cx.update(|_, cx| router::handoff(cx).cloned()).unwrap();
        assert_eq!(kept.title.as_deref(), Some(title));
        at(cx, "handoff-pill-line").unwrap().size
    };
    let plain = wide(cx, "Review the plan");
    assert_eq!(wide(cx, "Review          the plan"), plain);
    assert_eq!(wide(cx, "Review\r\n\t\x0cthe plan"), plain);
}

#[gpui::test]
fn a_plain_navigation_carries_no_handoff(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = handed(cx, dir.path());
    // The same place without state is a new location, as React Router's is.
    cx.update(|_, cx| router::navigate("/cc-agent/old", cx));
    settle(cx);
    assert!(cx.update(|_, cx| router::handoff(cx).is_none()));
    assert!(at(cx, PILL).is_none());
}

#[test]
fn the_pill_names_the_dispatcher_by_title_or_short_id() {
    let t = Translator::new("en").unwrap();
    let line = |title| handoff_card::pill("83639512aaaaaaaa7ed0", title, &t);
    assert_eq!(
        line(Some(" Plan ")),
        "Came from dispatcher Plan · click to return"
    );
    let short = "Came from dispatcher 83639512…7ed0 · click to return";
    assert_eq!(line(Some("  ")), short);
    assert_eq!(line(None), short);
    assert_eq!(handoff_card::dismiss(&t), "Dismiss handoff return hint");
}
