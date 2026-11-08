//! The grouped main list driven by clicks: folding, Show all and the saved dialogue fold.

use std::{path::Path, time::Duration};

use gpui::{
    Context, IntoElement, Modifiers, ParentElement as _, Render, Styled as _, TestAppContext,
    VisualTestContext, Window, WindowAppearance, div, px, size,
};

use super::{Loaded, Sessions, group::Session};
use crate::{os_appearance::Initial, preferences::Preferences, theme::Appearance, theme::Mode};

struct List;

impl Render for List {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .children(super::list(window, cx))
    }
}

fn task(id: &str, dir: Option<&str>, dialogue: bool, activity: i64) -> Session {
    Session {
        id: id.into(),
        title: id.into(),
        status: "active".into(),
        source: "desktop".into(),
        dialogue,
        working_dir: dir.map(str::to_owned),
        project_dir: None,
        remote_host: None,
        user_send_at: Some(activity),
        pinned_at: None,
        messages: 1,
        created_at: activity,
        activity_at: activity,
    }
}

fn open<'a>(cx: &'a mut TestAppContext, dir: &Path) -> &'a mut VisualTestContext {
    let week = 7 * 24 * 60 * 60 * 1000;
    let old = jiff::Timestamp::now().as_millisecond() - week;
    let mut sessions: Vec<Session> = (0..7)
        .map(|i| task(&format!("a{i}"), Some("/work/app"), false, old - i))
        .collect();
    sessions.push(task("d0", None, true, old));
    cx.update(|cx| {
        let initial = Initial {
            os: WindowAppearance::Dark,
            gpui_agrees: true,
        };
        ely_gpui_component::init_for_tests(cx);
        cx.set_global(crate::motion::ReducedMotion::default());
        cx.set_global(crate::units::Zoom(1.0));
        crate::sessions::install(std::path::PathBuf::new(), cx);
        crate::localdb::install(None, cx);
        cx.set_global(Appearance::new(Mode::System, None, initial, None).unwrap());
        cx.set_global(crate::i18n::Translator::new("en").unwrap());
        cx.set_global(Preferences::load(dir).unwrap());
        cx.global_mut::<Sessions>().loaded = Some(Loaded {
            sessions,
            recent: Vec::new(),
            aliases: Vec::new(),
        });
    });
    let (_, vcx) = cx.add_window_view(|_, _| List);
    vcx.simulate_resize(size(px(400.0), px(900.0)));
    vcx.run_until_parked();
    vcx
}

fn shown(cx: &mut VisualTestContext, id: &str) -> bool {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.debug_bounds(Box::leak(format!("task-{id}").into_boxed_str()))
        .is_some()
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} not drawn"));
    cx.simulate_click(bounds.center(), Modifiers::none());
    cx.run_until_parked();
}

fn wait(cx: &mut VisualTestContext, ms: u64) {
    cx.executor().advance_clock(Duration::from_millis(ms));
    cx.run_until_parked();
}

#[gpui::test]
fn folds_projects_and_shows_all(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    assert!(shown(cx, "a4") && !shown(cx, "a5"), "five, then Show all");
    click(cx, "project:local:/work/app-show-all");
    assert!(shown(cx, "a6"));
    click(cx, "project:local:/work/app-header");
    wait(cx, 100);
    assert!(shown(cx, "a0"), "still closing");
    wait(cx, 150);
    assert!(!shown(cx, "a0"), "closed, unmounted");
    click(cx, "project:local:/work/app-header");
    wait(cx, 250);
    assert!(
        shown(cx, "a4") && !shown(cx, "a5"),
        "Show all reset once closed"
    );
}

#[gpui::test]
fn the_dialogue_fold_is_saved(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    assert!(shown(cx, "d0"));
    click(cx, "dialogue:all-header");
    wait(cx, 250);
    assert!(!shown(cx, "d0"));
    let saved = Preferences::load(dir.path()).unwrap();
    assert_eq!(
        saved
            .text("cc-agent.sidebar.dialogueGroupCollapsed")
            .unwrap(),
        Some(r#"["all"]"#)
    );
}

#[gpui::test]
fn the_dialogue_fold_keeps_other_keys_cindy_reads(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let key = "cc-agent.sidebar.dialogueGroupCollapsed";
    let prefs = serde_json::json!({ key: r#"["device:x",1e400,"all"]"# });
    std::fs::write(dir.path().join(crate::preferences::FILE), prefs.to_string()).unwrap();
    let cx = open(cx, dir.path());
    assert!(!shown(cx, "d0"), "read through JSON.parse's number range");
    click(cx, "dialogue:all-header");
    wait(cx, 250);
    assert!(shown(cx, "d0"));
    let saved = Preferences::load(dir.path()).unwrap();
    assert_eq!(saved.text(key).unwrap(), Some(r#"["device:x"]"#));
}
