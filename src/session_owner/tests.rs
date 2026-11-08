use super::{LOCAL_OWNER, Mode, owner, read_mode, storage_key};

/// From node: `sha256('local-v1').digest('hex').slice(0, 20)`.
#[test]
fn the_storage_key_is_cindys() {
    assert_eq!(storage_key(LOCAL_OWNER), "8abfb6a6ce34715bea84");
}

#[test]
fn reads_the_mode_as_the_store_does() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("app-session.json");
    assert_eq!(read_mode(dir.path()), Mode::SignedOut, "missing");
    for (text, mode) in [
        (r#"{"activeMode":"local"}"#, Mode::Local),
        (r#"{"activeMode":"cloud"}"#, Mode::Cloud),
        (r#"{"activeMode":"Local"}"#, Mode::SignedOut),
        (r#"{"activeMode":1}"#, Mode::SignedOut),
        ("[1]", Mode::SignedOut),
        (r#"{"activeMode":"local","future":1e400}"#, Mode::Local),
    ] {
        std::fs::write(&file, text).unwrap();
        assert_eq!(read_mode(dir.path()), mode, "{text}");
        assert!(file.exists(), "a parsed file stays: {text}");
    }
    std::fs::write(&file, "{not json").unwrap();
    assert_eq!(read_mode(dir.path()), Mode::SignedOut);
    assert!(!file.exists(), "this store removes a file it cannot parse");
}

#[test]
fn only_the_account_free_owner_opens_here() {
    let dir = tempfile::tempdir().unwrap();
    let local = owner(Mode::Local, dir.path()).expect("local owner");
    assert_eq!(local.id, "local-v1");
    assert_eq!(local.dir, dir.path().join("owners/8abfb6a6ce34715bea84"));
    assert!(owner(Mode::Cloud, dir.path()).is_none());
    assert!(owner(Mode::SignedOut, dir.path()).is_none());
}
