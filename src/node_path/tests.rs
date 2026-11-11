use serde_json::Value;

use super::{posix, win32};

/// Runs `port` on every case of `function` and returns how many there were.
fn golden(function: &str, port: impl Fn(&Value) -> Value) -> usize {
    let all = include_str!("../../tests/fixtures/node-path/expected.json");
    let all: Value = serde_json::from_str(all).unwrap();
    let cases = all[function].as_array().unwrap();
    for case in cases {
        assert_eq!(port(&case[0]), case[1], "{function} {}", case[0]);
    }
    cases.len()
}

fn one(port: impl Fn(&str) -> String) -> impl Fn(&Value) -> Value {
    move |input| port(input.as_str().unwrap()).into()
}

fn two(port: impl Fn(&[&str]) -> String) -> impl Fn(&Value) -> Value {
    move |input| port(&[input[0].as_str().unwrap(), input[1].as_str().unwrap()]).into()
}

#[test]
fn posix_paths_are_nodes() {
    assert!(golden("posixNormalize", one(posix::normalize)) > 2000);
    assert!(golden("posixBasename", one(|path| posix::basename(path).into())) > 2000);
    assert!(golden("posixJoin", two(posix::join)) > 2000);
}

#[test]
fn win32_paths_are_nodes() {
    assert!(golden("win32Normalize", one(win32::normalize)) > 2000);
    assert!(golden("win32Basename", one(|path| win32::basename(path).into())) > 2000);
    assert!(golden("win32Join", two(win32::join)) > 2000);
    let absolute = |input: &Value| win32::is_absolute(input.as_str().unwrap()).into();
    assert!(golden("win32Absolute", absolute) > 2000);
}
