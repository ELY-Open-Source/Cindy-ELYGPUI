use super::*;
use crate::rich_answer::js;

#[test]
fn artifacts_match_cindys() {
    let golden = crate::command_output::tests::golden();
    let cases = golden.obj().unwrap()["withHistoryArtifacts"].arr().unwrap();
    assert!(cases.len() > 10);
    for case in cases {
        let row = case.arr().unwrap()[0].arr().unwrap()[0].clone();
        let shown = js::stringify(&row, "");
        let got = with_artifacts(row);
        assert_eq!(
            js::stringify(&got, ""),
            js::stringify(&case.arr().unwrap()[1], ""),
            "{shown:.400}"
        );
    }
}

#[test]
fn the_cache_keys_on_the_row_and_its_command() {
    let paths = |id: &str, command: &str| command_paths(id, command);
    let command = format!("echo '{}' > /work/cached-report.txt", "x".repeat(2000));
    assert_eq!(paths("cache-a", &command), ["/work/cached-report.txt"]);
    let key = cache_key("cache-a", &command);
    assert!(CACHE.lock().unwrap().contains_key(&key));
    assert_eq!(
        paths("cache-a", "echo ok > /work/other.txt"),
        ["/work/other.txt"]
    );
    assert_eq!(
        cache_key("a", "😀"),
        "a\u{0}2\u{0}2147429447".replace("2147429447", &{
            let mut h = 2_166_136_261u32;
            for u in "😀".encode_utf16() {
                h = (h ^ u32::from(u)).wrapping_mul(16_777_619);
            }
            h.to_string()
        })
    );
}

#[test]
fn heads_and_spreads_count_utf16_units() {
    assert_eq!(head(&"a".repeat(300)).len(), 256);
    let cut = format!("{}😀", "a".repeat(255));
    assert_eq!(head(&cut), format!("{}\u{FFFD}", "a".repeat(255)));
    let spread = spread(&Js::Str("a😀".into()));
    assert_eq!(
        js::stringify(&Js::Obj(spread), ""),
        "{\"0\":\"a\",\"1\":\"\u{FFFD}\",\"2\":\"\u{FFFD}\"}"
    );
}

#[test]
fn the_cache_drops_its_least_recently_used() {
    let mut cache = IndexMap::new();
    let miss = |n: usize| move || vec![n.to_string()];
    for n in 0..CACHE_LIMIT {
        cached(&mut cache, n.to_string(), miss(n));
    }
    assert_eq!(cached(&mut cache, "0".into(), || unreachable!()), ["0"]);
    cached(&mut cache, "new".into(), miss(9));
    assert_eq!(cache.len(), CACHE_LIMIT);
    assert!(cache.contains_key("0") && !cache.contains_key("1"));
    assert_eq!(cache.keys().last().map(String::as_str), Some("new"));
}
