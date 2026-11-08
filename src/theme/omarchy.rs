use std::{
    collections::HashMap,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result, bail};
use futures::channel::mpsc::UnboundedSender;
use notify::{
    EventKind, RecommendedWatcher, RecursiveMode, Watcher as _,
    event::{AccessKind, AccessMode},
};

use super::{ThemeData, normalize::normalize};
use crate::theme_import::convert_vscode_theme;

pub const THEME_FILE: &str = "vscode-theme.json";

/// Omarchy's current-theme folder; read, never written.
pub fn state_dir(home: &Path) -> PathBuf {
    home.join(".local")
        .join("state")
        .join("omarchy")
        .join("current")
}

/// Cindy's `readOmarchyTheme`: none when Omarchy is not installed.
pub fn read(
    dir: &Path,
    defaults: &HashMap<String, (Option<String>, Option<String>)>,
) -> Result<Option<ThemeData>> {
    match fs::metadata(dir.join("theme.name")) {
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        other => other.with_context(|| format!("stat {}", dir.join("theme.name").display()))?,
    };
    let file = dir.join("theme").join(THEME_FILE);
    let raw = fs::read_to_string(&file).with_context(|| format!("read {}", file.display()))?;
    let Some(theme) =
        convert_vscode_theme(&raw, "Omarchy")?.and_then(|c| c.themes.into_iter().next())
    else {
        bail!("{THEME_FILE} is not a VS Code color theme");
    };
    let colors = normalize(theme.colors.into_iter().collect(), theme.kind, defaults);
    Ok(Some(ThemeData {
        id: "omarchy".into(),
        kind: theme.kind,
        family: None,
        colors,
    }))
}

/// Cindy's `watchOmarchyTheme`: one message per finished `theme.name` write.
pub fn watch(dir: &Path, on_switch: UnboundedSender<()>) -> Result<Option<RecommendedWatcher>> {
    if !dir.is_dir() {
        return Ok(None);
    }
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        match event {
            Ok(event)
                if event.kind == EventKind::Access(AccessKind::Close(AccessMode::Write))
                    && event
                        .paths
                        .iter()
                        .any(|p| p.file_name().is_some_and(|n| n == "theme.name")) =>
            {
                if on_switch.unbounded_send(()).is_err() {
                    log::warn!(target: "theme", "Omarchy switch arrived after the app stopped listening");
                }
            }
            Ok(_) => {}
            Err(error) => log::error!(target: "theme", "Omarchy theme watcher failed: {error}"),
        }
    })?;
    watcher.watch(dir, RecursiveMode::NonRecursive)?;
    log::info!(target: "theme", "Watching Omarchy theme switches in {}", dir.display());
    Ok(Some(watcher))
}
