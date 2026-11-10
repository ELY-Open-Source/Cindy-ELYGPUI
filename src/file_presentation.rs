//! maker-shared `filePresentation.ts`: what a file looks like, never what may open it.

#![cfg_attr(not(test), expect(dead_code, reason = "9.5i's file chip draws it"))]

use std::sync::LazyLock;

use regex::Regex;

use crate::{js_re, theme_import::color::js_trim};

/// `FileVisualKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Code,
    Text,
    Pdf,
    Document,
    Sheet,
    Slide,
    Image,
    Audio,
    Video,
    Archive,
    Database,
    File,
}

const CODE: &[&str] = &[
    "ts",
    "tsx",
    "js",
    "jsx",
    "mjs",
    "cjs",
    "py",
    "rb",
    "go",
    "rs",
    "java",
    "kt",
    "kts",
    "swift",
    "c",
    "h",
    "cpp",
    "cc",
    "cxx",
    "hpp",
    "cs",
    "php",
    "dart",
    "lua",
    "scala",
    "sc",
    "groovy",
    "gradle",
    "pl",
    "pm",
    "r",
    "hs",
    "proto",
    "sh",
    "bash",
    "zsh",
    "ps1",
    "yaml",
    "yml",
    "toml",
    "ini",
    "json",
    "jsonc",
    "xml",
    "html",
    "htm",
    "xhtml",
    "vue",
    "svelte",
    "css",
    "scss",
    "sass",
    "less",
    "sql",
    "graphql",
    "gql",
    "diff",
    "patch",
    "csproj",
    "sln",
    "shader",
    "unityproj",
    "asmdef",
    "dockerfile",
    "makefile",
    "mk",
];
const TEXT: &[&str] = &[
    "md", "mdx", "markdown", "mdown", "mkdn", "mkd", "txt", "text", "log", "rst",
];
const IMAGE: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "tga", "tif", "tiff", "svg", "avif", "heic",
    "heif",
];

/// `EXTENSIONS`.
const EXTENSIONS: &[(Kind, &[&str])] = &[
    (Kind::Code, CODE),
    (Kind::Text, TEXT),
    (Kind::Pdf, &["pdf"]),
    (Kind::Document, &["doc", "docx", "rtf", "odt", "pages"]),
    (
        Kind::Sheet,
        &["xls", "xlsx", "xlsm", "csv", "tsv", "ods", "numbers"],
    ),
    (Kind::Slide, &["ppt", "pptx", "odp", "key"]),
    (Kind::Image, IMAGE),
    (
        Kind::Audio,
        &["mp3", "m4a", "wav", "flac", "ogg", "aac", "aiff", "opus"],
    ),
    (
        Kind::Video,
        &["mp4", "mov", "webm", "mkv", "avi", "m4v", "mpeg", "mpg"],
    ),
    (
        Kind::Archive,
        &["zip", "7z", "rar", "tar", "gz", "tgz", "bz2", "xz", "zst"],
    ),
    (Kind::Database, &["db", "sqlite", "sqlite3", "realm"]),
];
const CODE_NAMES: &[&str] = &[
    "dockerfile",
    "makefile",
    "gemfile",
    "rakefile",
    "procfile",
    "vagrantfile",
    "jenkinsfile",
    "cmakelists.txt",
    ".gitignore",
    ".gitattributes",
    ".npmrc",
    ".editorconfig",
    ".env",
];
const TEXT_NAMES: &[&str] = &["readme", "license", "licence", "changelog", "notice"];
const CODE_MIMES: &[&str] = &[
    "application/json",
    "application/xml",
    "application/javascript",
    "text/javascript",
    "text/html",
    "text/css",
];

/// A JS `.` between literals: one UTF-16 unit, no line terminator.
const DOT: &str = r"[^\n\r\x{2028}\x{2029}\x{10000}-\x{10ffff}]";

/// The kind a MIME type hints at, read when the name tells nothing.
fn hinted(mime_type: &str) -> Kind {
    static SHEET: LazyLock<Regex> = LazyLock::new(|| {
        js_re::regex(
            &format!("spreadsheet|excel|opendocument{DOT}spreadsheet"),
            false,
        )
    });
    static SLIDE: LazyLock<Regex> =
        LazyLock::new(|| js_re::regex("presentation|powerpoint", false));
    static DOCUMENT: LazyLock<Regex> = LazyLock::new(|| {
        js_re::regex(
            &format!("wordprocessing|msword|opendocument{DOT}text"),
            false,
        )
    });
    static ARCHIVE: LazyLock<Regex> = LazyLock::new(|| {
        let names = format!("zip|gzip|x-tar|x-7z-compressed|vnd{DOT}rar|x-rar-compressed");
        js_re::regex(&format!(r"^application\/({names})$"), false)
    });
    let mime = mime_type.split(';').next().unwrap_or_default();
    let mime = js_trim(mime).to_lowercase();
    let mime = mime.as_str();
    let sheets = ["text/csv", "text/tab-separated-values"];
    let databases = ["application/vnd.sqlite3", "application/x-sqlite3"];
    match mime {
        _ if mime.starts_with("image/") => Kind::Image,
        _ if mime.starts_with("audio/") => Kind::Audio,
        _ if mime.starts_with("video/") => Kind::Video,
        "application/pdf" => Kind::Pdf,
        _ if SHEET.is_match(mime) || sheets.contains(&mime) => Kind::Sheet,
        _ if SLIDE.is_match(mime) => Kind::Slide,
        _ if DOCUMENT.is_match(mime) || mime == "application/rtf" => Kind::Document,
        _ if CODE_MIMES.contains(&mime) => Kind::Code,
        _ if ARCHIVE.is_match(mime) => Kind::Archive,
        _ if databases.contains(&mime) => Kind::Database,
        _ if mime.starts_with("text/") => Kind::Text,
        _ => Kind::File,
    }
}

/// `getFileVisualKind`: a known name or extension wins over the MIME hint.
pub fn kind(name: &str, mime_type: &str) -> Kind {
    let base = name.rsplit(['\\', '/']).next().unwrap_or_default();
    let base = base.to_lowercase();
    if CODE_NAMES.contains(&base.as_str()) || base.starts_with(".env.") {
        return Kind::Code;
    }
    if TEXT_NAMES.contains(&base.as_str()) {
        return Kind::Text;
    }
    let extension = base.rfind('.').map(|dot| &base[dot + 1..]);
    let known = extension.and_then(|extension| {
        let holds = |(_, list): &&(Kind, &[&str])| list.contains(&extension);
        EXTENSIONS.iter().find(holds).map(|(kind, _)| *kind)
    });
    known.unwrap_or_else(|| hinted(mime_type))
}

/// `FILE_VISUAL_GLYPHS`: the lucide glyph's name.
pub fn glyph(kind: Kind) -> &'static str {
    match kind {
        Kind::Code => "FileCode",
        Kind::Text | Kind::Pdf | Kind::Document => "FileText",
        Kind::Sheet => "FileSpreadsheet",
        Kind::Slide => "FileChartColumn",
        Kind::Image => "FileImage",
        Kind::Audio => "FileAudio",
        Kind::Video => "FileVideo",
        Kind::Archive => "FileArchive",
        Kind::Database => "Database",
        Kind::File => "File",
    }
}

#[cfg(test)]
mod tests;
