use gpui::TestAppContext;
use serde_json::Value;

use super::{lit, resolved};
use crate::{
    local_path::smart::Smart,
    markdown_target::{Local, Place, Target},
    path_resolver::Kind,
};

fn local(kind: &Value) -> Local {
    match kind.as_str().unwrap() {
        "image" => Local::Image,
        "model" => Local::Model,
        "text" => Local::Text,
        "directory" => Local::Directory,
        other => panic!("kind {other}"),
    }
}

fn place(target: &Value) -> Place {
    let number = |name: &str| target[name].as_u64().map(|n| n as u32);
    Place {
        line: number("line"),
        column: number("column"),
    }
}

fn smart(result: &Value) -> Smart {
    let text = |name: &str| result[name].as_str().unwrap().to_owned();
    match result["status"].as_str().unwrap() {
        "unique" if result["kind"] == "directory" => {
            Smart::Unique(text("absPath"), Kind::Directory)
        }
        "unique" => Smart::Unique(text("absPath"), Kind::File),
        "multiple" => Smart::Multiple(Vec::new()),
        _ => Smart::None(text("fallbackAbsPath")),
    }
}

#[test]
fn a_unique_answer_lights_the_candidate_as_cindys() {
    let all = include_str!("../../../tests/fixtures/smart-paths/lit.json");
    let all: Vec<[Value; 3]> = serde_json::from_str(all).unwrap();
    for [target, result, expected] in &all {
        let candidate = Target::Candidate {
            href: target["href"].as_str().unwrap().into(),
            original: target["originalHref"].as_str().unwrap().into(),
            kind: local(&target["localKind"]),
            at: place(target),
        };
        let wanted = (!expected.is_null()).then(|| Target::Resolved {
            href: expected["href"].as_str().unwrap().into(),
            path: expected["absPath"].as_str().unwrap().into(),
            kind: local(&expected["localKind"]),
            at: place(expected),
        });
        assert_eq!(lit(&candidate, &smart(result)), wanted, "{target} {result}");
    }
    assert!(all.len() > 20);
}

#[cfg(unix)]
#[gpui::test]
fn a_candidate_lights_once_its_lookup_lands(cx: &mut TestAppContext) {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().to_str().unwrap().to_owned();
    std::fs::create_dir_all(root.path().join("pkg/src")).unwrap();
    std::fs::write(root.path().join("pkg/src/app.ts"), "").unwrap();
    let candidate = |href: &str| Target::Candidate {
        href: href.into(),
        original: format!("{href}:4"),
        kind: Local::Text,
        at: Place {
            line: Some(4),
            column: None,
        },
    };
    let read = |target: &Target, streaming: bool, cx: &mut TestAppContext| {
        cx.update(|cx| resolved(&format!("{target:?}"), target, &dir, streaming, cx))
    };
    let (file, folder, ghost) = (
        candidate("src/app.ts"),
        candidate("pkg"),
        candidate("no.ts"),
    );
    // A row still being written asks nothing.
    assert_eq!(read(&file, true, cx), None);
    cx.run_until_parked();
    assert_eq!(read(&file, true, cx), None);
    for target in [&file, &folder, &ghost] {
        assert_eq!(read(target, false, cx), None);
    }
    cx.run_until_parked();
    let lit = |href: &str, path: &str, kind| Target::Resolved {
        href: href.into(),
        path: format!("{dir}/{path}"),
        kind,
        at: Place {
            line: Some(4),
            column: None,
        },
    };
    let found = read(&file, false, cx);
    assert_eq!(
        found,
        Some(lit("src/app.ts", "pkg/src/app.ts", Local::Text))
    );
    let found = read(&folder, false, cx);
    assert_eq!(found, Some(lit("pkg", "pkg", Local::Directory)));
    assert_eq!(read(&ghost, false, cx), None);
    // What is no candidate stays as it is.
    let external = Target::External {
        href: "https://x.y".into(),
    };
    assert_eq!(read(&external, false, cx), None);
}

