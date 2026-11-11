use gpui::{Modifiers, TestAppContext, VisualTestContext, px};

use super::title;
use crate::{
    appearance_settings::CodeSize,
    chat_message::merge::Fields,
    i18n::Translator,
    rich_answer::js,
    session_view::{
        links::tests::{click, focus, key},
        user_row::tests::{at, open, settle},
    },
    user_message::Orca,
};

const LEAD: &str = r#"{"orcaSource":"lead","content":"do it"}"#;
const WORKER: &str = r#"{"origin":{"kind":"orca","senderSessionId":"s2","senderLabel":"Reviewer"},"sourceDevice":{"deviceId":"d1","platform":"mobile"},"goalObjective":{"updated":true}}"#;

fn orca(source: &str, content: &str) -> String {
    let content = serde_json::to_string(content).unwrap();
    format!(r#"{{"orcaSource":"{source}","content":{content}}}"#)
}

fn height(cx: &mut VisualTestContext, selector: &str) -> f32 {
    at(cx, selector).unwrap().size.height.into()
}

/// How many `x` one row holds at `width`, in the code face at `size`.
fn per_row(cx: &mut VisualTestContext, size: f32, width: f32) -> usize {
    use ely_gpui_component::theme::ActiveTheme as _;
    cx.update(|window, cx| {
        let mut style = window.text_style();
        style.font_family = cx.theme().mono_family.clone();
        let run = style.to_run(10);
        let system = window.text_system();
        let line = system.shape_line("xxxxxxxxxx".into(), px(size), &[run], None);
        (width / (f32::from(line.width) / 10.0)).floor() as usize
    })
}

#[test]
fn the_title_names_the_lead_or_the_worker() {
    let t = Translator::new("en").unwrap();
    let read = |source: &str, origin: &str| {
        let mut message = Fields::default();
        if !origin.is_empty() {
            let origin = js::parse(origin).unwrap();
            message.insert("automationOrigin".into(), Some(origin.into()));
        }
        let orca = Orca {
            source: source.into(),
            content: String::new(),
        };
        title(&message, &orca, &t)
    };
    let named = r#"{"kind":"session","orcaSenderLabel":"Reviewer"}"#;
    assert_eq!(read("lead", ""), "Message from Lead");
    assert_eq!(read("lead", named), "Message from Lead");
    assert_eq!(read("worker", ""), "Message from Worker");
    assert_eq!(read("worker", named), "Message from Worker \"Reviewer\"");
    for origin in [
        r#"{"kind":"session","orcaSenderLabel":""}"#,
        r#"{"kind":"session"}"#,
        r#"{"kind":"scheduler","orcaSenderLabel":"Reviewer"}"#,
    ] {
        assert_eq!(read("worker", origin), "Message from Worker", "{origin}");
    }
}

#[gpui::test]
fn the_card_sits_left_and_unfolds_on_a_click(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), 2400.0, &[(LEAD, None), ("plain", None)]);
    let column = at(cx, "session-messages").unwrap();
    let card = at(cx, "msg-c0-orca").unwrap();
    assert_eq!(card.left(), column.left(), "justify-start");
    assert_eq!(card.top(), column.top() + px(28.0));
    assert_eq!(card.size.width, px(560.0), "w-full max-w-[560px]");
    let head = at(cx, "msg-c0-orca-head").unwrap();
    assert_eq!(head.size.width, px(558.0), "w-full inside the border");
    assert_eq!(head.size.height, px(30.0), "py-2 about the 14px icons");
    assert_eq!(card.size.height, px(32.0));
    assert!(at(cx, "msg-c0-orca-body").is_none(), "folded at first");
    for part in ["bubble", "sources"] {
        assert!(at(cx, &format!("msg-c0-{part}")).is_none(), "{part}");
    }
    let title = at(cx, "msg-c0-orca-title").unwrap();
    assert_eq!(
        title.left(),
        head.left() + px(34.0),
        "px-3, the icon, gap-2"
    );
    assert_eq!(title.right(), head.right() - px(34.0), "flex-1");
    assert_eq!(title.size.height, px(13.0), "text-13 leading-none");
    let next = at(cx, "item-msg-c1").unwrap();
    assert_eq!(next.top(), card.bottom() + px(14.0));
    click(cx, "msg-c0-orca-head", Modifiers::none());
    let body = at(cx, "msg-c0-orca-body").unwrap();
    let head = at(cx, "msg-c0-orca-head").unwrap();
    assert_eq!(body.top(), head.bottom());
    assert_eq!(body.size.width, px(558.0));
    let text = at(cx, "msg-c0-orca-text").unwrap();
    assert_eq!(text.left(), body.left() + px(12.0), "px-3");
    assert_eq!(text.top(), body.top() + px(9.0), "border-t and py-2");
    assert_eq!(text.size.height, px(18.0), "the code size and 4px");
    assert_eq!(body.size.height, px(35.0));
    assert_eq!(height(cx, "msg-c0-orca"), 67.0);
    assert!(at(cx, "msg-c1-orca").is_none());
    click(cx, "msg-c0-orca-head", Modifiers::none());
    assert!(at(cx, "msg-c0-orca-body").is_none());
}

