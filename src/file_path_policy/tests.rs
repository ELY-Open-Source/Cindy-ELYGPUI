//! The system blocklist's roots and the boundary rules `isPathAllowedAgainst` keeps.

use super::{allowed_against, strip_namespace, system_blocklist, win32_blocklist};

#[cfg(unix)]
#[test]
fn posix_roots_block_themselves_and_what_lies_inside() {
    use super::allowed;
    for path in [
        "/etc",
        "/etc/",
        "/etc/passwd",
        "/proc/1",
        "/tmp/../etc/x",
        "/var/log",
        "//dev",
    ] {
        assert!(!allowed(path), "{path}");
    }
    for path in ["/etcetera", "/var/logs", "/var/tmp", "/home/u/work", "/"] {
        assert!(allowed(path), "{path}");
    }
    assert!(!allowed(""), "empty");
    assert!(!allowed("etc/passwd"), "relative");
}

#[cfg(unix)]
#[test]
fn a_root_with_a_trailing_slash_or_dots_still_matches() {
    let list = vec!["/data/foo/".to_owned(), "/srv/./x".to_owned()];
    assert!(!allowed_against("/data/foo", &list));
    assert!(!allowed_against("/data/foo/bar", &list));
    assert!(allowed_against("/data/foobar", &list));
    assert!(!allowed_against("/srv/x/y", &list));
}

#[test]
fn the_win32_roots_follow_the_env_one_per_spelling() {
    let env = |key: &str| match key {
        "SystemRoot" => Some("D:\\WINDOWS".to_owned()),
        "ProgramFiles" => Some("C:\\Program Files".to_owned()),
        "ProgramFiles(x86)" => Some("c:\\program files".to_owned()),
        _ => None,
    };
    assert_eq!(
        win32_blocklist(env),
        ["D:\\WINDOWS", "C:\\Program Files", "C:\\ProgramData"]
    );
    assert_eq!(
        system_blocklist(true, |_| None).len(),
        4,
        "C-drive defaults"
    );
    assert_eq!(system_blocklist(false, |_| None)[0], "/etc");
}

#[test]
fn namespace_prefixes_come_off() {
    assert_eq!(strip_namespace("\\\\?\\C:\\Windows"), "C:\\Windows");
    assert_eq!(strip_namespace("\\\\.\\C:\\Windows"), "C:\\Windows");
    assert_eq!(strip_namespace("\\\\?\\unc\\srv\\share"), "\\\\srv\\share");
    assert_eq!(strip_namespace("C:\\x"), "C:\\x");
}

#[cfg(windows)]
#[test]
fn a_share_root_matches_without_its_trailing_separator() {
    let list = vec!["\\\\srv\\share".to_owned(), "C:\\Windows".to_owned()];
    assert!(!allowed_against("\\\\srv\\share\\x", &list));
    assert!(!allowed_against("\\\\?\\c:\\windows\\system32", &list));
    assert!(allowed_against("C:\\Windows123", &list));
}

#[test]
fn the_sensitive_list_is_cindys_on_every_platform() {
    use super::sensitive::{Host, Platform, blocklist};
    use serde_json::Value;

    let cases = include_str!("../../tests/fixtures/file-policy/expected.json");
    let cases: Vec<(Value, Vec<String>)> = serde_json::from_str(cases).unwrap();
    assert!(cases.len() > 700);
    for (host, expected) in cases {
        let text = |key: &str| host[key].as_str().unwrap().to_owned();
        let platform = match text("platform").as_str() {
            "win32" => Platform::Win32,
            "darwin" => Platform::Darwin,
            _ => Platform::Other,
        };
        let sep = if platform == Platform::Win32 {
            "\\"
        } else {
            "/"
        };
        let under = |root: &str, at: &str| {
            let inside = match root.ends_with(sep) {
                true => root.to_owned(),
                false => format!("{root}{sep}"),
            };
            at == root || at.starts_with(&inside)
        };
        // The recorder's file system of words.
        let real = |value: &str| {
            let links = host["links"].as_array().unwrap();
            let pair = |link: &Value| {
                (
                    link[0].as_str().unwrap().to_owned(),
                    link[1].as_str().unwrap().to_owned(),
                )
            };
            let link = links.iter().map(pair).find(|(from, _)| under(from, value));
            let at = match link {
                Some((from, to)) => format!("{to}{}", &value[from.len()..]),
                None => value.to_owned(),
            };
            let exists = host["exists"].as_array().unwrap();
            exists
                .iter()
                .any(|root| under(root.as_str().unwrap(), &at))
                .then_some(at)
        };
        let env = |key: &str| host["env"][key].as_str().map(str::to_owned);
        let (home, cwd) = (text("home"), text("cwd"));
        let list = blocklist(&Host {
            platform,
            home: &home,
            cwd: &cwd,
            env: &env,
            real: &real,
        });
        assert_eq!(list, expected, "{host}");
    }
}
