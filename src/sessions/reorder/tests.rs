use std::path::Path;

use serde_json::Value;

use super::{active, build, normalize};
use crate::sessions::{
    group::{self, tests::session},
    lists::entry_identity,
    settings::{self, Mutation},
};

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn matches_cindys_pinned_reorder() {
    let read = |name: &str| -> Vec<Value> {
        let path = format!(
            "{}/tests/fixtures/pinned-reorder/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    };
    for (case, want) in read("input.json").iter().zip(read("expected.json")) {
        let name = case["name"].as_str().unwrap();
        let platform = case["platform"].as_str().unwrap_or("linux");
        let key = |e: &str| entry_identity(e, platform);
        let sessions: Vec<_> = case["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .map(session)
            .collect();
        let pinned: Vec<String> = group::pinned_in_display_order(&sessions)
            .into_iter()
            .map(|s| s.id)
            .collect();
        let manual = strings(&case["manual"]);
        let shown = active(&manual, &pinned, &key);
        let (base, order) = build(&manual, &shown, &strings(&case["visible"]), &key);
        assert_eq!(shown, strings(&want["active"]), "{name}: active");
        assert_eq!(base, strings(&want["base"]), "{name}: base");
        assert_eq!(
            normalize(&order, &shown, &key),
            strings(&want["next"]),
            "{name}: next"
        );
    }
}

fn file(dir: &Path, order: &[&str]) {
    let text = serde_json::json!({ "pinnedOrder": order, "kept": { "a": 1 } });
    std::fs::write(dir.join(settings::FILE), text.to_string()).unwrap();
}

fn reorder(dir: &Path, base: &[&str], order: &[&str], platform: &str) -> Vec<String> {
    let own = |l: &[&str]| l.iter().map(|s| s.to_string()).collect();
    let mutation = Mutation::Reorder {
        base: own(base),
        order: own(order),
    };
    settings::save(dir, &mutation, platform).unwrap()
}

fn promote(dir: &Path, entry: &str) {
    settings::save(dir, &Mutation::Promote(entry.into()), "linux").unwrap();
}

#[test]
fn a_drag_onto_the_unranked_pins_is_saved_as_dropped() {
    let active: Vec<String> = ["ranked-a", "ranked-b", "unranked-a", "unranked-b"]
        .map(String::from)
        .to_vec();
    let key = |e: &str| e.to_owned();
    for (ranked, desired) in [
        (
            vec!["ranked-a", "ranked-b"],
            vec!["ranked-a", "unranked-a", "ranked-b", "unranked-b"],
        ),
        (
            vec!["ranked-a", "ranked-b"],
            vec!["ranked-a", "ranked-b", "unranked-b", "unranked-a"],
        ),
        (
            vec![],
            vec!["unranked-b", "ranked-a", "ranked-b", "unranked-a"],
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        file(dir.path(), &ranked);
        let ranked: Vec<String> = ranked.iter().map(|s| s.to_string()).collect();
        let visible: Vec<String> = desired.iter().map(|s| s.to_string()).collect();
        let (base, order) = build(&ranked, &active, &visible, &key);
        let base: Vec<&str> = base.iter().map(String::as_str).collect();
        let order: Vec<&str> = order.iter().map(String::as_str).collect();
        assert_eq!(reorder(dir.path(), &base, &order, "linux"), desired);
        let saved = settings::read(dir.path(), "linux").pinned_order;
        assert_eq!(saved, desired);
    }
}

#[test]
fn a_drag_keeps_hidden_slots_and_concurrent_pin_changes() {
    let dir = tempfile::tempdir().unwrap();
    file(dir.path(), &["ranked-pin", "hidden-pin"]);
    let own = |l: &[&str]| -> Vec<String> { l.iter().map(|s| s.to_string()).collect() };
    let (base, order) = build(
        &own(&["ranked-pin", "hidden-pin"]),
        &own(&["ranked-pin", "hidden-pin", "observed-a", "observed-b"]),
        &own(&["observed-b", "observed-a", "ranked-pin"]),
        &|e| e.to_owned(),
    );
    promote(dir.path(), "concurrent-pin");
    file(dir.path(), &["concurrent-pin", "hidden-pin"]);
    let base: Vec<&str> = base.iter().map(String::as_str).collect();
    let order: Vec<&str> = order.iter().map(String::as_str).collect();
    assert_eq!(
        reorder(dir.path(), &base, &order, "linux"),
        ["concurrent-pin", "observed-b", "hidden-pin", "observed-a"],
        "the removed rank stays removed"
    );
    let text = std::fs::read_to_string(dir.path().join(settings::FILE)).unwrap();
    assert!(text.contains("\"kept\""), "other fields survive: {text}");
}

#[test]
fn a_rank_outside_the_loaded_list_keeps_its_slot() {
    let dir = tempfile::tempdir().unwrap();
    let ranked = ["ranked-a", "not-loaded", "ranked-b"];
    file(dir.path(), &ranked);
    let own = |l: &[&str]| -> Vec<String> { l.iter().map(|s| s.to_string()).collect() };
    let (base, order) = build(
        &own(&ranked),
        &own(&["ranked-a", "ranked-b", "observed-pin"]),
        &own(&["ranked-b", "observed-pin", "ranked-a"]),
        &|e| e.to_owned(),
    );
    let base: Vec<&str> = base.iter().map(String::as_str).collect();
    let order: Vec<&str> = order.iter().map(String::as_str).collect();
    assert_eq!(
        reorder(dir.path(), &base, &order, "linux"),
        ["ranked-b", "not-loaded", "observed-pin", "ranked-a"]
    );
}

#[test]
fn a_stale_drag_keeps_a_pin_from_another_window() {
    let dir = tempfile::tempdir().unwrap();
    file(dir.path(), &["session-a", "session-b"]);
    promote(dir.path(), "session-c");
    let saved = reorder(
        dir.path(),
        &["session-a", "session-b"],
        &["session-b", "session-a"],
        "linux",
    );
    assert_eq!(saved, ["session-c", "session-b", "session-a"]);
}

const STORED: &str = "project:local:D:/École/Project-A";
const VARIANT: &str = "project:local:d:/école/project-a";

#[test]
fn windows_project_variants_rebase_by_identity() {
    let dir = tempfile::tempdir().unwrap();
    let base = [STORED, VARIANT, "session-a"];
    file(dir.path(), &base);
    settings::save(dir.path(), &Mutation::Promote("session-c".into()), "win32").unwrap();
    let saved = reorder(dir.path(), &base, &["session-a", VARIANT, STORED], "win32");
    assert_eq!(saved, ["session-c", "session-a", STORED]);

    let dir = tempfile::tempdir().unwrap();
    file(dir.path(), &[STORED, VARIANT, "session-c", "session-a"]);
    let saved = reorder(
        dir.path(),
        &[STORED, VARIANT, "session-a"],
        &["session-a", VARIANT, STORED],
        "win32",
    );
    assert_eq!(
        saved,
        ["session-a", "session-c", STORED],
        "the newcomer keeps its slot"
    );

    let dir = tempfile::tempdir().unwrap();
    file(dir.path(), &["session-a"]);
    let saved = reorder(
        dir.path(),
        &[STORED, "session-a", VARIANT],
        &[VARIANT, "session-a", STORED],
        "win32",
    );
    assert_eq!(
        saved,
        ["session-a"],
        "a removed identity is not resurrected"
    );
}

#[test]
fn a_malformed_reorder_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    file(dir.path(), &["a"]);
    for (base, order) in [(vec!["a", "a"], vec!["a"]), (vec!["a"], vec![""])] {
        let mutation = Mutation::Reorder {
            base: base.iter().map(|s| s.to_string()).collect(),
            order: order.iter().map(|s| s.to_string()).collect(),
        };
        assert!(settings::save(dir.path(), &mutation, "linux").is_err());
    }
    assert_eq!(settings::read(dir.path(), "linux").pinned_order, ["a"]);
}
