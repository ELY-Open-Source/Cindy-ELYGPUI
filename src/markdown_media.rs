//! `markdownMedia.ts`: a Markdown image that is one of Cindy's own videos.

use std::sync::LazyLock;

use regex::Regex;

use crate::remote_media_url;

static SCHEME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:cindy-media|xdt-video|cindy-remote-media)://").expect("a fixed regex")
});
static VIDEO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\.(?:mp4|webm|mov|m4v)(?:[?#]|$)").expect("a fixed regex"));

/// `isManagedMarkdownVideoUrl`: a remote media URL is judged by the address it wraps.
pub fn is_managed_video(src: &str) -> bool {
    let wrapped = remote_media_url::orig(src);
    let target = wrapped.as_deref().unwrap_or(src);
    SCHEME.is_match(target) && VIDEO.is_match(target)
}
