use gpui::{Modifiers, ScrollDelta, ScrollEvent, TestAppContext, VisualTestContext, point, px};

use super::super::Shell;
use super::super::tests::{away, drawn, key_open, open, wait};
use crate::sessions::{
    Sessions, filter,
    group::Entry,
    main_list::lists,
    rail::{
        pinned::tests::{hover, rail},
        sections::Section,
    },
    view_tests::task,
};

/// Adds `n` draft tasks with no folder, and `projects` one-task projects, all older than a0.
fn add(cx: &mut VisualTestContext, drafts: usize, projects: usize) {
    cx.update(|_, cx| {
        let old = jiff::Timestamp::now().as_millisecond() - 9 * 24 * 60 * 60 * 1000;
        let loaded = cx.global_mut::<Sessions>().loaded.as_mut().unwrap();
        for i in 0..drafts {
            let at = old - 60_000 * i as i64;
            loaded
                .sessions
                .push(task(&format!("u{i}"), None, false, at));
        }
        for i in 0..projects {
            let dir = format!("/work/p{i:02}");
            let at = old - 60_000 * (i + drafts) as i64;
            loaded
                .sessions
                .push(task(&format!("p{i}"), Some(&dir), false, at));
        }
    });
}

fn show(cx: &mut VisualTestContext) {
    let tile = drawn(cx, "rail-projects").unwrap();
    hover(cx, tile.center());
    assert_eq!(open(cx), Some(Section::Projects));
}

#[gpui::test]
fn drafts_come_first_then_each_project_with_its_unpinned_count(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    add(cx, 1, 0);
    show(cx);
    let draft = drawn(cx, "task-rail-u0").expect("the draft");
    let app = drawn(cx, "rail-project-local:/work/app").expect("the project");
    assert!(draft.top() < app.top());
    assert!(
        drawn(cx, "task-rail-a3").is_none(),
        "tasks wait for the level"
    );
    let count = cx.update(|_, cx| {
        let l = lists(cx).unwrap();
        l.rail_projects[0].sessions.len()
    });
    assert_eq!(count, 4, "seven tasks, three pinned");
}

#[gpui::test]
fn a_pinned_project_stays_in_the_panel(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    cx.update(|_, cx| {
        let loaded = cx.global_mut::<Sessions>().loaded.as_mut().unwrap();
        loaded.settings.pinned_order = vec!["project:local:/work/app".into()];
    });
    let (main, rail_keys) = cx.update(|_, cx| {
        let l = lists(cx).unwrap();
        let main = l
            .main
            .iter()
            .any(|e| matches!(e, Entry::Project(p) if p.key == "local:/work/app"));
        (
            main,
            l.rail_projects
                .into_iter()
                .map(|p| p.key)
                .collect::<Vec<_>>(),
        )
    });
    assert!(!main, "the main list leaves it to Pinned");
    assert_eq!(rail_keys, ["local:/work/app"]);
    show(cx);
    assert!(drawn(cx, "rail-project-local:/work/app").is_some());
}

#[gpui::test]
fn five_drafts_then_show_all(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    add(cx, 7, 0);
    show(cx);
    assert!(drawn(cx, "task-rail-u4").is_some() && drawn(cx, "task-rail-u5").is_none());
    let more = drawn(cx, "rail-panel-show-all").unwrap();
    cx.simulate_click(more.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(drawn(cx, "task-rail-u6").is_some());
    assert!(drawn(cx, "rail-panel-show-all").is_none());
}

#[gpui::test]
fn twenty_projects_then_show_all_which_resets_with_the_panel(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    add(cx, 0, 22);
    show(cx);
    // /work/app first, then p00 on; rows keep `h-8` and overflow the list.
    let row = drawn(cx, "rail-project-local:/work/p18").expect("the twentieth row");
    assert_eq!(row.size.height, px(32.0));
    assert!(
        drawn(cx, "rail-project-local:/work/p19").is_none(),
        "twenty shown"
    );
    // Show all sits past the list's 420px, as Cindy's does.
    let list = drawn(cx, "rail-panel").unwrap();
    cx.simulate_event(ScrollEvent {
        position: list.center(),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-1000.0))),
        ..Default::default()
    });
    let more = drawn(cx, "rail-panel-show-projects").unwrap();
    assert_eq!(more.size.height, px(24.0), "`h-6`");
    cx.simulate_click(more.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(
        drawn(cx, "rail-project-local:/work/p21").is_some(),
        "all shown"
    );
    hover(cx, away());
    wait(cx, 120);
    assert_eq!(open(cx), None);
    show(cx);
    assert!(
        drawn(cx, "rail-project-local:/work/p19").is_none(),
        "folded again"
    );
}

#[gpui::test]
fn a_keyboard_open_focuses_the_first_project(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    key_open(cx, "rail-projects");
    let inside = cx.update(|window, cx| {
        let shell = cx.global::<Shell>().focus[0].clone().unwrap();
        shell.contains_focused(window, cx) && !shell.is_focused(window)
    });
    assert!(inside, "the project row took focus");
}

#[gpui::test]
fn the_created_sort_orders_projects_by_their_newest_task(cx: &mut TestAppContext) {
    let (_dir, cx) = rail(cx);
    add(cx, 0, 2);
    let keys = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| {
            let l = lists(cx).unwrap();
            l.rail_projects
                .into_iter()
                .map(|p| p.key)
                .collect::<Vec<_>>()
        })
    };
    cx.update(|_, cx| {
        let loaded = cx.global_mut::<Sessions>().loaded.as_mut().unwrap();
        let p1 = loaded.sessions.iter_mut().find(|s| s.id == "p1").unwrap();
        p1.created_at = i64::MAX;
    });
    let by_activity = keys(cx);
    cx.update(|_, cx| filter::set(filter::SORT_BY, "created", cx));
    assert_eq!(
        keys(cx),
        ["local:/work/p01", "local:/work/app", "local:/work/p00"]
    );
    assert_eq!(
        by_activity,
        ["local:/work/app", "local:/work/p00", "local:/work/p01"]
    );
}
