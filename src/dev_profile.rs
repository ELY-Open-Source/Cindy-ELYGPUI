//! Cindy's `devCliFlags.ts`: where a source run keeps its data.

mod canonical;

use std::path::{Path, PathBuf};

/// `OFFICIAL_USER_DATA_DIR_NAMES`.
const OFFICIAL: [&str; 3] = ["Cindy", "CindyGlobal", "CindyDev"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    ProductionShared,
    IsolatedSandbox,
    Custom,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Self::ProductionShared => "production-shared",
            Self::IsolatedSandbox => "isolated-sandbox",
            Self::Custom => "custom",
        }
    }
}

pub struct Input<'a> {
    pub args: &'a [String],
    pub env: &'a dyn Fn(&str) -> Option<String>,
    pub default_dir: &'a Path,
    pub app_data_dir: &'a Path,
}

#[derive(Debug, PartialEq)]
pub struct Profile {
    pub invalid_name: Option<String>,
    pub dir_override: Option<PathBuf>,
    pub kind: Kind,
    pub isolated_on_production: bool,
}

/// `ISOLATION_NAME_RE`: `[A-Za-z0-9_-]{1,32}`.
fn valid_name(name: &str) -> bool {
    (1..=32).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// `resolveDevCliFlags` for a source run, with `canonicalize` as the path judge.
pub fn resolve(input: &Input, canonicalize: &dyn Fn(&Path) -> PathBuf) -> Profile {
    let (mut isolated, mut name, mut invalid_name) = (false, None, None);
    let mut take = |raw: String| {
        if valid_name(&raw) {
            name = Some(raw);
        } else {
            invalid_name = Some(raw);
        }
    };
    for arg in input.args {
        if arg == "--isolated" {
            isolated = true;
        } else if let Some(raw) = arg.strip_prefix("--isolated=") {
            isolated = true;
            take(raw.to_owned());
        }
    }
    let env = input.env;
    if !isolated && env("XDT_ISOLATED").as_deref() == Some("1") {
        isolated = true;
        if let Some(raw) = env("XDT_ISOLATED_NAME").filter(|n| !n.trim().is_empty()) {
            take(raw.trim().to_owned());
        }
    }
    let env_dir = env("XDT_USER_DATA_DIR")
        .filter(|d| !d.trim().is_empty())
        .map(PathBuf::from);
    let suffix = name.as_ref().map_or(String::new(), |n| format!("-{n}"));
    let mut sandbox = input.default_dir.as_os_str().to_owned();
    sandbox.push(format!("-dev2{suffix}"));
    let sandbox = PathBuf::from(sandbox);
    let dir_override = env_dir
        .clone()
        .or_else(|| isolated.then(|| sandbox.clone()));
    let trusted = env_dir.is_none() || env("XDT_USER_DATA_DIR_EPOCH").as_deref() == Some("1");
    let epoch_derived = isolated
        && trusted
        && dir_override
            .as_deref()
            .is_some_and(|d| canonicalize(d) == canonicalize(&sandbox));
    let effective = dir_override.as_deref().unwrap_or(input.default_dir);
    let target = canonicalize(effective);
    let official = OFFICIAL
        .iter()
        .any(|dir| canonicalize(&input.app_data_dir.join(dir)) == target);
    let kind = if official {
        Kind::ProductionShared
    } else if epoch_derived {
        Kind::IsolatedSandbox
    } else {
        Kind::Custom
    };
    Profile {
        isolated_on_production: isolated && official,
        invalid_name,
        dir_override,
        kind,
    }
}

/// The userData of this run, as `index.ts` sets it before anything reads it.
pub fn user_data_dir(region_dir: PathBuf) -> anyhow::Result<PathBuf> {
    if !cfg!(debug_assertions) {
        return Ok(region_dir);
    }
    let app_data = region_dir
        .parent()
        .expect("a region folder has a parent")
        .to_owned();
    let args: Vec<String> = std::env::args().collect();
    let env = |key: &str| std::env::var(key).ok();
    let input = Input {
        args: &args,
        env: &env,
        default_dir: &region_dir,
        app_data_dir: &app_data,
    };
    let profile = resolve(&input, &canonical::path);
    if let Some(name) = &profile.invalid_name {
        eprintln!(
            "[cindy] WARN: invalid --isolated name \"{name}\" (allowed: A-Za-z0-9_-, max 32 chars); falling back to the DEFAULT sandbox"
        );
    }
    let dir = profile.dir_override.clone().unwrap_or(region_dir);
    if profile.isolated_on_production {
        eprintln!(
            "[cindy] FATAL: --isolated cannot use the official Cindy profile ({}). Use the default sandbox, --isolated=<name>, or a directory that is not an official userData.",
            dir.display()
        );
        std::process::exit(1);
    }
    if profile.dir_override.is_some() {
        eprintln!("[cindy] dev userData override → {}", dir.display());
    }
    eprintln!("[cindy] dev profile {}", profile.kind.label());
    Ok(dir)
}

#[cfg(test)]
mod tests;