#[gpui::test]
fn the_keyboard_unfolds_the_card(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), 2400.0, &[(LEAD, None), (LEAD, None)]);
    focus(cx, "msg-c0-orca-head");
    assert!(
        at(cx, "msg-c0-orca-body").is_none(),
        "a press alone is no click"
    );
    cx.update(|window, cx| window.focus_next(cx));
    key(cx, "enter");
    assert!(
        at(cx, "msg-c1-orca-body").is_some(),
        "the header is a Tab stop"
    );
    assert!(at(cx, "msg-c0-orca-body").is_none());
    key(cx, "space");
    assert!(at(cx, "msg-c1-orca-body").is_none());
}

#[gpui::test]
fn the_source_labels_stand_left_over_the_card(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let rows = [(orca("worker", "done"), Some(WORKER))];
    let rows: Vec<(&str, Option<&str>)> = rows.iter().map(|(c, m)| (c.as_str(), *m)).collect();
    let cx = open(cx, dir.path(), 2400.0, &rows);
    let column = at(cx, "session-messages").unwrap();
    let labels = at(cx, "msg-c0-sources").unwrap();
    assert_eq!(labels.left(), column.left(), "align start");
    assert_eq!(labels.top(), column.top() + px(28.0));
    let sender = at(cx, "msg-c0-source-session").unwrap();
    let device = at(cx, "msg-c0-source-device").unwrap();
    assert_eq!(sender.left(), labels.left());
    assert_eq!(device.left(), sender.right() + px(12.0));
    let card = at(cx, "msg-c0-orca").unwrap();
    assert_eq!(card.top(), labels.bottom() + px(8.0), "gap-2");
    assert_eq!(card.left(), column.left());
    assert!(
        at(cx, "msg-c0-goal").is_none(),
        "the goal badge is the bubble's"
    );
}

