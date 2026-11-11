//! `localPathResolver.ts`'s reading of a reference's shape; nothing here touches a disk.
#![allow(dead_code)] // The tree's reader comes with 9.6e.

use std::sync::LazyLock;

use regex::Regex;

use super::{drive, strip_drive_slash};
use crate::{
    cindy_json::{decode_uri_component, encode_uri_component},
    js_re,
};

/// `LocalHrefKind`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Image,
    Model,
    Text,
    External,
    Directory,
}

/// A query to the end of the text, as a JavaScript dot reads it.
const QUERY: &str = r"(\?[^\n\r\u{2028}\u{2029}]*)?$";

fn pattern(source: &str) -> Regex {
    js_re::regex(source, false)
}

/// `URL_SCHEME_RE`.
pub fn scheme_slashes(text: &str) -> bool {
    static SCHEME: LazyLock<Regex> = LazyLock::new(|| pattern(r"^[A-Za-z][A-Za-z0-9+.-]*://"));
    SCHEME.is_match(text)
}

/// `looksLikeDirectoryPath`.
pub fn directory(text: &str) -> bool {
    text.ends_with(['/', '\\'])
}

/// `looksLikeFilePath`.
pub fn file(text: &str) -> bool {
    static POSIX: LazyLock<Regex> =
        LazyLock::new(|| pattern(r"^/[^/\s][^\s]*\.[A-Za-z0-9]{1,10}(\?[^\s]*)?$"));
    static RELATIVE: LazyLock<Regex> =
        LazyLock::new(|| pattern(r"^[^\s:]*[\\/][^\s:]+\.[A-Za-z0-9]{1,10}(\?[^\s]*)?$"));
    if text.is_empty() || text.contains('\n') || scheme_slashes(text) || directory(text) {
        return false;
    }
    drive(text) || POSIX.is_match(text) || RELATIVE.is_match(text)
}

/// `classifyMarkdownHref`.
pub fn classify(href: &str) -> Kind {
    static EXTENSION: LazyLock<Regex> =
        LazyLock::new(|| pattern(&format!(r"\.[A-Za-z0-9]{{1,10}}{QUERY}")));
    static IMAGE: LazyLock<Regex> =
        LazyLock::new(|| pattern(&format!(r"\.(?i-u:png|jpe?g|gif|webp|svg|bmp|ico){QUERY}")));
    static MODEL: LazyLock<Regex> =
        LazyLock::new(|| pattern(&format!(r"\.(?i-u:glb|gltf|fbx){QUERY}")));
    let file_url = href.starts_with("file://");
    if href.is_empty() || href.starts_with('#') {
        return Kind::External;
    }
    if scheme_slashes(href) && !file_url {
        return Kind::External;
    }
    let probe = href.strip_prefix("file://").unwrap_or(href);
    let local = probe.contains(['/', '\\']) || EXTENSION.is_match(probe);
    match () {
        () if !local => Kind::External,
        () if directory(probe) => Kind::Directory,
        () if IMAGE.is_match(probe) => Kind::Image,
        () if MODEL.is_match(probe) => Kind::Model,
        () => Kind::Text,
    }
}

/// `basenameFromPathLike`, lowered.
fn name(path: &str) -> String {
    path.rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
        .to_lowercase()
}

/// `safeDecodeURIComponent`.
fn decoded(value: &str) -> String {
    decode_uri_component(value).unwrap_or_else(|| value.to_owned())
}

/// `resolveKnownLocalFileHref`: the path of the one attached file `href` names.
pub fn known<'a>(href: &str, files: &'a [(String, String)]) -> Option<&'a str> {
    static ROOTED: LazyLock<Regex> =
        LazyLock::new(|| pattern(r"^(?:(?i-u:file)://|[\\/]|[A-Za-z]:[\\/]|~[\\/])"));
    let trimmed = crate::theme_import::color::js_trim(href);
    let path = trimmed.split('#').next().unwrap_or(trimmed);
    let path = decoded(path.split('?').next().unwrap_or(path));
    let wanted = name(&path);
    if ROOTED.is_match(&path) || wanted.is_empty() {
        return None;
    }
    let mut named = files
        .iter()
        .filter(|(file, path)| wanted == name(file) || wanted == name(path));
    let (_, path) = named.next()?;
    named.next().is_none().then_some(path.as_str())
}

/// `toLocalFileUrl`.
fn file_url(path: &str, revision: Option<&str>) -> String {
    let url = format!("xdt-file://local/?path={}", encode_uri_component(path));
    match revision {
        Some(revision) if !revision.is_empty() => {
            format!("{url}&v={}", encode_uri_component(revision))
        }
        _ => url,
    }
}

/// `normalizeXdtFileUrlForRenderer`: a path-shaped `xdt-file:///` as the handler's query form.
pub fn xdt_file(url: &str) -> String {
    static REVISION: LazyLock<Regex> = LazyLock::new(|| pattern("(?:^|&)v=([^&#]*)"));
    let Some(rest) = url
        .strip_prefix("xdt-file://")
        .filter(|rest| rest.starts_with('/'))
    else {
        return url.to_owned();
    };
    let cut = rest.find(['?', '#']).unwrap_or(rest.len());
    let path = decoded(&rest[..cut]);
    let path = strip_drive_slash(&path);
    let query = rest[cut..].strip_prefix('?');
    let revision = query.and_then(|query| REVISION.captures(query));
    file_url(&path, revision.map(|found| decoded(&found[1])).as_deref())
}

#[cfg(test)]
mod tests {
    use super::{Kind, classify, directory, file, known, xdt_file};
    use crate::{
        deep_link::tests::text,
        markdown_target::tests::{files, golden, optional},
        rich_answer::js::Js,
    };

    #[test]
    fn shapes_read_as_cindys() {
        golden("looksLikeFilePath", |args| {
            Js::Bool(file(optional(args, 0).unwrap_or("")))
        });
        golden("looksLikeDirectoryPath", |args| {
            Js::Bool(directory(optional(args, 0).unwrap_or("")))
        });
        golden("classifyMarkdownHref", |args| {
            text(match classify(optional(args, 0).unwrap_or("")) {
                Kind::Image => "image-local",
                Kind::Model => "model-local",
                Kind::Text => "text-local",
                Kind::External => "external",
                Kind::Directory => "directory",
            })
        });
        golden("normalizeXdtFileUrlForRenderer", |args| {
            Js::Str(xdt_file(args[0].str().unwrap()))
        });
    }

    #[test]
    fn an_attached_file_resolves_as_cindys() {
        golden("resolveKnownLocalFileHref", |args| {
            known(args[0].str().unwrap(), &files(args, 1)).map_or(Js::Null, text)
        });
    }
}
