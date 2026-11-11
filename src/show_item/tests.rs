//! The handler's checks, and the path the file manager is asked for.

use std::cell::RefCell;

use super::{MISSING, NO_TARGET, REFUSED, show};

thread_local! {
    static SHOWN: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

pub fn shown(path: String) {
    SHOWN.with_borrow_mut(|all| all.push(path));
}

/// The paths shown on this thread since the last call.
pub fn take() -> Vec<String> {
    SHOWN.take()
}

#[cfg(unix)]
#[test]
fn a_file_is_shown_at_its_normalized_path() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_str().unwrap();
    std::fs::write(dir.path().join("a.txt"), "").unwrap();
    assert_eq!(show(&format!("{root}/x/..//a.txt")), Ok(()));
    assert_eq!(take(), [format!("{root}/a.txt")]);
    // A folder is shown too, with the slash it came with.
    assert_eq!(show(&format!("{root}/")), Ok(()));
    assert_eq!(take(), [format!("{root}/")]);
}

#[cfg(unix)]
#[test]
fn refused_paths_answer_with_cindys_errors() {
    let dir = tempfile::tempdir().unwrap();
    let gone = dir.path().join("gone.txt");
    assert_eq!(show(""), Err(NO_TARGET.into()));
    assert_eq!(show("work/a.txt"), Err(NO_TARGET.into()));
    assert_eq!(show("/tmp/../etc/ssh/x"), Err(REFUSED.into()));
    assert_eq!(show(gone.to_str().unwrap()), Err(MISSING.into()));
    // A link whose file is gone does not exist, as `fs.existsSync` follows it.
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&gone, &link).unwrap();
    assert_eq!(show(link.to_str().unwrap()), Err(MISSING.into()));
    assert!(take().is_empty());
}
