//! The grouped main list driven by clicks: folding, Show all and the saved dialogue fold.

use std::{path::Path, time::Duration};

use ely_gpui_component::primitives::FocusScope;
use gpui::{
    Context, FocusHandle, IntoElement, Modifiers, ParentElement as _, Render, Styled as _,
    TestAppContext, VisualTestContext, Window, WindowAppearance, div, px, size,
};

use super::{Loaded, Sessions, group::Session};
use crate::{os_appearance::Initial, preferences::Preferences, theme::Appearance, theme::Mode};

/// The list inside Ely's Tab scope, as the main window holds it.
struct List(FocusHandle);

impl Render for List {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        FocusScope::new(&self.0).size_full().child(
            div()
                .size_full()
                .flex()
                .flex_col()
                .children(super::list(window, cx)),
        )
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
        agent_kind: "cc".into(),
        orca_lead: false,
        muted: false,
        tokens: 0,
        cost_usd: 0.0,
        cost_amount: 0.0,
        cost_currency: None,
        activity_at: activity,
        preview: None,
        summary: None,
    }
}

pub(super) fn open<'a>(cx: &'a mut TestAppContext, dir: &Path) -> &'a mut VisualTestContext {
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
        cx.set_global(Preferences::load(dir).unwrap());
        crate::sessions::install(std::path::PathBuf::new(), None, cx);
        crate::localdb::install(None, cx);
        cx.set_global(Appearance::new(Mode::System, None, initial, None).unwrap());
        cx.set_global(crate::i18n::Translator::new("en").unwrap());
        cx.set_global(crate::toast::Toasts::default());
        cx.global_mut::<Sessions>().loaded = Some(Loaded {
            sessions,
            recent: Vec::new(),
            aliases: Vec::new(),
            settings: Default::default(),
        });
    });
    let (view, vcx) = cx.add_window_view(|_, cx| List(cx.focus_handle()));
    vcx.update(|window, cx| window.focus(&view.read(cx).0.clone(), cx));
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