#[gpui::test]
fn the_body_is_the_message_in_pre_wrap(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let long = "word ".repeat(120);
    let rows = [
        orca("lead", "a\r\nb\n"),
        orca("lead", ""),
        orca("lead", "\n"),
        orca("lead", "a\n\nb"),
        orca("lead", &long),
        orca("lead", &"x".repeat(400)),
    ];
    let rows: Vec<(&str, Option<&str>)> = rows.iter().map(|row| (row.as_str(), None)).collect();
    let cx = open(cx, dir.path(), 2400.0, &rows);
    for n in 0..rows.len() {
        click(cx, &format!("msg-c{n}-orca-head"), Modifiers::none());
    }
    assert_eq!(height(cx, "msg-c0-orca-text"), 36.0, "the last feed ends");
    assert_eq!(height(cx, "msg-c1-orca-text"), 0.0, "an empty pre");
    assert_eq!(height(cx, "msg-c1-orca-body"), 17.0);
    assert_eq!(height(cx, "msg-c2-orca-text"), 18.0);
    assert_eq!(height(cx, "msg-c3-orca-text"), 54.0, "an empty line stands");
    let wrapped = height(cx, "msg-c4-orca-text");
    assert!(wrapped > 18.0, "words wrap at the card's width");
    let text = at(cx, "msg-c4-orca-text").unwrap();
    let body = at(cx, "msg-c4-orca-body").unwrap();
    assert!(text.right() <= body.right() - px(12.0));
    let broken = at(cx, "msg-c5-orca-text").unwrap();
    assert!(broken.size.height > px(18.0), "break-words");
    assert!(broken.right() <= body.right() - px(12.0));
}

#[gpui::test]
fn the_body_takes_the_code_size(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let long = orca("lead", &"x".repeat(400));
    let cx = open(cx, dir.path(), 2400.0, &[(long.as_str(), None)]);
    click(cx, "msg-c0-orca-head", Modifiers::none());
    let small = height(cx, "msg-c0-orca-text");
    assert_eq!(small % 18.0, 0.0);
    cx.update(|_, cx| cx.set_global(CodeSize(20.0)));
    settle(cx);
    let large = height(cx, "msg-c0-orca-text");
    assert_eq!(large % 24.0, 0.0, "the line is the code size and 4px");
    assert!(large / 24.0 > small / 18.0, "the glyphs grow with it");
}

#[gpui::test]
fn the_text_breaks_at_the_body_width(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path(), 2400.0, &[("plain", None)]);
    // 560 less the border and `px-3`, at the code size less 2px.
    let per = per_row(cx, 12.0, 534.0);
    assert!(per_row(cx, 12.0, 560.0) > per && per_row(cx, 14.0, 534.0) < per);
    let dir = tempfile::tempdir().unwrap();
    let rows = [
        orca("lead", &"x".repeat(per * 3)),
        orca("lead", &"x".repeat(per * 3 + 1)),
        // A carriage return draws nothing.
        orca("lead", &format!("{}\r{}", "x".repeat(per - 1), "x")),
    ];
    let rows: Vec<(&str, Option<&str>)> = rows.iter().map(|row| (row.as_str(), None)).collect();
    let mut other = TestAppContext::single();
    let cx = open(&mut other, dir.path(), 2400.0, &rows);
    for n in 0..rows.len() {
        click(cx, &format!("msg-c{n}-orca-head"), Modifiers::none());
    }
    assert_eq!(height(cx, "msg-c0-orca-text"), 54.0);
    assert_eq!(height(cx, "msg-c1-orca-text"), 72.0);
    assert_eq!(height(cx, "msg-c2-orca-text"), 18.0);
}

#[gpui::test]
fn a_narrow_column_shrinks_the_card_and_its_text(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let meta = r#"{"origin":{"kind":"orca","senderSessionId":"s2","senderLabel":"A\nB"}}"#;
    let cx = open(cx, dir.path(), 700.0, &[(LEAD, None)]);
    let wide = at(cx, "msg-c0-orca").unwrap().size.width;
    assert!(wide < px(560.0));
    // The compact rail: the code size less 3px.
    let per = per_row(cx, 11.0, f32::from(wide) - 26.0);
    assert!(per_row(cx, 12.0, f32::from(wide) - 26.0) < per);
    let dir = tempfile::tempdir().unwrap();
    let text = orca("worker", &"x".repeat(per * 2 + 1));
    let mut other = TestAppContext::single();
    let cx = open(
        &mut other,
        dir.path(),
        700.0,
        &[(text.as_str(), Some(meta))],
    );
    click(cx, "msg-c0-orca-head", Modifiers::none());
    let column = at(cx, "session-messages").unwrap();
    let card = at(cx, "msg-c0-orca").unwrap();
    assert_eq!(card.size.width, column.size.width);
    assert_eq!(height(cx, "msg-c0-orca-text"), 54.0);
    let head = at(cx, "msg-c0-orca-head").unwrap();
    assert_eq!(head.size.height, px(30.0), "the icons hold the height");
    // `text-13` less the rail's pixel, on one line whatever the role holds.
    assert_eq!(height(cx, "msg-c0-orca-title"), 12.0);
}

