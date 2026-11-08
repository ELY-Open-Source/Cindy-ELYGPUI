//! Cindy's `appSessionState.ts`, read only: whose data this window shows.

use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};

const FILE: &str = "app-session.json";

/// `LOCAL_PROFILE_DATA_OWNER_ID`.
pub const LOCAL_OWNER: &str = "local-v1";

/// `AppSessionMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    SignedOut,
    Local,
    Cloud,
}

impl Mode {
    fn parse(value: Option<&str>) -> Self {
        match value {
            Some("local") => Self::Local,
            Some("cloud") => Self::Cloud,
            _ => Self::SignedOut,
        }
    }
}

/// The active owner, when this chapter can name one.
pub struct Owner {
    pub id: &'static str,
    pub dir: PathBuf,
}

/// `dataOwnerStorageKey`: the first 20 hex characters of the id's SHA-256.
pub fn storage_key(owner_id: &str) -> String {
    let digest = format!("{:x}", Sha256::digest(owner_id.as_bytes()));
    digest[..20].to_owned()
}

/// The `app session` override file as `createOverrideSettingsFile` reads it: a file that does
/// not parse is logged and removed, since this store does not keep unreadable files.
pub fn read_mode(user_data: &Path) -> Mode {
    let path = user_data.join(FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Mode::SignedOut,
        Err(error) => return unreadable(&path, &error.to_string()),
    };
    let parsed = match crate::cindy_json::roundtrip(&text) {
        Ok(parsed) => parsed,
        Err(error) => return unreadable(&path, &error.to_string()),
    };
    let mode = Mode::parse(parsed.get("activeMode").and_then(|m| m.as_str()));
    log::info!(target: "appSessionState", "app session settings loaded: activeMode {mode:?}, path {}", path.display());
    mode
}

fn unreadable(path: &Path, error: &str) -> Mode {
    log::warn!(target: "appSessionState", "app session settings read failed; falling back to defaults: {error}, path {}", path.display());
    if let Err(error) = std::fs::remove_file(path) {
        log::warn!(target: "appSessionState", "unreadable app session settings not removed: {error}");
    }
    Mode::SignedOut
}

/// The owner this chapter opens: only the account-free one needs no sign-in.
pub fn owner(mode: Mode, user_data: &Path) -> Option<Owner> {
    match mode {
        Mode::Local => Some(Owner {
            id: LOCAL_OWNER,
            dir: user_data.join("owners").join(storage_key(LOCAL_OWNER)),
        }),
        Mode::SignedOut | Mode::Cloud => {
            log::info!(target: "appSessionState", "{mode:?}: the account and its data arrive with chapter 13");
            None
        }
    }
}

#[cfg(test)]
mod tests;
