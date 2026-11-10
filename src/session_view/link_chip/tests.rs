use gpui::{TestAppContext, px};

use super::super::user_row::tests::{at, open, settle};
use super::{Found, Want, compact, display_text, folder, project, session};
use crate::{i18n::Translator, rich_answer::js};

/// The session chip's label and what it still wants, by the widths the test font gives.
fn asked(href: &str, label: Option<&str>, found: &Found) -> (bool, Vec<Want>) {
    let t = Translator::new("en").unwrap();
    let mut wants = Vec::new();
    let chip = session(href, label, (found, &t), &mut wants);
    (chip.is_some(), wants)
}

#[test]
fn a_session_link_asks_once_for_what_it_lacks() {
    let mut found = Found::default();
    let title = || vec![Want::Title("s".into())];
    assert_eq!(asked("cindy://session/s", None, &found), (true, title()));
    // White space is no label; the author's own words need no read.
    assert_eq!(
        asked("cindy://session/s", Some(" \n"), &found),
        (true, title())
    );
    assert_eq!(
        asked("cindy://session/s", Some("mine"), &found),
        (true, vec![])
    );
    // Another device's task has no row here.
    assert_eq!(
        asked("cindy://session/s?device=d", None, &found),
        (true, vec![])
    );
    let text = vec![Want::Text("s".into(), "m".into())];
    assert_eq!(
        asked("xdt-maker://session/s?message=m", None, &found),
        (true, text)
    );
    assert_eq!(
        asked("cindy://session/s?message=m", Some("x"), &found),
        (true, vec![])
    );
    assert_eq!(asked("cindy://session/", None, &found), (false, vec![]));
    // An asked read, answered or not, is not asked again.
    found.titles.insert("s".into(), None);
    found
        .texts
        .insert(("s".into(), "m".into()), Some("hi".into()));
    assert_eq!(asked("cindy://session/s", None, &found), (true, vec![]));
    assert_eq!(
        asked("cindy://session/s?message=m", None, &found),
        (true, vec![])
    );
}

#[test]
fn a_session_link_says_what_is_known_of_its_task() {
    let t = Translator::new("en").unwrap();
    let mut found = Found::default();
    found.titles.insert("s".into(), Some("Plan".into()));
    found.titles.insert("new".into(), Some("New Maker".into()));
    let text = ("s".to_owned(), "m".to_owned());
    found.texts.insert(text, Some(" the\nwords ".into()));
    let says = |href: &str, label: Option<&str>| {
        let chip = session(href, label, (&found, &t), &mut Vec::new()).unwrap();
        chip.label().to_owned()
    };
    assert_eq!(says("cindy://session/s", None), "Plan");
    assert_eq!(says("cindy://session/s", Some(" mine ")), "mine");
    // The draft title is the unnamed label; another device's task is not read here.
    assert_eq!(says("cindy://session/new", None), "Untitled session");
    assert_eq!(says("cindy://session/s?device=d", None), "A session");
    assert_eq!(says("cindy://session/none", None), "A session");
    assert_eq!(says("cindy://session/s?message=m", None), "the words");
    assert_eq!(says("cindy://session/s?message=x", None), "Quoted message");
    assert_eq!(says("cindy://session/s?message=m", Some("mine")), "mine");
}

#[test]
fn a_project_link_names_its_folder() {
    let says = |label| project("cindy://project/%2Fa%2Fwork", label).unwrap();
    assert_eq!(says(None).label(), "work");
    assert_eq!(says(Some(" ")).label(), "work");
    assert_eq!(says(Some("Mine")).label(), "Mine");
    assert_eq!(folder("/a/b/"), "b");
    assert_eq!(folder("C:\\a\\b\\\\"), "b");
    assert_eq!(folder("b"), "b");
    assert_eq!(folder("//"), "//");
    assert!(project("cindy://project/%2Fa%2Fb", None).is_some());
    assert!(project("cindy://project/", Some("x")).is_none());
}