#[gpui::test]
fn a_forwarded_request_shows_its_words(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let meta = r#"{"origin":{"kind":"orca","senderSessionId":"s2"}}"#;
    let envelope =
        "[Source] A task\nUser\u{2019}s original words:\"say this\"\nAgent supplement:\"\"";
    let sent = orca("worker", envelope);
    // A channel's message keeps the card and shows the words the channel saved.
    let hook = r#"{"hookSource":{"im":"feishu","userText":"said"}}"#;
    let rows = [
        (sent.as_str(), Some(meta)),
        (sent.as_str(), None),
        (sent.as_str(), Some(hook)),
    ];
    let cx = open(cx, dir.path(), 2400.0, &rows);
    for n in 0..3 {
        click(cx, &format!("msg-c{n}-orca-head"), Modifiers::none());
    }
    // The envelope's three lines read as eight.
    assert_eq!(height(cx, "msg-c0-orca-text"), 144.0);
    assert_eq!(
        height(cx, "msg-c1-orca-text"),
        54.0,
        "any other stays verbatim"
    );
    assert_eq!(height(cx, "msg-c2-orca-text"), 18.0, "the channel's words");
}

#[gpui::test]
fn labels_that_wrap_start_each_row_at_the_left(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let name = "A device with a name of some length";
    let meta = format!(
        r#"{{"origin":{{"kind":"orca","senderSessionId":"s2","senderSessionTitle":"A stored title of some length to fill the row"}},"sourceDevice":{{"deviceId":"d1","platform":"mobile","name":"{name}"}}}}"#
    );
    let cx = open(cx, dir.path(), 700.0, &[(LEAD, Some(meta.as_str()))]);
    let labels = at(cx, "msg-c0-sources").unwrap();
    let sender = at(cx, "msg-c0-source-session").unwrap();
    let device = at(cx, "msg-c0-source-device").unwrap();
    assert!(device.top() > sender.top(), "the device wraps");
    assert_eq!(sender.left(), labels.left());
    assert_eq!(device.left(), labels.left(), "justify-start");
    let short = sender.right().min(device.right());
    assert!(short < labels.right(), "one row has room to spare");
}

#[gpui::test]
fn a_pre_of_ignored_characters_holds_no_line(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    // Blink's heights for the same `pre`, from Cindy's Electron.
    let cases = [
        ("\r", 0.0),
        ("\r\r", 0.0),
        ("\x0c", 0.0),
        ("\r\x0c", 0.0),
        ("\r\n", 18.0),
        ("a\r", 18.0),
        ("a\n\x0c", 18.0),
        ("\n\x0c", 18.0),
        ("\x0c\n\x0c", 18.0),
        ("a\n\x0c\r", 18.0),
        ("a\x0cb", 18.0),
        ("a\n\x0b", 36.0),
    ];
    let rows = cases.map(|(text, _)| orca("lead", text));
    let rows: Vec<(&str, Option<&str>)> = rows.iter().map(|row| (row.as_str(), None)).collect();
    let cx = open(cx, dir.path(), 2400.0, &rows);
    for (n, (text, tall)) in cases.iter().enumerate() {
        click(cx, &format!("msg-c{n}-orca-head"), Modifiers::none());
        assert_eq!(
            height(cx, &format!("msg-c{n}-orca-text")),
            *tall,
            "{text:?}"
        );
        // Folded again, so the next card stays in view.
        click(cx, &format!("msg-c{n}-orca-head"), Modifiers::none());
    }
}