pub(super) fn click(cx: &mut VisualTestContext, selector: &'static str) {
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

#[gpui::test]
fn the_pinned_section_folds_and_saves_it(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    cx.update(|_, cx| {
        let loaded = cx.global_mut::<Sessions>().loaded.as_mut().unwrap();
        loaded.sessions[0].pinned_at = Some(1);
    });
    assert!(shown(cx, "a0"), "pinned, above the list");
    let key = "cc-agent.sidebar.pinnedSectionCollapsed";
    click(cx, "pinned-title");
    wait(cx, 250);
    assert!(!shown(cx, "a0"));
    assert!(shown(cx, "a1"), "the project keeps its other tasks");
    let saved = |dir: &Path| {
        Preferences::load(dir)
            .unwrap()
            .text(key)
            .unwrap()
            .map(str::to_owned)
    };
    assert_eq!(saved(dir.path()).as_deref(), Some("true"));
    click(cx, "pinned-title");
    wait(cx, 250);
    assert!(shown(cx, "a0"));
    assert_eq!(saved(dir.path()), None, "expanding removes the key");
}

fn settings(cx: &mut VisualTestContext, pinned: &[&str], hidden: &[&str]) {
    cx.update(|_, cx| {
        let loaded = cx.global_mut::<Sessions>().loaded.as_mut().unwrap();
        loaded.settings.pinned_order = pinned.iter().map(|s| (*s).to_owned()).collect();
        loaded.settings.hidden = hidden.iter().map(|s| (*s).to_owned()).collect();
    });
}

#[gpui::test]
fn show_all_belongs_to_the_drawn_list(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    click(cx, "project:local:/work/app-show-all");
    assert!(shown(cx, "a6"));
    settings(cx, &["project:local:/work/app"], &[]);
    assert!(
        shown(cx, "a4") && !shown(cx, "a5"),
        "pinned, the project starts at five"
    );
    settings(cx, &[], &[]);
    assert!(!shown(cx, "a5"), "back in the main list, five again");
    click(cx, "project:local:/work/app-show-all");
    settings(cx, &[], &["local:/work/app"]);
    assert!(
        shown(cx, "a0") && !shown(cx, "a6"),
        "hidden: its tasks read as Chat, five shown"
    );
    settings(cx, &[], &[]);
    assert!(shown(cx, "a4") && !shown(cx, "a5"), "restored at five");
}

#[gpui::test]
fn an_unsaved_fold_does_not_turn(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    cx.update(|_, cx| {
        cx.global_mut::<Sessions>()
            .loaded
            .as_mut()
            .unwrap()
            .sessions[0]
            .pinned_at = Some(1);
    });
    assert!(shown(cx, "a0") && shown(cx, "d0"));
    std::fs::remove_dir_all(dir.path()).unwrap();
    click(cx, "pinned-title");
    click(cx, "dialogue:all-header");
    wait(cx, 250);
    assert!(shown(cx, "a0"), "the pinned section stays open");
    assert!(shown(cx, "d0"), "the dialogue group stays open");
}

#[gpui::test]
fn closing_pinned_resets_its_projects_show_all(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    settings(cx, &["project:local:/work/app"], &[]);
    assert!(!shown(cx, "a5"), "pinned at five");
    click(cx, "project:local:/work/app-show-all");
    assert!(shown(cx, "a6"));
    click(cx, "pinned-title");
    wait(cx, 250);
    assert!(!shown(cx, "a0"));
    click(cx, "pinned-title");
    wait(cx, 250);
    assert!(shown(cx, "a4") && !shown(cx, "a5"), "reopened at five");
}

fn pick(cx: &mut VisualTestContext, rows: &[&str]) {
    click(cx, "sidebar-filter-button");
    for row in rows {
        click(cx, Box::leak(format!("menu-row-{row}").into_boxed_str()));
    }
}

#[gpui::test]
fn the_filter_menu_writes_what_the_list_reads(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    cx.update(|_, cx| {
        let loaded = cx.global_mut::<Sessions>().loaded.as_mut().unwrap();
        loaded.sessions[1].agent_kind = "codex".into();
    });
    let saved = |dir: &Path, key: &str| {
        Preferences::load(dir)
            .unwrap()
            .text(key)
            .unwrap()
            .map(str::to_owned)
    };
    pick(cx, &["Filters", "Harness", "Codex"]);
    assert_eq!(
        saved(dir.path(), "cc-agent.sidebar.filter.vendor").as_deref(),
        Some("codex")
    );
    assert!(
        shown(cx, "a1") && !shown(cx, "a0") && !shown(cx, "d0"),
        "only the Codex task"
    );
    pick(cx, &["Filters", "Reset filters"]);
    assert!(shown(cx, "a0") && shown(cx, "d0"));
    pick(cx, &["Group by", "Project"]);
    assert_eq!(
        saved(dir.path(), "cc-agent.sidebar.filter.groupBy").as_deref(),
        Some("flat")
    );
    assert!(shown(cx, "a6"), "flat: every task, past the project's five");
    pick(cx, &["Group by", "Chat"]);
    assert_eq!(
        saved(dir.path(), "cc-agent.sidebar.filter.groupDialogue").as_deref(),
        Some("false")
    );
}

#[gpui::test]
fn the_scope_header_stands_before_the_list_loads(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    cx.update(|_, cx| cx.global_mut::<Sessions>().loaded = None);
    assert!(!shown(cx, "a0"));
    pick(cx, &["Group by", "Chat"]);
    let saved = Preferences::load(dir.path()).unwrap();
    assert_eq!(
        saved.text("cc-agent.sidebar.filter.groupDialogue").unwrap(),
        Some("false")
    );
}

#[gpui::test]
fn task_info_follows_the_menu(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    cx.update(|_, cx| {
        let s = &mut cx
            .global_mut::<Sessions>()
            .loaded
            .as_mut()
            .unwrap()
            .sessions[0];
        (s.tokens, s.cost_usd) = (1_500, 0.5);
    });
    let has = |cx: &mut VisualTestContext, field: &str| {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        cx.debug_bounds(Box::leak(format!("task-{field}-a0").into_boxed_str()))
            .is_some()
    };
    assert!(has(cx, "time") && !has(cx, "tokens"));
    pick(cx, &["Session info", "Tokens"]);
    pick(cx, &["Session info", "Time"]);
    assert!(has(cx, "tokens") && !has(cx, "time"));
    let saved = Preferences::load(dir.path()).unwrap();
    assert_eq!(
        saved.text(super::meta::TASK_INFO).unwrap(),
        Some(r#"{"version":1,"fields":["tokens"]}"#)
    );
}

#[gpui::test]
fn the_display_menus_save_both_modes(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    let saved = |dir: &Path, key: &str| {
        Preferences::load(dir)
            .unwrap()
            .text(key)
            .unwrap()
            .map(str::to_owned)
    };
    pick(cx, &["Display", "Text"]);
    cx.update(|_, cx| {
        let loaded = cx.global_mut::<Sessions>().loaded.as_mut().unwrap();
        loaded.sessions[0].pinned_at = Some(1);
    });
    cx.update(|window, _| window.refresh());
    assert_eq!(
        saved(dir.path(), "sidebar.mainListMode").as_deref(),
        Some("text")
    );
    click(cx, "pinned-view-style-button");
    click(cx, "menu-row-Cards");
    assert_eq!(
        saved(dir.path(), "sidebar.cardMode").as_deref(),
        Some("card")
    );
    let modes = cx.update(|_, cx| (super::view_mode::pinned(cx), super::view_mode::main(cx)));
    assert_eq!(
        modes,
        (super::view_mode::Pinned::Card, super::view_mode::Main::Text)
    );
}

#[gpui::test]
fn list_mode_draws_list_rows_inset_in_groups(cx: &mut TestAppContext) {
    use super::view_mode::{self, Main, Pinned};
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    let bounds = |cx: &mut VisualTestContext, id: &str| {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        cx.debug_bounds(Box::leak(format!("task-{id}").into_boxed_str()))
            .unwrap()
    };
    // 59.4, snapped to the device pixel grid.
    let list_height = |h: gpui::Pixels| (f32::from(h) - (42.0 + 1.45 * 12.0)).abs() < 0.2;
    let list = bounds(cx, "a1");
    assert!(list_height(list.size.height), "a fresh install lists");
    cx.update(|_, cx| view_mode::set_main(Main::Text, cx));
    let text = bounds(cx, "a1");
    assert_eq!(text.size.height, px(32.0));
    assert_eq!(
        list.origin.x - text.origin.x,
        px(8.0),
        "pl-2 in list mode only"
    );
    cx.update(|_, cx| {
        let loaded = cx.global_mut::<Sessions>().loaded.as_mut().unwrap();
        loaded.sessions[0].pinned_at = Some(1);
        view_mode::set_pinned(Pinned::List, cx);
    });
    assert!(list_height(bounds(cx, "a0").size.height));
    assert_eq!(
        bounds(cx, "a1").size.height,
        px(32.0),
        "the main list keeps text"
    );
}

#[gpui::test]
fn card_mode_lays_pins_out_in_columns(cx: &mut TestAppContext) {
    use super::view_mode::{self, Pinned};
    let dir = tempfile::tempdir().unwrap();
    let cx = open(cx, dir.path());
    cx.update(|_, cx| {
        let loaded = cx.global_mut::<Sessions>().loaded.as_mut().unwrap();
        for (i, s) in loaded.sessions.iter_mut().take(3).enumerate() {
            s.pinned_at = Some(i as i64 + 1);
            s.summary = Some("summary".into());
        }
        view_mode::set_pinned(Pinned::Card, cx);
    });
    let at = |cx: &mut VisualTestContext, id: &str| {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        cx.debug_bounds(Box::leak(format!("task-{id}").into_boxed_str()))
            .unwrap()
            .origin
    };
    // Newest pin first: a2, a1, a0.
    let (a, b, c) = (at(cx, "a2"), at(cx, "a1"), at(cx, "a0"));
    assert!(
        a.y == b.y && a.x < b.x,
        "two columns side by side: {a:?} {b:?}"
    );
    assert!(c.x == a.x && c.y > a.y, "the third under the first: {c:?}");
    cx.simulate_resize(size(px(240.0), px(900.0)));
    let (a, b) = (at(cx, "a2"), at(cx, "a1"));
    assert!(
        a.x == b.x && b.y > a.y,
        "one column when narrow: {a:?} {b:?}"
    );
}