#[cfg(unix)]
#[gpui::test]
fn a_mounted_target_keeps_its_answer_past_the_cache(cx: &mut TestAppContext) {
    use crate::local_path::smart::{ask, peek};
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().to_str().unwrap().to_owned();
    std::fs::write(root.path().join("target.ts"), "").unwrap();
    let target = Target::Candidate {
        href: "target.ts".into(),
        original: "target.ts".into(),
        kind: Local::Text,
        at: Place::default(),
    };
    let draw = |id: &str, cx: &mut TestAppContext| {
        cx.update(|cx| {
            super::begin(cx);
            let found = resolved(id, &target, &dir, false, cx);
            super::end(cx);
            found
        })
    };
    assert_eq!(draw("a", cx), None);
    cx.run_until_parked();
    assert!(draw("a", cx).is_some());
    cx.update(|cx| (0..500).for_each(|i| ask(&format!("other-{i}.ts"), &dir, |_| {}, cx)));
    cx.run_until_parked();
    assert!(cx.update(|cx| peek("target.ts", &dir, cx).is_none()));
    // `logs/review/9.6h2-r1-review-differential.log`: the hook's state stays, and asks nothing.
    assert!(draw("a", cx).is_some());
    cx.run_until_parked();
    assert!(cx.update(|cx| peek("target.ts", &dir, cx).is_none()));
    // A target mounted anew reads the cache, which has let the answer go.
    assert_eq!(draw("b", cx), None);
    cx.run_until_parked();
    assert!(draw("b", cx).is_some());
    // An answer the cache dropped before its hook read it leaves the hook unlit.
    let many = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            super::begin(cx);
            let lit = (0..501).filter(|i| {
                let href = format!("m-{i}.ts");
                let target = Target::Candidate {
                    href: href.clone(),
                    original: href.clone(),
                    kind: Local::Text,
                    at: Place::default(),
                };
                resolved(&href, &target, &dir, false, cx).is_some()
            });
            let lit = lit.count();
            super::end(cx);
            lit
        })
    };
    for i in 0..501 {
        std::fs::write(root.path().join(format!("m-{i}.ts")), "").unwrap();
    }
    assert_eq!(many(cx), 0);
    cx.run_until_parked();
    for _ in 0..3 {
        assert_eq!(many(cx), 500);
        cx.run_until_parked();
    }
}

// `logs/review/9.6h2-r2-review-differential.log`: the hook reads when its own lookup lands.
#[cfg(unix)]
#[gpui::test]
fn a_hook_takes_its_answer_when_its_lookup_lands(cx: &mut TestAppContext) {
    use crate::local_path::smart::{ask, now, peek, recheck};
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().to_str().unwrap().to_owned();
    std::fs::write(root.path().join("target.ts"), "").unwrap();
    let candidate = |href: &str| Target::Candidate {
        href: href.into(),
        original: href.into(),
        kind: Local::Text,
        at: Place::default(),
    };
    let draw = |href: &str, cx: &mut TestAppContext| {
        cx.update(|cx| resolved(href, &candidate(href), &dir, false, cx))
    };
    // A later batch evicts the answer before the hook draws again.
    assert_eq!(draw("target.ts", cx), None);
    cx.run_until_parked();
    cx.update(|cx| (0..500).for_each(|i| ask(&format!("other-{i}.ts"), &dir, |_| {}, cx)));
    cx.run_until_parked();
    assert!(cx.update(|cx| peek("target.ts", &dir, cx).is_none()));
    assert!(draw("target.ts", cx).is_some());
    // A later recheck finds the file; the hook keeps the miss its own lookup gave it.
    assert_eq!(draw("created.ts", cx), None);
    cx.run_until_parked();
    std::fs::write(root.path().join("created.ts"), "").unwrap();
    assert!(cx.update(|cx| {
        let boundary = now(cx);
        recheck("created.ts", &dir, Some(boundary), cx)
    }));
    cx.run_until_parked();
    assert!(cx.update(|cx| {
        peek("created.ts", &dir, cx).is_some_and(|found| matches!(found, Smart::Unique(..)))
    }));
    assert_eq!(draw("created.ts", cx), None);
}
