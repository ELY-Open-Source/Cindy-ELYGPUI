use std::time::{Duration, Instant};

use super::{Resolved, cache::Cache};

/// The golden's cases, each on a tree of its own; its paths are POSIX ones.
#[cfg(unix)]
mod golden {
    use std::{collections::HashMap, fs, path::Path};

    use serde_json::{Value, json};

    use crate::path_resolver::{
        Kind, Options, Resolved,
        cache::{all_cached, cached},
        decode, resolve, resolve_all,
        walk::{find, find_many},
    };

    fn shown(result: &Resolved) -> Value {
        let (status, candidates, kind) = match result {
            Resolved::None => ("none", Vec::new(), Value::Null),
            Resolved::Unique(path, Kind::File) => ("unique", vec![path.clone()], "file".into()),
            Resolved::Unique(path, Kind::Directory) => {
                ("unique", vec![path.clone()], "directory".into())
            }
            Resolved::Multiple(all) => ("multiple", all.clone(), Value::Null),
        };
        json!({ "status": status, "candidates": candidates, "kind": kind })
    }

    fn put(root: &Path, file: &str) {
        let path = root.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "").unwrap();
    }

    /// A step's hrefs, each once.
    fn hrefs(list: &Value, root: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for href in list.as_array().unwrap() {
            let href = href.as_str().unwrap().replace("<root>", root);
            if !out.contains(&href) {
                out.push(href);
            }
        }
        out
    }

    /// One step of a case, on the tree under `root`, as the golden's script runs it.
    fn run(step: &Value, root: &Path) -> Value {
        let at = root.to_str().unwrap();
        let real = |text: &Value| text.as_str().unwrap().replace("<root>", at);
        let blocked = match &step["blocked"] {
            Value::String(_) => None,
            Value::Array(list) => Some(list.iter().map(real).collect()),
            _ => Some(Vec::new()),
        };
        let allowed = |path: &str| {
            blocked
                .as_ref()
                .is_some_and(|list| !list.iter().any(|b| b == path))
        };
        let number = |name: &str, default| step[name].as_u64().map_or(default, |n| n as usize);
        let home = format!("{at}/home");
        let options = Options {
            allowed: &allowed,
            max_candidates: number("max", 10),
            max_depth: number("depth", 8),
            home: &home,
        };
        let dir = match &step["dir"] {
            Value::Null => format!("{at}/workspace"),
            dir => real(dir),
        };
        let each = |list: &[String], results: HashMap<String, Resolved>| {
            Value::Array(
                list.iter()
                    .map(|href| json!([href, shown(&results[href])]))
                    .collect(),
            )
        };
        if let Some(file) = step["write"].as_str() {
            put(root, file);
        } else if let Some(file) = step["remove"].as_str() {
            fs::remove_file(root.join(file)).unwrap();
        } else if let Some(link) = step["link"].as_array() {
            let (path, target) = (link[0].as_str().unwrap(), link[1].as_str().unwrap());
            std::os::unix::fs::symlink(target, root.join(path)).unwrap();
        }
        if !step["find"].is_null() {
            json!(find(&dir, &real(&step["find"]), &options))
        } else if !step["many"].is_null() {
            let list = hrefs(&step["many"], at);
            let wanted: Vec<&str> = list.iter().map(String::as_str).collect();
            let mut found = find_many(&dir, &wanted, &options);
            let pairs = list
                .iter()
                .map(|href| json!([href, found.remove(href).unwrap()]));
            Value::Array(pairs.collect())
        } else if let Some(value) = step["decode"].as_str() {
            json!(decode(value))
        } else if !step["resolve"].is_null() {
            shown(&resolve(&real(&step["resolve"]), &dir, &options))
        } else if !step["batch"].is_null() {
            let list = hrefs(&step["batch"], at);
            each(&list, resolve_all(&list, &dir, &options))
        } else if !step["cached"].is_null() {
            let refresh = step["refresh"] == true;
            shown(&cached(&real(&step["cached"]), &dir, refresh, &options))
        } else if !step["batchCached"].is_null() {
            let list = hrefs(&step["batchCached"], at);
            each(
                &list,
                all_cached(&list, &dir, step["revalidate"] == true, &options),
            )
        } else {
            Value::Null
        }
    }

    #[test]
    fn every_step_answers_as_cindys() {
        let _serial = crate::path_resolver::cache::serial();
        let all = include_str!("../../tests/fixtures/path-resolver/expected.json");
        let all: Vec<Value> = serde_json::from_str(all).unwrap();
        let mut steps = 0;
        for case in &all {
            let root = tempfile::tempdir().unwrap();
            let at = root.path().to_str().unwrap();
            for folder in ["workspace", "home"] {
                fs::create_dir_all(root.path().join(folder)).unwrap();
            }
            for file in case["files"].as_array().unwrap() {
                put(root.path(), file.as_str().unwrap());
            }
            let expected = case["out"].to_string().replace("<root>", at);
            let expected: Vec<Value> = serde_json::from_str(&expected).unwrap();
            for (step, expected) in case["steps"].as_array().unwrap().iter().zip(&expected) {
                assert_eq!(&run(step, root.path()), expected, "{} {step}", case["name"]);
                steps += 1;
            }
        }
        assert!(steps > 180, "{steps}");
    }
}

#[test]
fn an_answer_lives_thirty_seconds() {
    let (mut cache, now) = (Cache::default(), Instant::now());
    cache.put("a".into(), Resolved::None, now);
    let late = now + Duration::from_millis(29_999);
    assert_eq!(cache.take("a", late, |_| true), Some(Resolved::None));
    assert_eq!(
        cache.take("a", now + Duration::from_secs(30), |_| true),
        None
    );
    // The stale answer went with that read.
    assert_eq!(cache.take("a", now, |_| true), None);
}

#[test]
fn a_refused_answer_is_dropped() {
    let (mut cache, now) = (Cache::default(), Instant::now());
    cache.put("a".into(), Resolved::None, now);
    assert_eq!(cache.take("a", now, |_| false), None);
    assert_eq!(cache.take("a", now, |_| true), None);
}

#[test]
fn the_oldest_answer_leaves_a_full_cache() {
    let (mut cache, now) = (Cache::default(), Instant::now());
    for n in 0..200 {
        cache.put(format!("k{n}"), Resolved::None, now);
    }
    // A key kept again holds its place, and the oldest still goes for it.
    cache.put("k5".into(), Resolved::Multiple(Vec::new()), now);
    assert_eq!(cache.take("k0", now, |_| true), None);
    assert_eq!(
        cache.take("k5", now, |_| true),
        Some(Resolved::Multiple(Vec::new()))
    );
    cache.put("new".into(), Resolved::None, now);
    assert_eq!(cache.take("k1", now, |_| true), Some(Resolved::None));
    cache.put("newer".into(), Resolved::None, now);
    assert_eq!(cache.take("k1", now, |_| true), None);
    assert_eq!(cache.take("k2", now, |_| true), Some(Resolved::None));
}
