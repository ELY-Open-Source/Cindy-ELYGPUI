//! `handleRemoveProjectFromSidebar`: confirmed, then the key joins `hiddenProjectKeys`.

use gpui::{App, AppContext as _, Window};

use super::super::{Sessions, paths::PLATFORM, rail, select, settings};
use crate::{
    confirm::{self, Ask},
    i18n::{Arg, Translator},
    toast,
};

/// Only a local project leaves the sidebar, as Add Project restores local folders alone.
pub fn remove(key: &str, name: &str, window: &mut Window, cx: &mut App) {
    if !key.starts_with("local:") {
        return;
    }
    let t = cx.global::<Translator>();
    let text = |k: &str| t.t(&format!("ccAgent.sidebar.projectAction.{k}"), &[]);
    let ask = Ask {
        title: t.t(
            "ccAgent.sidebar.projectAction.removeFromSidebarConfirmTitle",
            &[("name", Arg::Text(name))],
        ),
        description: Some(text("removeFromSidebarConfirmDescription")),
        confirm_text: Some(text("removeFromSidebarConfirmAction")),
        auto_focus_confirm: true,
    };
    let answer = confirm::ask(window, ask, cx);
    let (key, handle) = (key.to_owned(), window.window_handle());
    let owner_dir = cx.global::<Sessions>().owner_dir.clone();
    cx.spawn(async move |cx| {
        let Ok(true) = answer.await else {
            return log::info!(target: "sessions", "remove project {key}: not confirmed");
        };
        let saving = cx.background_spawn({
            let key = key.clone();
            async move {
                let Some(owner_dir) = owner_dir else {
                    anyhow::bail!("sidebar settings require an active data owner");
                };
                settings::hide(&owner_dir, &key, PLATFORM)
            }
        });
        let result = saving.await;
        cx.update(|cx| {
            let shown = handle.update(cx, |_, window, cx| settled(&key, result, window, cx));
            if shown.is_err() {
                log::warn!(target: "sessions", "remove project {key}: its window closed");
            }
        });
    })
    .detach();
}

fn settled(key: &str, result: anyhow::Result<Vec<String>>, window: &mut Window, cx: &mut App) {
    let hidden = match result {
        Ok(hidden) => hidden,
        Err(error) => {
            log::warn!(target: "sessions", "remove project {key} from sidebar failed: {error:#}");
            let message = cx
                .global::<Translator>()
                .t("ccAgent.sidebar.projectAction.removeFromSidebarFailed", &[]);
            toast::error(window, message, toast::Options::default(), cx);
            return;
        }
    };
    log::info!(target: "sessions", "project {key} removed from the sidebar");
    if let Some(loaded) = cx.global_mut::<Sessions>().loaded.as_mut() {
        loaded.settings.hidden = hidden;
    }
    super::super::watch_owner(cx);
    select::clear(cx);
    rail::sections::close_all(window, cx);
    cx.refresh_windows();
}

#[cfg(test)]
mod tests;
