//! `pendingProjectFocus.ts` and the sidebar's effect on it; no machine switch to fall back from.

use gpui::{AnyWindowHandle, App, Window};

use super::{
    Sessions, fold, main_list,
    paths::{PLATFORM, compare, normalize_key},
    reveal,
};
use crate::{i18n::Translator, toast};

/// The folder asked for, and the window its toast goes to.
pub(super) struct Pending {
    dir: String,
    window: AnyWindowHandle,
}

/// `findProjectRepresentativeKey`: the drawn key that compares as `dir`'s.
fn representative(projects: &[String], dir: &str, platform: &str) -> Option<String> {
    let key = normalize_key(dir).unwrap_or_else(|| format!("local:{dir}"));
    let wanted = compare(&key, platform)?;
    let same = |project: &&String| compare(project, platform).as_ref() == Some(&wanted);
    projects.iter().find(same).cloned()
}

/// `requestProjectFocus`: the latest ask replaces one not yet consumed.
pub fn request(dir: &str, window: &Window, cx: &mut App) {
    if dir.is_empty() {
        return;
    }
    cx.global_mut::<Sessions>().focus = Some(Pending {
        dir: dir.to_owned(),
        window: window.window_handle(),
    });
    cx.defer(settle);
}

/// The effect: it waits out the first load, then consumes the ask.
pub(super) fn settle(cx: &mut App) {
    let store = cx.global::<Sessions>();
    if store.focus.is_none() || (store.loaded.is_none() && store.loading) {
        return;
    }
    let projects = main_list::lists(cx).map_or(Vec::new(), |lists| lists.focusable);
    let Some(Pending { dir, window }) = cx.global_mut::<Sessions>().focus.take() else {
        return;
    };
    let Some(key) = representative(&projects, &dir, PLATFORM) else {
        log::warn!(target: "sessions", "project focus: none of {} projects", projects.len());
        let message = cx
            .global::<Translator>()
            .t("ccAgent.sidebar.deepLink.projectNotFound", &[]);
        let shown = window.update(cx, |_, window, cx| {
            toast::warning(window, message, toast::Options::default(), cx);
        });
        if let Err(error) = shown {
            log::warn!(target: "sessions", "project focus: the toast had no window: {error:#}");
        }
        return;
    };
    log::info!(target: "sessions", "project focus: {key}");
    // `collapse.expand`: only a folded project changes.
    let fold_id = format!("project:{key}");
    if cx.global_mut::<Sessions>().collapsed.remove(&fold_id) {
        fold::toggle(&fold_id, false, cx);
    }
    reveal::project(&key, cx);
    cx.refresh_windows();
}

#[cfg(test)]
mod tests;
