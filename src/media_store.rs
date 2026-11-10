//! Where Cindy's managed images live: `imageCacheStore.ts`'s and `blobStore.ts`'s `resolveSafe`.

use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
};

use url::Url;

use crate::{cindy_json::decode_uri_component, dev_profile::canonical::resolve};

/// `RESERVED_HOSTS`: an integration's folder under `cc-agent`.
const RESERVED: &[(&str, &str)] = &[
    ("feishu-media-images", "feishu-media/images"),
    ("feishu-media-files", "feishu-media/files"),
    ("lizi-art-media-images", "lizi-art-media/images"),
    ("lizi-confluence-attachments", "lizi-confluence/attachments"),
    ("lizi-jira-attachments", "lizi-jira/attachments"),
];

/// `blobStore.ts`'s `MIME_BY_EXT` keys.
const BLOB_EXTS: &[&str] = &[
    ".png", ".jpg", ".jpeg", ".gif", ".webp", ".mp4", ".webm", ".mov", ".mp3", ".wav", ".m4a",
    ".ogg", ".glb",
];

/// Why an address names no file; the words are Cindy's.
type Refused = &'static str;

/// `imageCacheStore.resolveSafe` for `xdt-image://<host>/<filename>`.
fn cached(user_data: &Path, url: &str) -> Result<PathBuf, Refused> {
    if !url.starts_with("xdt-image://") {
        return Err("xdt-image: invalid url");
    }
    let malformed = "xdt-image: malformed url";
    let parsed = Url::parse(url).map_err(|_| malformed)?;
    let host = decode_uri_component(parsed.host_str().unwrap_or_default()).ok_or(malformed)?;
    let path = parsed.path();
    let name = decode_uri_component(path.strip_prefix('/').unwrap_or(path)).ok_or(malformed)?;
    let outside = "xdt-image: path out of bounds";
    let bad = |part: &str| part.is_empty() || part.contains("..") || part.contains('\0');
    if bad(&host) || bad(&name) || name.contains(['/', '\\']) {
        return Err(outside);
    }
    let agent = user_data.join("cc-agent");
    let within = |base: PathBuf, path: PathBuf| {
        let (base, path) = (resolve(&base), resolve(&path));
        // A name is never `.`: the URL parser folds it away.
        path.starts_with(&base).then_some(path).ok_or(outside)
    };
    if let Some((_, dir)) = RESERVED.iter().find(|(reserved, _)| *reserved == host) {
        let base = agent.join(dir);
        return within(base.clone(), base.join(&name));
    }
    // `assertSafeSessionId`.
    if host.contains(['/', '\\', ':']) {
        return Err(outside);
    }
    let root = agent.join("images");
    within(root.clone(), root.join(&host).join(&name))
}

/// `blobStore.resolveSafe`: only `cindy-media://blobs/<sha256><ext>` as the store writes it.
fn blob(user_data: &Path, url: &str) -> Result<PathBuf, Refused> {
    let invalid = "cindy-media: invalid url";
    let name = url.strip_prefix("cindy-media://blobs/").ok_or(invalid)?;
    let (hash, ext) = name.split_at_checked(64).ok_or(invalid)?;
    let hex = hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    if !hex || !BLOB_EXTS.contains(&ext) {
        return Err(invalid);
    }
    let root = user_data.join("cindy-media/blobs");
    Ok(root.join(&hash[..2]).join(name))
}

/// The file a managed address names.
pub fn path(user_data: &Path, url: &str) -> Result<PathBuf, Refused> {
    match url.starts_with("cindy-media://") {
        true => blob(user_data, url),
        false => cached(user_data, url),
    }
}

/// Why an image did not load: `readCachedImage`'s `NOT_FOUND`, or anything else.
#[derive(Debug, PartialEq)]
pub enum Failure {
    Missing,
    Unavailable(String),
}

/// The protocol handlers' read, as `fs.readFile` reads it.
pub fn read(user_data: &Path, url: &str) -> Result<Vec<u8>, Failure> {
    let path = path(user_data, url).map_err(|refused| Failure::Unavailable(refused.into()))?;
    std::fs::read(&path).map_err(|error| match error.kind() {
        ErrorKind::NotFound => Failure::Missing,
        _ => Failure::Unavailable(error.to_string()),
    })
}

#[cfg(test)]
mod tests;
