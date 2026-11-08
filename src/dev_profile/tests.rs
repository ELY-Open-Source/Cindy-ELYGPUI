//! Cases from Cindy's `devCliFlags.test.ts`.

use std::path::{Path, PathBuf};

use super::{Input, Kind, Profile, resolve};

const APP_DATA: &str = "/home/u/.config";
const DEFAULT: &str = "/home/u/.config/CindyGlobal";

fn plain(p: &Path) -> PathBuf {
    p.to_owned()
}

fn run(args: &[&str], env: &[(&str, &str)]) -> Profile {
    let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
    let lookup = |key: &str| {
        env.iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| (*v).to_owned())
    };
    let input = Input {
        args: &args,
        env: &lookup,
        default_dir: Path::new(DEFAULT),
        app_data_dir: Path::new(APP_DATA),
    };
    resolve(&input, &plain)
}

fn dir(profile: &Profile) -> Option<&str> {
    profile.dir_override.as_deref().and_then(Path::to_str)
}

#[test]
fn no_flags_keep_the_official_profile() {
    let profile = run(&[], &[]);
    assert_eq!(
        (dir(&profile), profile.kind),
        (None, Kind::ProductionShared)
    );
}

#[test]
fn isolated_runs_in_a_dev2_sandbox() {
    let default = run(&["--isolated"], &[]);
    assert_eq!(dir(&default), Some("/home/u/.config/CindyGlobal-dev2"));
    assert_eq!(default.kind, Kind::IsolatedSandbox);
    let named = run(&["--isolated=ui-mock"], &[]);
    assert_eq!(
        dir(&named),
        Some("/home/u/.config/CindyGlobal-dev2-ui-mock")
    );
    let bad = run(&["--isolated=bad name"], &[]);
    assert_eq!(
        dir(&bad),
        Some("/home/u/.config/CindyGlobal-dev2"),
        "not unisolated"
    );
    assert_eq!(bad.invalid_name.as_deref(), Some("bad name"));
    let long = "a".repeat(33);
    assert!(
        run(&[&format!("--isolated={long}")], &[])
            .invalid_name
            .is_some()
    );
}

#[test]
fn the_env_matches_the_flags_and_the_flags_win() {
    let env = run(&[], &[("XDT_ISOLATED", "1"), ("XDT_ISOLATED_NAME", "1")]);
    assert_eq!(
        dir(&env),
        Some("/home/u/.config/CindyGlobal-dev2-1"),
        "a sandbox named 1"
    );
    for off in ["0", "true", "yes"] {
        assert_eq!(dir(&run(&[], &[("XDT_ISOLATED", off)])), None, "{off}");
    }
    let both = run(
        &["--isolated=a"],
        &[("XDT_ISOLATED", "1"), ("XDT_ISOLATED_NAME", "b")],
    );
    assert_eq!(dir(&both), Some("/home/u/.config/CindyGlobal-dev2-a"));
    let bad = run(&[], &[("XDT_ISOLATED", "1"), ("XDT_ISOLATED_NAME", "x/y")]);
    assert_eq!(bad.invalid_name.as_deref(), Some("x/y"));
}

#[test]
fn an_explicit_dir_wins_and_needs_the_epoch_to_count_as_a_sandbox() {
    let blank = run(&["--isolated"], &[("XDT_USER_DATA_DIR", "  ")]);
    assert_eq!(dir(&blank), Some("/home/u/.config/CindyGlobal-dev2"));
    let custom = run(&["--isolated"], &[("XDT_USER_DATA_DIR", "/tmp/c")]);
    assert_eq!((dir(&custom), custom.kind), (Some("/tmp/c"), Kind::Custom));
    let sandbox = [("XDT_USER_DATA_DIR", "/home/u/.config/CindyGlobal-dev2")];
    assert_eq!(
        run(&["--isolated"], &sandbox).kind,
        Kind::Custom,
        "untrusted without the epoch"
    );
    let epoch = [sandbox[0], ("XDT_USER_DATA_DIR_EPOCH", "1")];
    assert_eq!(run(&["--isolated"], &epoch).kind, Kind::IsolatedSandbox);
    let only_dir = run(&[], &[("XDT_USER_DATA_DIR", "/tmp/c")]);
    assert_eq!(only_dir.kind, Kind::Custom);
}

#[test]
fn isolated_refuses_every_official_profile() {
    for official in ["CindyGlobal", "Cindy", "CindyDev"] {
        let path = format!("{APP_DATA}/{official}");
        let profile = run(&["--isolated"], &[("XDT_USER_DATA_DIR", &path)]);
        assert!(profile.isolated_on_production, "{official}");
        assert_eq!(profile.kind, Kind::ProductionShared);
        let shared = run(&[], &[("XDT_USER_DATA_DIR", &path)]);
        assert!(!shared.isolated_on_production);
        assert_eq!(shared.kind, Kind::ProductionShared);
    }
}

#[test]
fn the_real_judge_follows_the_file_system() {
    let root = tempfile::tempdir().unwrap();
    let real = root.path().join("Profile");
    std::fs::create_dir(&real).unwrap();
    let judge = super::canonical::path;
    let dotted = real.join(".");
    assert_eq!(judge(&dotted), judge(&real), "a `.` segment");
    let fresh = real.join("not-yet").join("deeper");
    assert_eq!(judge(&fresh), judge(&real).join("not-yet").join("deeper"));
    let sandbox = root.path().join("Cindy-dev2");
    for spelling in [
        "other/../Cindy-dev2",
        "./Cindy-dev2",
        "Cindy-dev2/",
        "a/b/../../Cindy-dev2",
    ] {
        let written = PathBuf::from(format!("{}/{spelling}", root.path().display()));
        assert_eq!(judge(&written), judge(&sandbox), "missing {spelling}");
    }
    let other_case = root.path().join("profile");
    let insensitive = std::fs::metadata(&other_case).is_ok();
    assert_eq!(judge(&other_case) == judge(&real), insensitive);
}
