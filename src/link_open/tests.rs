use gpui::{Modifiers, MouseButton, TestAppContext};

use super::{Kind, Target, for_url, install, is_local, preference, separate};
use crate::{
    preferences::Preferences,
    rich_answer::js::{self, Js},
};

#[test]
fn local_urls_are_cindys() {
    let all = js::parse(include_str!("../../tests/fixtures/link-open/expected.json")).unwrap();
    let cases = all.obj().unwrap()["isLocalOpenUrl"].arr().unwrap();
    assert!(cases.len() > 50);
    for case in cases {
        let [args, expected] = case.arr().unwrap() else {
            panic!("case shape")
        };
        let url = args.arr().unwrap()[0].str().unwrap();
        assert_eq!(&Js::Bool(is_local(url)), expected, "{url:?}");
    }
}

fn start(cx: &mut TestAppContext, stored: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    cx.update(|cx| {
        let mut prefs = Preferences::load(dir.path()).unwrap();
        for (key, value) in stored {
            prefs.set(key, value).unwrap();
        }
        cx.set_global(prefs);
        install(cx);
    });
    dir
}

fn stored(cx: &mut TestAppContext, key: &str) -> Option<String> {
    cx.update(|cx| {
        cx.global::<Preferences>()
            .text(key)
            .unwrap()
            .map(str::to_owned)
    })
}

const WEB: &str = "chat.webLinkOpenPreference";
const LOCAL: &str = "chat.localLinkOpenPreference";
const LEGACY: &str = "chat.linkOpenPreference";

#[gpui::test]
fn the_defaults_part_web_from_local(cx: &mut TestAppContext) {
    let _dir = start(cx, &[]);
    cx.update(|cx| {
        assert_eq!(preference(Kind::Web, cx), Target::External);
        assert_eq!(preference(Kind::Local, cx), Target::Sidebar);
        assert_eq!(for_url("https://example.com/", cx), Target::External);
        assert_eq!(for_url("http://localhost:3000/", cx), Target::Sidebar);
    });
}

#[gpui::test]
fn an_override_is_read_once_and_a_stored_default_goes(cx: &mut TestAppContext) {
    let _dir = start(cx, &[(WEB, "sidebar"), (LOCAL, "sidebar")]);
    cx.update(|cx| {
        assert_eq!(preference(Kind::Web, cx), Target::Sidebar);
        assert_eq!(preference(Kind::Local, cx), Target::Sidebar);
        // The memory answers from here on.
        cx.global_mut::<Preferences>().set(WEB, "external").unwrap();
        assert_eq!(preference(Kind::Web, cx), Target::Sidebar);
    });
    assert_eq!(stored(cx, LOCAL), None, "a default is no override");
}

#[gpui::test]
fn an_unset_kind_reads_the_file_each_time(cx: &mut TestAppContext) {
    let _dir = start(cx, &[(WEB, "nonsense")]);
    cx.update(|cx| {
        assert_eq!(preference(Kind::Web, cx), Target::External);
        cx.global_mut::<Preferences>().set(WEB, "sidebar").unwrap();
        assert_eq!(preference(Kind::Web, cx), Target::Sidebar);
    });
    assert_eq!(stored(cx, WEB).as_deref(), Some("sidebar"));
}

#[gpui::test]
fn the_old_single_choice_moves_where_it_differs(cx: &mut TestAppContext) {
    let _dir = start(cx, &[(LEGACY, "sidebar")]);
    cx.update(|cx| assert_eq!(preference(Kind::Web, cx), Target::Sidebar));
    assert_eq!(stored(cx, WEB).as_deref(), Some("sidebar"));
    assert_eq!(stored(cx, LOCAL), None, "sidebar is the local default");
    assert_eq!(stored(cx, LEGACY), None);
    cx.update(|cx| assert_eq!(preference(Kind::Local, cx), Target::Sidebar));
}

#[gpui::test]
fn the_old_choice_leaves_a_newer_one_alone(cx: &mut TestAppContext) {
    let _dir = start(cx, &[(LEGACY, "external"), (LOCAL, "sidebar")]);
    cx.update(|cx| {
        assert_eq!(preference(Kind::Local, cx), Target::Sidebar);
        assert_eq!(preference(Kind::Web, cx), Target::External);
    });
    assert_eq!(stored(cx, LEGACY), None);
    assert_eq!(stored(cx, WEB), None, "external is the web default");
}

#[gpui::test]
fn an_empty_choice_counts_as_unset(cx: &mut TestAppContext) {
    let _dir = start(cx, &[(LEGACY, "sidebar"), (WEB, "")]);
    cx.update(|cx| assert_eq!(preference(Kind::Web, cx), Target::Sidebar));
    assert_eq!(stored(cx, WEB).as_deref(), Some("sidebar"));
}

#[gpui::test]
fn the_old_choice_moves_once_a_run(cx: &mut TestAppContext) {
    let _dir = start(cx, &[]);
    cx.update(|cx| {
        assert_eq!(preference(Kind::Web, cx), Target::External);
        cx.global_mut::<Preferences>()
            .set(LEGACY, "sidebar")
            .unwrap();
        assert_eq!(preference(Kind::Web, cx), Target::External);
    });
    assert_eq!(stored(cx, LEGACY).as_deref(), Some("sidebar"));
}

#[gpui::test]
fn an_unknown_old_choice_stays(cx: &mut TestAppContext) {
    let _dir = start(cx, &[(LEGACY, "browser")]);
    cx.update(|cx| assert_eq!(preference(Kind::Local, cx), Target::Sidebar));
    assert_eq!(stored(cx, LEGACY).as_deref(), Some("browser"));
}

#[test]
fn a_modifier_or_the_middle_button_opens_apart() {
    let with = |change: fn(&mut Modifiers)| {
        let mut modifiers = Modifiers::none();
        change(&mut modifiers);
        modifiers
    };
    assert!(!separate(Modifiers::none(), MouseButton::Left));
    assert!(!separate(with(|m| m.alt = true), MouseButton::Left));
    assert!(!separate(with(|m| m.function = true), MouseButton::Right));
    assert!(separate(with(|m| m.shift = true), MouseButton::Left));
    assert!(separate(with(|m| m.platform = true), MouseButton::Left));
    assert!(separate(with(|m| m.control = true), MouseButton::Left));
    assert!(separate(Modifiers::none(), MouseButton::Middle));
}

#[cfg(unix)]
#[gpui::test]
fn a_failed_move_keeps_the_old_choice(cx: &mut TestAppContext) {
    use std::{fs, os::unix::fs::PermissionsExt as _};
    let dir = start(cx, &[(LEGACY, "sidebar")]);
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o555)).unwrap();
    cx.update(|cx| assert_eq!(preference(Kind::Web, cx), Target::External));
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(stored(cx, LEGACY).as_deref(), Some("sidebar"));
    assert_eq!(stored(cx, WEB), None);
    let kept = Preferences::load(dir.path()).unwrap();
    assert_eq!(kept.text(LEGACY).unwrap(), Some("sidebar"));
}
