//! The golden's cases, each on a tree of its own; its paths are POSIX ones.
#![cfg(unix)]

use std::{fs, path::Path, time::Duration};

use gpui::TestAppContext;
use serde_json::{Map, Value, json};

use super::{Mounted, begin, end, focused, lookup, path, relative};
use crate::{
    local_path::smart::{self, SmartPaths},
    path_resolver::cache::serial,
};

fn put(root: &Path, file: &str) {
    let path = root.join(file);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, "").unwrap();
}

/// The hooks of `probes` drawn once, each with what it answers.
fn draw(probes: &[Value], at: &str, cx: &mut TestAppContext) -> Value {
    let real = |text: &str| text.replace("<root>", at);
    cx.update(|cx| {
        begin(cx);
        let mut out = Map::new();
        for probe in probes {
            let id = probe[0].as_str().unwrap();
            let src = probe[1].as_str().map(real);
            let dir = real(probe[4].as_str().unwrap_or("<root>/workspace"));
            let flags = (probe[2] == true, probe[3] == true);
            let media = path(id, src.as_deref(), &dir, flags, cx);
            let shown = json!({
                "src": media.src, "pending": media.pending, "revision": media.revision
            });
            out.insert(id.to_owned(), shown);
        }
        end(cx);
        Value::Object(out)
    })
}

/// One case's steps on the tree under `root`, as the golden's script runs them.
fn run(steps: &[Value], root: &Path, cx: &mut TestAppContext) -> Vec<Value> {
    let at = root.to_str().unwrap().to_owned();
    let workspace = format!("{at}/workspace");
    let mut probes: &[Value] = &[];
    let mut out = Vec::new();
    for step in steps {
        if let Some(render) = step["render"].as_array() {
            probes = render;
        } else if step["focus"] == true {
            cx.update(focused);
        } else if step["land"] == true {
            cx.run_until_parked();
        } else if let Some(ms) = step["tick"].as_u64() {
            cx.executor().advance_clock(Duration::from_millis(ms));
        } else if let Some(file) = step["write"].as_str() {
            put(root, file);
        } else if let Some(file) = step["remove"].as_str() {
            fs::remove_file(root.join(file)).unwrap();
        } else if let Some(count) = step["flood"].as_u64() {
            cx.update(|cx| {
                (0..count)
                    .for_each(|i| smart::ask(&format!("flood-{i}.ts"), &workspace, |_| {}, cx))
            });
            cx.run_until_parked();
        } else {
            panic!("unknown step {step}");
        }
        // React renders again after a state change; a render that changes nothing reads the same.
        out.push(draw(probes, &at, cx));
        if step["land"] == true {
            // A boundary's effect may send a lookup that the same landing takes in.
            cx.run_until_parked();
            *out.last_mut().unwrap() = draw(probes, &at, cx);
        }
    }
    out
}

#[gpui::test]
fn every_step_answers_as_cindys(cx: &mut TestAppContext) {
    let _serial = serial();
    let all = include_str!("../../tests/fixtures/workspace-media/expected.json");
    let all: Vec<Value> = serde_json::from_str(all).unwrap();
    let mut steps = 0;
    for case in &all {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("workspace")).unwrap();
        for file in case["files"].as_array().unwrap() {
            put(root.path(), file.as_str().unwrap());
        }
        // Cindy's cache, its hooks and its streaming keys start empty in each case.
        cx.update(|cx| {
            cx.set_global(SmartPaths::default());
            cx.set_global(Mounted::default());
        });
        let expected = case["out"]
            .to_string()
            .replace("<root>", root.path().to_str().unwrap());
        let expected: Vec<Value> = serde_json::from_str(&expected).unwrap();
        let ran = run(case["steps"].as_array().unwrap(), root.path(), cx);
        assert_eq!(ran.len(), expected.len(), "{}", case["name"]);
        for (n, (ran, expected)) in ran.iter().zip(&expected).enumerate() {
            assert_eq!(ran, expected, "{} step {n}", case["name"]);
            steps += 1;
        }
    }
    assert!(steps > 90, "{steps}");
}

#[test]
fn a_relative_path_has_no_drive_scheme_or_root() {
    for src in ["a.png", "sub/a.png", "./a.png", "1:a.png", "a b:c", "é:a"] {
        assert!(relative(src), "{src}");
    }
    for src in [
        "/a",
        "\\a",
        "C:/a",
        "c:\\a",
        "x:",
        "data:a",
        "a+b.c-d_e:f",
        "C:a",
    ] {
        assert!(!relative(src), "{src}");
    }
}

#[test]
fn a_lookup_path_stays_in_its_folder() {
    assert_eq!(lookup("./a//b\\c/../d").as_deref(), Some("a/b/d"));
    assert_eq!(lookup("a/.."), None);
    assert_eq!(lookup("a/../../b"), None);
    assert_eq!(lookup("./."), None);
}