#[test]
fn a_message_label_is_one_line_of_240_units() {
    assert_eq!(compact(" a\n\u{3000}b "), "a b");
    let long = format!("{}\u{1f600}", "a".repeat(239));
    let cut = compact(&long);
    assert_eq!(cut.encode_utf16().count(), 240);
    assert!(cut.ends_with('\u{2026}'));
    assert_eq!(compact(&"a".repeat(240)), "a".repeat(240));
}

#[test]
fn a_message_tells_its_words_and_no_payload() {
    let text = |row: &str| display_text(&js::parse(row).unwrap());
    let said = |words: &str| Some(words.to_owned());
    assert_eq!(text(r#"{"role":"user","content":" hi \n"}"#), said("hi"));
    let quoted = r#"{"role":"user","content":{"text":"> <!-- cindy-composer-quote -->\n> q\n\nyes","quotesEncoded":true}}"#;
    assert_eq!(text(quoted), said("> q\n\nyes"));
    let files = r#"{"role":"user","content":{"text":" ","images":[{"url":"cindy-media://a","mimeType":"image/png","originalName":"a.png"}],"files":[{"name":"b.txt","path":"/b.txt"}]}}"#;
    assert_eq!(text(files), said("a.png \u{b7} b.txt"));
    assert_eq!(text(r#"{"role":"user","content":""}"#), None);
    assert_eq!(text(r#"{"role":"assistant","content":" ok "}"#), said("ok"));
    let blocks = r#"{"role":"assistant","content":[{"type":"text","text":"a"},{"type":"tool_use","text":"x"},{"type":"text","text":"b "}]}"#;
    assert_eq!(text(blocks), said("ab"));
    assert_eq!(text(r#"{"role":"assistant","content":{"text":"x"}}"#), None);
    assert_eq!(text(r#"{"role":"tool","content":"x"}"#), None);
}

/// A chip's width for `letters` of 7.2 pixels and an icon, as the row snaps it.
fn wide(cx: &mut gpui::VisualTestContext, chip: &str, letters: f32) {
    let width = f32::from(at(cx, chip).unwrap().size.width);
    let over = width - (letters * 7.2 + 38.0);
    assert!((0.0..1.0).contains(&over), "{chip}: {width}");
}

#[gpui::test]
fn link_chips_read_their_task_and_message(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let body = "cindy://session/old cindy://session/none [mine](cindy://session/old) \
        cindy://project/%2Fhome%2Fme%2Fwork%2F";
    let quoted = "cindy://session/s1?message=c2";
    let rows = [(body, None), (quoted, None), ("the words", None)];
    let cx = open(cx, dir.path(), 2400.0, &rows);
    settle(cx);
    // The task's title, `A session` for none, the author's label, the folder, the message.
    wide(cx, "msg-c0-chip-0", 8.0);
    wide(cx, "msg-c0-chip-1", 9.0);
    wide(cx, "msg-c0-chip-2", 4.0);
    wide(cx, "msg-c0-chip-3", 4.0);
    wide(cx, "msg-c1-chip-0", 9.0);
    // The message link's quote is not lifted, so its row grows a pixel more.
    assert_eq!(at(cx, "msg-c0-text").unwrap().size.height, px(30.0));
    assert_eq!(at(cx, "msg-c1-text").unwrap().size.height, px(31.0));
}

#[gpui::test]
fn an_image_path_breaks_anywhere_in_its_letters(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    // 50 letters a row: the path fills the first row's rest where a word would start the next.
    let path = format!("/tmp/{}.png", "a".repeat(85));
    let word = "b".repeat(94);
    let (path, word) = (format!("see {path}"), format!("see {word}"));
    let cx = open(cx, dir.path(), 2400.0, &[(&path, None), (&word, None)]);
    let rows = |cx: &mut gpui::VisualTestContext, key: &str| {
        f32::from(at(cx, &format!("{key}-text")).unwrap().size.height) / 24.0
    };
    assert_eq!((rows(cx, "msg-c0"), rows(cx, "msg-c1")), (2.0, 3.0));
}
