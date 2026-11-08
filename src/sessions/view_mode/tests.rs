use gpui::{App, TestAppContext};

use super::{
    LEGACY_USAGE_KEYS, MAIN_KEY, Main, OWNER_CLAIM_KEY, PINNED_KEY, Pinned, VINTAGE_KEY,
    claim_captured_legacy_data, install, main, pinned, set_main, set_pinned, vintage,
};
use crate::preferences::Preferences;

fn prefs(entries: &[(&str, &str)]) -> (tempfile::TempDir, Preferences) {
    let dir = tempfile::tempdir().unwrap();
    let mut prefs = Preferences::load(dir.path()).unwrap();
    for (key, value) in entries {
        prefs.set(key, value).unwrap();
    }
    (dir, prefs)
}

fn vintage_of(entries: &[(&str, &str)]) -> &'static str {
    vintage(&mut prefs(entries).1)
}

#[test]
fn an_empty_store_is_a_fresh_install() {
    assert_eq!(vintage_of(&[]), "fresh");
}

#[test]
fn any_legacy_usage_key_is_a_legacy_install() {
    for key in LEGACY_USAGE_KEYS {
        assert_eq!(vintage_of(&[(key, "x")]), "legacy", "{key}");
    }
    assert_eq!(
        vintage_of(&[("cc-agent.sidebar.collapsedProjects.owner.owner-1", "[]")]),
        "legacy"
    );
    assert_eq!(
        vintage_of(&[("cc-agent.sidebar.collapsedProjectsSomethingElse", "x")]),
        "fresh"
    );
}

#[test]
fn a_claim_counts_only_when_it_captured_old_data() {
    let empty = r#"{"version":1,"ownerId":"owner-1","legacy":{"schemaVersion":1,"values":{"cc-agent.sidebar.filter.projects":null,"cc-agent.sidebar.pinnedSessionOrder":null}}}"#;
    let captured = r#"{"version":1,"ownerId":"owner-1","legacy":{"schemaVersion":1,"values":{"cc-agent.sidebar.filter.projects":"[\"/repo\"]"}}}"#;
    assert!(!claim_captured_legacy_data(empty));
    assert!(claim_captured_legacy_data(captured));
    assert!(claim_captured_legacy_data(
        r#"{"version":1,"ownerId":"owner-1"}"#
    ));
    for malformed in ["owner-1", "{oops", "42", r#"{"version":1}"#] {
        assert!(!claim_captured_legacy_data(malformed), "{malformed}");
    }
    assert!(claim_captured_legacy_data(
        r#"{"version":1.0,"ownerId":"o","legacy":{"values":[null,1e400]}}"#
    ));
    assert!(!claim_captured_legacy_data(
        r#"{"version":1,"ownerId":"o","legacy":null}"#
    ));
    let deep = format!(
        r#"{{"version":1,"ownerId":"o","deep":{}0{},"legacy":{{"values":{{"k":"v"}}}}}}"#,
        "[".repeat(200),
        "]".repeat(200)
    );
    assert!(
        claim_captured_legacy_data(&deep),
        "JSON.parse has no depth limit"
    );
    assert_eq!(vintage_of(&[(OWNER_CLAIM_KEY, captured)]), "legacy");
    assert_eq!(vintage_of(&[(OWNER_CLAIM_KEY, empty)]), "fresh");
}

#[test]
fn the_answer_is_fixed_once_stored() {
    let (_dir, mut store) = prefs(&[]);
    assert_eq!(vintage(&mut store), "fresh");
    assert_eq!(store.text(VINTAGE_KEY).unwrap(), Some("fresh"));
    store
        .set(LEGACY_USAGE_KEYS[0], "{\"kind\":\"new\"}")
        .unwrap();
    assert_eq!(vintage(&mut store), "fresh");
}

fn modes(cx: &mut TestAppContext, entries: &[(&str, &str)]) -> tempfile::TempDir {
    let (dir, store) = prefs(entries);
    cx.update(|cx| {
        cx.set_global(store);
        install(cx);
    });
    dir
}

fn read(cx: &mut TestAppContext) -> (Pinned, Main) {
    cx.update(|cx: &mut App| (pinned(cx), main(cx)))
}

#[gpui::test]
fn a_fresh_install_lists_and_a_legacy_one_keeps_text(cx: &mut TestAppContext) {
    let _dir = modes(cx, &[]);
    assert_eq!(read(cx), (Pinned::Text, Main::List));
}

#[gpui::test]
fn an_untouched_legacy_install_keeps_text(cx: &mut TestAppContext) {
    let _dir = modes(cx, &[(LEGACY_USAGE_KEYS[0], "{\"kind\":\"new\"}")]);
    assert_eq!(read(cx).1, Main::Text);
}

#[gpui::test]
fn the_pinned_value_migrates_the_main_mode(cx: &mut TestAppContext) {
    let _dir = modes(
        cx,
        &[
            (LEGACY_USAGE_KEYS[0], "{\"kind\":\"new\"}"),
            (PINNED_KEY, "card"),
        ],
    );
    assert_eq!(read(cx), (Pinned::Card, Main::List));
    let _dir = modes(cx, &[(PINNED_KEY, "true")]);
    assert_eq!(read(cx), (Pinned::Card, Main::List));
    let _dir = modes(cx, &[(PINNED_KEY, "false")]);
    assert_eq!(read(cx), (Pinned::Text, Main::Text));
    let _dir = modes(cx, &[(PINNED_KEY, "list"), (MAIN_KEY, "text")]);
    assert_eq!(read(cx), (Pinned::List, Main::Text));
    let _dir = modes(cx, &[(PINNED_KEY, "grid"), (MAIN_KEY, "card")]);
    assert_eq!(
        read(cx),
        (Pinned::Text, Main::Text),
        "an unread key is still a trace"
    );
}

#[gpui::test]
fn the_migrated_main_mode_is_kept_once_read(cx: &mut TestAppContext) {
    let _dir = modes(cx, &[(PINNED_KEY, "text")]);
    assert_eq!(read(cx).1, Main::Text);
    cx.update(|cx| set_pinned(Pinned::Card, cx));
    assert_eq!(read(cx), (Pinned::Card, Main::Text));
}

#[gpui::test]
fn a_choice_is_stored_and_switches_even_when_unsaved(cx: &mut TestAppContext) {
    let dir = modes(cx, &[]);
    cx.update(|cx| set_main(Main::Text, cx));
    let stored = Preferences::load(dir.path()).unwrap();
    assert_eq!(stored.text(MAIN_KEY).unwrap(), Some("text"));
    let file = dir.path().join(crate::preferences::FILE);
    std::fs::remove_file(&file).unwrap();
    std::fs::create_dir(&file).unwrap();
    cx.update(|cx| set_pinned(Pinned::List, cx));
    assert_eq!(read(cx), (Pinned::List, Main::Text));
}
