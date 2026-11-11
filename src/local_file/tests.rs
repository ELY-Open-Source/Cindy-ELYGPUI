//! Each of the handler's refusals, on real files.

use super::{Served, read};
use crate::local_path::shape::image_src;

/// The address the renderer asks for an absolute path.
fn url(path: &str) -> String {
    image_src(path, "", true).expect("an absolute path has an address")
}

#[cfg(unix)]
#[test]
fn a_media_file_is_served_and_the_rest_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let dir = dir.path().canonicalize().unwrap();
    let at = |name: &str| dir.join(name).to_string_lossy().into_owned();
    std::fs::write(at("a 100% b.PNG"), b"png").unwrap();
    std::fs::write(at("notes.txt"), b"text").unwrap();
    std::fs::write(at("draw.svg"), b"<svg/>").unwrap();
    std::fs::create_dir(at("folder.png")).unwrap();
    std::os::unix::fs::symlink(at("a 100% b.PNG"), at("link.png")).unwrap();
    std::os::unix::fs::symlink("/etc/passwd", at("secret.png")).unwrap();
    std::os::unix::fs::symlink(at("loop.png"), at("loop.png")).unwrap();

    let served = |bytes: &[u8], mime| {
        Ok(Served {
            bytes: bytes.to_vec(),
            mime,
        })
    };
    assert_eq!(read(&url(&at("a 100% b.PNG"))), served(b"png", "image/png"));
    assert_eq!(
        read(&url(&at("draw.svg"))),
        served(b"<svg/>", "image/svg+xml")
    );
    assert_eq!(read(&url(&at("link.png"))), served(b"png", "image/png"));
    assert_eq!(
        read(&url(&at("x/../draw.svg"))),
        served(b"<svg/>", "image/svg+xml")
    );

    assert_eq!(read("xdt-file://local/"), Err(400));
    assert_eq!(read("xdt-file://local/?path="), Err(400));
    assert_eq!(read("xdt-file://local/?other=%2Fa.png"), Err(400));
    assert_eq!(read("xdt-file://local/?path=a.png"), Err(403));
    assert_eq!(read("xdt-file://local/?path=C%3A%5Ca.png"), Err(403));
    assert_eq!(read(&url(&at("draw.svg/"))), Err(403));
    assert_eq!(read(&url(&at("notes.txt"))), Err(415));
    assert_eq!(read(&url(&at("none"))), Err(415));
    assert_eq!(read(&url("/etc/shadow.png")), Err(403));
    assert_eq!(read(&url(&at("secret.png"))), Err(403));
    assert_eq!(read(&url(&at("missing.png"))), Err(404));
    assert_eq!(read(&url(&at("notes.txt/a.png"))), Err(404));
    assert_eq!(read(&url(&at("loop.png"))), Err(404));
    assert_eq!(read(&url(&at("folder.png"))), Err(404));
    assert_eq!(read("not a url"), Err(500));
}
