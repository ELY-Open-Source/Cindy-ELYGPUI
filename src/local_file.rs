//! `localFileProtocol.ts`: the read behind an `xdt-file://local/?path=` address.

use std::io;

use url::Url;

use crate::{file_path_policy, node_path};

#[cfg(test)]
mod tests;

/// `MEDIA_EXT_WHITELIST` with `MIME_BY_EXT`.
const MEDIA: [(&str, &str); 16] = [
    (".png", "image/png"),
    (".jpg", "image/jpeg"),
    (".jpeg", "image/jpeg"),
    (".gif", "image/gif"),
    (".webp", "image/webp"),
    (".svg", "image/svg+xml"),
    (".bmp", "image/bmp"),
    (".ico", "image/x-icon"),
    (".pdf", "application/pdf"),
    (".drawio", "application/xml"),
    (".glb", "model/gltf-binary"),
    (".gltf", "model/gltf+json"),
    (".mp4", "video/mp4"),
    (".m4v", "video/mp4"),
    (".mov", "video/quicktime"),
    (".webm", "video/webm"),
];

/// A file the handler serves: its bytes and its `Content-Type`.
#[derive(Debug, PartialEq)]
pub struct Served {
    pub bytes: Vec<u8>,
    pub mime: &'static str,
}

/// The status an error answers with: `realpath`'s own catch, or the handler's outer one.
fn status(error: &io::Error, realpath: bool) -> u16 {
    #[cfg(unix)]
    let (missing, denied) = {
        let is = |codes: &[i32]| {
            error
                .raw_os_error()
                .is_some_and(|code| codes.contains(&code))
        };
        match realpath {
            true => (
                is(&[libc::ENOENT, libc::ENOTDIR, libc::ELOOP]),
                is(&[libc::EACCES, libc::EPERM]),
            ),
            false => (is(&[libc::ENOENT]), is(&[libc::EACCES])),
        }
    };
    // libuv reads Windows' access refusal as `EPERM`, which the outer catch does not know.
    #[cfg(not(unix))]
    let (missing, denied) = (
        error.kind() == io::ErrorKind::NotFound,
        realpath && error.kind() == io::ErrorKind::PermissionDenied,
    );
    match (missing, denied) {
        (true, _) => 404,
        (false, true) => 403,
        (false, false) => 500,
    }
}

/// `WIN_ABS_RE` or a leading slash.
fn absolute(path: &str) -> bool {
    let b = path.as_bytes();
    let drive = b.len() > 2 && b[0].is_ascii_alphabetic() && b[1] == b':';
    path.starts_with('/') || (drive && matches!(b[2], b'/' | b'\\'))
}

/// The handler's answer to a `GET` of `url` with no `Range`: the file, or the refusal's status.
pub fn read(url: &str) -> Result<Served, u16> {
    let outer = |error: io::Error| {
        let status = status(&error, false);
        if status == 500 {
            log::error!(target: "localFileProtocol", "[xdt-file] handler error: {error}");
        }
        status
    };
    let parsed = Url::parse(url).map_err(|error| {
        log::error!(target: "localFileProtocol", "[xdt-file] handler error: {error}");
        500_u16
    })?;
    let named = parsed.query_pairs().find(|(key, _)| key == "path");
    let decoded = named.map(|(_, path)| path.into_owned()).unwrap_or_default();
    if decoded.is_empty() {
        return Err(400);
    }
    if !absolute(&decoded) {
        return Err(403);
    }
    // Node asks for the current folder only where a path needs it; here a lost folder refuses all.
    let cwd = std::env::current_dir().map_err(outer)?;
    let cwd = cwd.to_string_lossy();
    let resolved = node_path::resolve(&decoded, &cwd);
    let normalized = node_path::normalize(&decoded);
    let same = match cfg!(windows) {
        true => resolved.to_lowercase() == normalized.to_lowercase(),
        false => resolved == normalized,
    };
    if !same {
        return Err(403);
    }
    let ext = node_path::extname(&resolved).to_lowercase();
    let Some((_, mime)) = MEDIA.iter().find(|(known, _)| *known == ext) else {
        return Err(415);
    };
    if !file_path_policy::media_allowed(&resolved, &cwd) {
        return Err(403);
    }
    let real = std::fs::canonicalize(&resolved).map_err(|error| match status(&error, true) {
        500 => outer(error),
        status => status,
    })?;
    if !file_path_policy::media_allowed(&real.to_string_lossy(), &cwd) {
        return Err(403);
    }
    if !std::fs::metadata(&real).map_err(outer)?.is_file() {
        return Err(404);
    }
    // Cindy streams the file after its headers; a read that fails there fails the load alike.
    let bytes = std::fs::read(&real).map_err(outer)?;
    Ok(Served { bytes, mime })
}
