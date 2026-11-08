use ely_gpui_component::theme::{Mode as ElyMode, Theme};
use gpui::{App, Context, Window};

use super::{Appearance, Kind, ely, omarchy};

fn ely_mode(kind: Kind) -> ElyMode {
    match kind {
        Kind::Light => ElyMode::Light,
        Kind::Dark => ElyMode::Dark,
    }
}

fn log_theme(appearance: &Appearance) {
    let active = &appearance.active;
    let fallback = if active.fallback {
        ", other type stands in"
    } else {
        ""
    };
    log::info!(target: "theme", "theme {} (family {}, mode {:?}{fallback})", active.theme.id, active.family, appearance.mode);
}

/// Sets the first theme before any window draws.
pub fn install(appearance: Appearance, cx: &mut App) {
    log_theme(&appearance);
    let mode = ely_mode(appearance.kind());
    Theme::set_palette(mode, Some(ely::palette(&appearance)), cx);
    Theme::set_mode_now(mode, cx);
    cx.set_global(appearance);
}

/// Hands the active theme to Ely and repaints.
fn apply(cx: &mut App) {
    let appearance = cx.global::<Appearance>();
    log_theme(appearance);
    let (kind, palette) = (appearance.kind(), ely::palette(appearance));
    Theme::set_palette(ely_mode(kind), Some(palette), cx);
    Theme::set_mode(ely_mode(kind), cx);
    crate::dock_icon::apply(cx);
    for handle in cx.windows() {
        let applied = handle.update(cx, |_, window, cx| {
            crate::window_material::apply(window, cx)
        });
        if let Err(error) = applied {
            log::debug!(target: "vibrancyConfig", "window gone before its material: {error:#}");
        }
    }
    cx.refresh_windows();
}

/// Re-resolves the theme when the OS turns light or dark.
pub fn follow_os<V>(_: &mut V, window: &mut Window, cx: &mut Context<V>) {
    let os = window.appearance();
    let changed = cx
        .global_mut::<Appearance>()
        .follow(os)
        .unwrap_or_else(|error| panic!("theme failed to follow the OS: {error:#}"));
    if changed {
        log::info!(target: "theme", "OS appearance {os:?}");
        apply(cx);
    }
}

/// Reloads themes after each Omarchy switch, as Cindy's `local-themes:changed` does.
pub fn follow_omarchy(dir: &std::path::Path, cx: &mut App) {
    let (sender, mut switches) = futures::channel::mpsc::unbounded();
    let watcher = match omarchy::watch(dir, sender) {
        Ok(Some(watcher)) => watcher,
        Ok(None) => return,
        Err(error) => {
            return log::error!(target: "theme", "Omarchy theme watcher not started: {error:#}");
        }
    };
    cx.spawn(async move |cx| {
        let _watcher = watcher;
        while futures::StreamExt::next(&mut switches).await.is_some() {
            cx.update(|cx| {
                log::info!(target: "theme", "Omarchy theme switched; reloading themes");
                if let Err(error) = cx.global_mut::<Appearance>().reload() {
                    panic!("themes failed to reload: {error:#}");
                }
                apply(cx);
            });
        }
    })
    .detach();
}
