use std::path::Path;

use super::{Failure, blob, cached, read};
use crate::rich_answer::js::{self, Js};

type Resolver = fn(&Path, &str) -> Result<std::path::PathBuf, &'static str>;

/// Runs `resolver` on every case of `function` and returns how many there were.
fn golden(function: &str, resolver: Resolver) -> usize {
    let all = include_str!("../../tests/fixtures/media-store/expected.json");
    let all = js::parse(all).unwrap();
    let cases = all.obj().unwrap()[function].arr().unwrap();
    let root = Path::new("/user-data");
    for case in cases {
        let [args, expected] = case.arr().unwrap() else {
            panic!("case shape")
        };
        let url = args.arr().unwrap()[0].str().unwrap();
        let found = match resolver(root, url) {
            Ok(path) => path
                .strip_prefix(root)
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned(),
            Err(refused) => format!("refused: {refused}"),
        };
        assert_eq!(&Js::Str(found), expected, "{function} {url}");
    }
    cases.len()
}

#[cfg(unix)]
#[test]
fn cached_images_resolve_as_cindys() {
    assert!(golden("cached", cached) > 50);
}

#[cfg(unix)]
#[test]
fn blobs_resolve_as_cindys() {
    assert!(golden("blob", blob) > 35);
}

#[test]
fn a_read_tells_missing_from_unreadable() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("cc-agent/images/s1");
    std::fs::create_dir_all(folder.join("folder.png")).unwrap();
    std::fs::write(folder.join("a.png"), b"bytes").unwrap();
    let read = |url: &str| read(dir.path(), url);
    assert_eq!(read("xdt-image://s1/a.png"), Ok(b"bytes".to_vec()));
    assert_eq!(read("xdt-image://s1/gone.png"), Err(Failure::Missing));
    assert!(matches!(
        read("xdt-image://s1/folder.png"),
        Err(Failure::Unavailable(_))
    ));
    let refused = Failure::Unavailable("xdt-image: path out of bounds".into());
    assert_eq!(read("xdt-image://s1/%2E%2E"), Err(refused));
    let hash = "ab".repeat(32);
    let blob = dir.path().join("cindy-media/blobs/ab");
    std::fs::create_dir_all(&blob).unwrap();
    std::fs::write(blob.join(format!("{hash}.png")), b"blob").unwrap();
    assert_eq!(
        read(&format!("cindy-media://blobs/{hash}.png")),
        Ok(b"blob".to_vec())
    );
    assert_eq!(
        read(&format!("cindy-media://blobs/{hash}.gif")),
        Err(Failure::Missing)
    );
    let invalid = Failure::Unavailable("cindy-media: invalid url".into());
    assert_eq!(read("cindy-media://client-wallpaper/x.png"), Err(invalid));
}
