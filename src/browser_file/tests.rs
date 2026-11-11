//! Cindy's own functions on crafted values and seeded mixes, under both platforms' rules.

use futures::FutureExt as _;
use gpui::TestAppContext;
use serde_json::Value;

use super::{Code, open, openable, target_on};
use crate::{open_external::tests as opener, open_path::tests as paths};

fn golden(name: &str) -> Vec<Value> {
    let all = include_str!("../../tests/fixtures/browser-file/expected.json");
    let all: Value = serde_json::from_str(all).unwrap();
    all[name].as_array().unwrap().clone()
}

fn targets(name: &str, windows: bool) -> usize {
    let (cases, mut bad) = (golden(name), 0);
    for case in &cases {
        let value = case[0].as_str().unwrap();
        // A Windows case names the current folder it ran in.
        let (cwd, want) = match windows {
            true => (case[1].as_str().unwrap(), &case[2]),
            false => ("/", &case[1]),
        };
        let got = match target_on(value, windows, cwd) {
            Err(()) => Value::from("threw"),
            Ok(None) => Value::Null,
            Ok(Some(target)) => serde_json::json!({
                "filePath": target.path, "fileUrl": target.url, "hasUrlState": false,
            }),
        };
        if got != *want {
            println!("DIFF {value:?} in {cwd:?}\n  got  {got}\n  want {want}");
            bad += 1;
        }
    }
    assert_eq!(bad, 0);
    cases.len()
}

#[test]
fn a_target_is_cindys_on_posix() {
    assert!(targets("posix", false) > 1500);
}

#[test]
fn a_target_is_cindys_on_windows() {
    assert!(targets("win32", true) > 1500);
}

#[test]
fn a_page_is_named_as_cindy_names_it() {
    let cases = golden("openable");
    for case in &cases {
        let name = case[0].as_str().unwrap();
        assert_eq!(openable(name), case[1].as_bool().unwrap(), "{name:?}");
    }
    assert_eq!(cases.len(), 20);
}

#[cfg(unix)]
#[gpui::test]
fn a_page_opens_by_its_url_then_by_its_path(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let page = dir.path().join("a b.html");
    std::fs::write(&page, "").unwrap();
    let path = page.to_str().unwrap().to_owned();
    let url = format!("file://{}", path.replace(' ', "%20"));
    let ask = |cx: &mut TestAppContext| {
        let opening = cx.update(|cx| open(&path, cx));
        cx.run_until_parked();
        opening.now_or_never().unwrap()
    };
    assert_eq!(ask(cx), Ok(()));
    assert_eq!(opener::take(), std::slice::from_ref(&url));
    assert!(paths::take().is_empty());
    // A refused URL falls back to the path; its error is the answer's.
    opener::fail(true);
    assert_eq!(ask(cx), Ok(()));
    assert_eq!(paths::take(), std::slice::from_ref(&page));
    paths::fail(Some("Failed to open path"));
    assert_eq!(
        ask(cx),
        Err((Code::OpenFailed, "Failed to open path".into()))
    );
    opener::fail(false);
    paths::fail(None);
    assert_eq!((opener::take().len(), paths::take().len()), (2, 1));
}

#[cfg(unix)]
#[gpui::test]
fn a_refused_page_answers_with_cindys_code(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let at = |name: &str| dir.path().join(name).to_str().unwrap().to_owned();
    std::fs::write(at("a.txt"), "").unwrap();
    let ask = |value: &str, cx: &mut TestAppContext| {
        let failure = cx.update(|cx| open(value, cx)).now_or_never().unwrap();
        failure.unwrap_err()
    };
    let invalid = "路径必须是绝对路径或本地 file:// URL";
    assert_eq!(ask("", cx), (Code::InvalidTarget, invalid.into()));
    assert_eq!(
        ask("https://example.test/index.html", cx).0,
        Code::InvalidTarget
    );
    assert_eq!(
        ask("/etc/ssh/a.html", cx),
        (Code::PathNotAllowed, "不允许访问该路径".into())
    );
    assert_eq!(
        ask(&at("a.txt"), cx),
        (Code::UnsupportedType, "该文件类型不支持浏览器查看".into())
    );
    assert_eq!(
        ask(&at("gone.html"), cx),
        (Code::NotFound, "文件不存在".into())
    );
    assert_eq!(Code::NotFound.name(), "BROWSER_FILE_NOT_FOUND");
    assert!(opener::take().is_empty() && paths::take().is_empty());
}
