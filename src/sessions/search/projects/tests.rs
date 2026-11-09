use gpui::{TestAppContext, VisualTestContext};
use rusqlite::Connection;

use super::{known, loaded, next, reconcile};
use crate::{
    localdb::search::conversations::tests::task,
    sessions::{
        Sessions,
        group::{Recent, Session},
        search::run::{self, tests::wait},
        view_tests::{self, click},
    },
};

fn keys(list: &[&str]) -> Vec<String> {
    list.iter().map(|k| (*k).to_owned()).collect()
}

#[test]
fn a_choice_adds_then_the_last_removal_is_all() {
    let one = next(None, "local:/a");
    assert_eq!(one, Some(keys(&["local:/a"])));
    let two = next(one.as_deref(), "local:/b");
    assert_eq!(two, Some(keys(&["local:/a", "local:/b"])));
    let back = next(two.as_deref(), "local:/a/");
    assert_eq!(
        back,
        Some(keys(&["local:/b"])),
        "compared on the normalized key"
    );
    assert_eq!(next(back.as_deref(), "local:/b"), None);
}

fn at(id: &str, dir: &str, status: &str) -> Session {
    Session {
        status: status.into(),
        ..view_tests::task(id, Some(dir), false, 1)
    }
}

#[test]
fn every_status_counts_but_workers_pins_and_hidden_projects_do_not() {
    let all = vec![
        at("a", "/work/app", "active"),
        at("b", "/work/app", "archived"),
        Session {
            orca_worker: true,
            ..at("w", "/work/app", "active")
        },
        Session {
            pinned_at: Some(1),
            ..at("p", "/work/app", "active")
        },
        at("h", "/work/hidden", "active"),
    ];
    let projects = known(&all, &[], &[], &keys(&["/work/hidden"]));
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].key, "local:/work/app");
    let ids: Vec<&str> = projects[0].sessions.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&"a") && ids.contains(&"b"));
}

#[test]
fn the_selection_keeps_known_projects_once_each() {
    let projects = known(&[at("a", "/work/app", "active")], &[], &[], &[]);
    let kept = reconcile(
        &keys(&["local:/work/app/", "local:/gone", "local:/work/app"]),
        &projects,
    );
    assert_eq!(kept, keys(&["local:/work/app"]));
}

fn recent(path: &str, last_used: i64) -> Recent {
    Recent {
        path: path.into(),
        kinds: Vec::new(),
        last_used,
    }
}

#[test]
fn projects_run_by_latest_activity_or_last_use() {
    let a = Session {
        activity_at: 10,
        ..at("a", "/work/a", "active")
    };
    let b = Session {
        activity_at: 100,
        ..at("b", "/work/b", "active")
    };
    let projects = known(&[a, b], &[recent("/work/c", 50)], &[], &[]);
    let order: Vec<&str> = projects.iter().map(|p| p.key.as_str()).collect();
    assert_eq!(order, ["local:/work/b", "local:/work/c", "local:/work/a"]);
}

fn tasks(db: &Connection) {
    task(db, "a0", "plan in app", 10);
    task(db, "x", "plan elsewhere", 20);
}

fn found(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|_, cx| {
        run::state(crate::sessions::search::Which::Inline, cx)
            .1
            .map_or_else(Vec::new, |r| {
                r.results.iter().map(|i| i.session.id.clone()).collect()
            })
    })
}

fn choose(cx: &mut VisualTestContext, row: &'static str) {
    click(cx, "search-filter-button");
    click(cx, "menu-row-Projects");
    click(cx, row);
    wait(cx, 250);
}

#[gpui::test]
fn a_project_choice_scopes_the_search_to_its_tasks(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = run::tests::search(cx, dir.path(), Some(tasks));
    cx.simulate_input("plan");
    wait(cx, 250);
    assert_eq!(found(cx).len(), 2);
    choose(cx, "menu-row-app · 7");
    assert_eq!(found(cx), keys(&["a0"]));
    choose(cx, "menu-row-All projects");
    assert_eq!(found(cx).len(), 2);
}

#[gpui::test]
fn a_project_that_leaves_drops_out_of_the_selection(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = run::tests::search(cx, dir.path(), Some(tasks));
    cx.simulate_input("plan");
    choose(cx, "menu-row-app · 7");
    assert_eq!(found(cx), keys(&["a0"]));
    cx.update(|_, cx| {
        cx.global_mut::<Sessions>()
            .loaded
            .as_mut()
            .unwrap()
            .known
            .clear();
        loaded(cx);
    });
    wait(cx, 250);
    let selection = cx.update(|_, cx| {
        crate::sessions::search::of(crate::sessions::search::Which::Inline, cx)
            .options
            .projects
            .clone()
    });
    assert_eq!(selection, None, "an empty selection is all");
    assert_eq!(found(cx).len(), 2);
}

#[gpui::test]
fn a_choice_that_keeps_the_scope_keeps_the_page(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let cx = run::tests::search(cx, dir.path(), Some(tasks));
    let pick = |cx: &mut VisualTestContext, keys: Option<Vec<String>>| {
        cx.update(|_, cx| {
            crate::sessions::search::of_mut(crate::sessions::search::Which::Inline, cx)
                .options
                .projects = keys;
            run::sync(crate::sessions::search::Which::Inline, cx);
            run::state(crate::sessions::search::Which::Inline, cx).0
        })
    };
    cx.update(|_, cx| {
        let empty = [recent("/work/e1", 1), recent("/work/e2", 2)];
        cx.global_mut::<Sessions>().loaded.as_mut().unwrap().known = known(&[], &empty, &[], &[]);
    });
    cx.simulate_input("plan");
    pick(cx, Some(keys(&["local:/work/e1"])));
    wait(cx, 900);
    let both = Some(keys(&["local:/work/e1", "local:/work/e2"]));
    assert_eq!(
        pick(cx, both),
        run::Status::Done,
        "no task enters the scope"
    );
    assert!(found(cx).is_empty());
}
