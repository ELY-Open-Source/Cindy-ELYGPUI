//! Remove from sidebar: the confirm, the write Cindy reads, and the rules `setProjectHidden` keeps.

use gpui::{MouseButton, TestAppContext, VisualTestContext};

use crate::sessions::{
    Sessions, settings,
    view_tests::{click, open},
};

const HEADER: &str = "project:local:/work/app-header";

fn menu(cx: &mut VisualTestContext) {
    let at = cx.debug_bounds(HEADER).unwrap().center();
    cx.simulate_event(gpui::MouseDownEvent {
        position: at,
        modifiers: gpui::Modifiers::none(),
        button: MouseButton::Right,
        click_count: 1,
        first_mouse: false,
    });
    cx.run_until_parked();
    click(cx, "menu-row-Remove Project from Sidebar");
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

fn hidden(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|_, cx| {
        cx.global::<Sessions>()
            .loaded
            .as_ref()
            .unwrap()
            .settings
            .hidden
            .clone()
    })
}

fn toasts(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|window, cx| {
        cx.global::<crate::toast::Toasts>()
            .items(window.window_handle().window_id())
            .iter()
            .map(|item| item.message.to_string())
            .collect()
    })
}

#[gpui::test]
fn a_confirmed_remove_hides_the_project(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let owner = dir.path().join("owner");
    let cx = open(cx, dir.path());
    cx.update(|_, cx| cx.global_mut::<Sessions>().owner_dir = Some(owner.clone()));
    menu(cx);
    assert!(
        cx.debug_bounds("confirm-dialog-confirm").is_some(),
        "asked first"
    );
    click(cx, "confirm-dialog-confirm");
    assert_eq!(hidden(cx), ["local:/work/app"]);
    let saved = std::fs::read_to_string(owner.join("sidebar-settings.json")).unwrap();
    let saved: serde_json::Value = serde_json::from_str(&saved).unwrap();
    assert_eq!(
        saved["hiddenProjectKeys"],
        serde_json::json!(["local:/work/app"])
    );
    assert!(toasts(cx).is_empty());
}

#[gpui::test]
fn escape_keeps_the_project(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let owner = dir.path().join("owner");
    let cx = open(cx, dir.path());
    cx.update(|_, cx| cx.global_mut::<Sessions>().owner_dir = Some(owner.clone()));
    menu(cx);
    cx.simulate_keystrokes("escape");
    assert!(hidden(cx).is_empty());
    assert!(!owner.join("sidebar-settings.json").exists());
}

#[gpui::test]
fn a_failed_write_toasts(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    menu(cx);
    click(cx, "confirm-dialog-confirm");
    assert!(hidden(cx).is_empty());
    assert_eq!(
        toasts(cx),
        ["Couldn’t remove the project from the sidebar — try again."]
    );
}

#[test]
fn hiding_keeps_the_file_and_one_spelling_per_project() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(settings::FILE);
    std::fs::write(
        &file,
        r#"{"pinnedOrder":["a"],"hiddenProjectKeys":["C:/Work/App",7],"other":true}"#,
    )
    .unwrap();
    let keys = settings::hide(dir.path(), "local:c:/work/app", "win32").unwrap();
    assert_eq!(
        keys,
        ["local:C:/Work/App", "local:7"],
        "already hidden on Windows"
    );
    let keys = settings::hide(dir.path(), "/srv/site/", "win32").unwrap();
    assert_eq!(keys.last().map(String::as_str), Some("local:/srv/site"));
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(saved["other"], true);
    assert_eq!(saved["pinnedOrder"], serde_json::json!(["a"]));
    assert!(settings::hide(dir.path(), "  ", "linux").is_err());
}

#[test]
fn the_ten_thousandth_hidden_project_is_the_last() {
    let dir = tempfile::tempdir().unwrap();
    let keys: Vec<String> = (0..10_000).map(|i| format!("local:/p{i}")).collect();
    let text = serde_json::json!({ "hiddenProjectKeys": keys }).to_string();
    std::fs::write(dir.path().join(settings::FILE), text).unwrap();
    assert!(settings::hide(dir.path(), "/new", "linux").is_err());
    assert_eq!(
        settings::hide(dir.path(), "/p9", "linux").unwrap().len(),
        10_000
    );
}

#[gpui::test]
fn asks_wait_their_turn_and_answer_in_order(cx: &mut TestAppContext) {
    use crate::confirm::{Ask, ask};
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    let question = |title: &str| Ask {
        title: title.into(),
        description: None,
        confirm_text: None,
        auto_focus_confirm: false,
    };
    let (first, second) = cx.update(|window, cx| {
        (
            ask(window, question("First"), cx),
            ask(window, question("Second"), cx),
        )
    });
    cx.run_until_parked();
    click(cx, "confirm-dialog-cancel");
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(150));
    cx.run_until_parked();
    use futures::FutureExt as _;
    let mut second = second;
    click(cx, "confirm-dialog-confirm");
    assert_eq!(
        (&mut second).now_or_never(),
        None,
        "the first fades out inert"
    );
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(100));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    click(cx, "confirm-dialog-confirm");
    let answers = (first.now_or_never(), second.now_or_never());
    assert_eq!(answers, (Some(Ok(false)), Some(Ok(true))));
}
