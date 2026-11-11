//! The golden's cases, each on a tree of its own; its paths are POSIX ones.
#![cfg(unix)]

use std::{fs, path::Path, time::Duration};

use futures::FutureExt as _;
use gpui::{Task, TestAppContext};
use serde_json::{Value, json};

use super::{Smart, SmartPaths, ask, now, peek, recheck, resolve_smart};
use crate::path_resolver::{Kind, cache::serial};

fn shown(result: &Smart) -> Value {
    match result {
        Smart::Unique(path, Kind::File) => {
            json!({ "status": "unique", "absPath": path, "kind": "file" })
        }
        Smart::Unique(path, Kind::Directory) => {
            json!({ "status": "unique", "absPath": path, "kind": "directory" })
        }
        Smart::Multiple(all) => json!({ "status": "multiple", "candidates": all }),
        Smart::None(fallback) => json!({ "status": "none", "fallbackAbsPath": fallback }),
    }
}

fn put(root: &Path, file: &str) {
    let path = root.join(file);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, "").unwrap();
}

/// One case's steps on the tree under `root`, as the golden's script runs them.
fn run(steps: &[Value], root: &Path, cx: &mut TestAppContext) -> Vec<Value> {
    let at = root.to_str().unwrap().to_owned();
    let workspace = format!("{at}/workspace");
    let real = |text: &Value| text.as_str().unwrap().replace("<root>", &at);
    let folder = |given: &Value| match given {
        Value::Null => workspace.clone(),
        given => real(given),
    };
    let mut mark = 0;
    let mut direct: Vec<Task<Smart>> = Vec::new();
    let mut out = Vec::new();
    for step in steps {
        if let Some(ops) = step["do"].as_array() {
            let answers = cx.update(|cx| {
                let answer = |op: &Value| {
                    let now = now(cx);
                    let (href, dir) = (|| real(&op[1]), || folder(&op[3]));
                    match op[0].as_str().unwrap() {
                        "mark" => mark = now,
                        "ask" => ask(&href(), &dir(), |_| {}, cx),
                        "peek" => return json!(peek(&href(), &dir(), cx).map(shown)),
                        "recheck" => {
                            let boundary = match op[2].as_str() {
                                Some("now") => Some(now),
                                Some("mark") => Some(mark),
                                _ => None,
                            };
                            return json!(recheck(&href(), &dir(), boundary, cx));
                        }
                        "direct" => {
                            direct.push(resolve_smart(&href(), &dir(), op[2] == true, cx));
                        }
                        op => panic!("unknown op {op}"),
                    }
                    Value::Null
                };
                ops.iter().map(answer).collect::<Vec<_>>()
            });
            out.push(json!(answers));
        } else if step["land"] == true {
            cx.run_until_parked();
            let landed = direct
                .drain(..)
                .map(|task| task.now_or_never().expect("landed"));
            out.push(json!(
                landed.map(|result| shown(&result)).collect::<Vec<_>>()
            ));
        } else {
            if let Some(ms) = step["tick"].as_u64() {
                cx.executor().advance_clock(Duration::from_millis(ms));
            } else if let Some(file) = step["write"].as_str() {
                put(root, file);
            } else if let Some(file) = step["remove"].as_str() {
                fs::remove_file(root.join(file)).unwrap();
            } else if let Some(count) = step["flood"].as_u64() {
                cx.update(|cx| {
                    (0..count).for_each(|i| ask(&format!("flood-{i}.ts"), &workspace, |_| {}, cx))
                });
                cx.run_until_parked();
            }
            out.push(Value::Null);
        }
    }
    out
}

#[gpui::test]
fn every_step_answers_as_cindys(cx: &mut TestAppContext) {
    let _serial = serial();
    let all = include_str!("../../../tests/fixtures/smart-paths/expected.json");
    let all: Vec<Value> = serde_json::from_str(all).unwrap();
    let mut steps = 0;
    for case in &all {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("workspace")).unwrap();
        for file in case["files"].as_array().unwrap() {
            put(root.path(), file.as_str().unwrap());
        }
        // Cindy's cache starts empty in each case.
        cx.update(|cx| cx.set_global(SmartPaths::default()));
        let expected = case["out"]
            .to_string()
            .replace("<root>", root.path().to_str().unwrap());
        let expected: Vec<Value> = serde_json::from_str(&expected).unwrap();
        let ran = run(case["steps"].as_array().unwrap(), root.path(), cx);
        for (n, (ran, expected)) in ran.iter().zip(&expected).enumerate() {
            assert_eq!(ran, expected, "{} step {n}", case["name"]);
            steps += 1;
        }
    }
    assert!(steps > 90, "{steps}");
}

/// The answer for `href` once everything in flight has landed.
fn landed(href: &str, dir: &str, cx: &mut TestAppContext) -> Option<Smart> {
    cx.run_until_parked();
    cx.update(|cx| peek(href, dir, cx).cloned())
}

// `logs/review/9.6h2-r1-review-differential.log`: stamps are whole milliseconds.
#[gpui::test]
fn a_send_in_the_boundarys_millisecond_is_checked_again(cx: &mut TestAppContext) {
    let _serial = serial();
    for (micros, again) in [(0, true), (500, true), (1000, false)] {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().to_str().unwrap().to_owned();
        cx.update(|cx| cx.set_global(SmartPaths::default()));
        let boundary = cx.update(now);
        cx.executor().advance_clock(Duration::from_micros(micros));
        cx.update(|cx| ask("late.ts", &dir, |_| {}, cx));
        assert!(cx.update(|cx| recheck("late.ts", &dir, Some(boundary), cx)));
        // The file appears after the first walk would have passed it.
        cx.executor().tick();
        put(root.path(), "sub/late.ts");
        let found = landed("late.ts", &dir, cx);
        assert_eq!(matches!(found, Some(Smart::Unique(..))), again, "{micros}");
    }
}

#[gpui::test]
fn a_negative_answer_two_stamps_old_is_checked_again(cx: &mut TestAppContext) {
    let _serial = serial();
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().to_str().unwrap().to_owned();
    cx.update(now);
    cx.executor().advance_clock(Duration::from_micros(500));
    cx.update(|cx| ask("ghost.ts", &dir, |_| {}, cx));
    assert!(matches!(landed("ghost.ts", &dir, cx), Some(Smart::None(_))));
    cx.executor()
        .advance_clock(Duration::from_micros(1_999_000));
    assert!(!cx.update(|cx| recheck("ghost.ts", &dir, None, cx)));
    cx.executor().advance_clock(Duration::from_micros(500));
    assert!(cx.update(|cx| recheck("ghost.ts", &dir, None, cx)));
}
