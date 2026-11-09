//! `InterruptedTasksResumeDialog`: the interrupted tasks, offered once per window and owner.

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    rc::Rc,
};

use gpui::{
    AnyElement, App, Global, IntoElement as _, ParentElement as _, Styled as _, Window, WindowId,
    div, prelude::FluentBuilder as _, relative,
};

use check::{Mark, Toggle};

use super::{Interrupted, candidates, shadowed, tasks::Task};
use crate::{
    appearance_settings::text_size,
    confirm_dialog::ConfirmDialog,
    i18n::{Arg, Translator},
    localdb::{LocalDb, State},
    sessions::{Sessions, lists, row, title},
    theme::Appearance,
    toast,
    units::css,
};

const LOG: &str = "interrupted-tasks-resume";

/// `_handledOwnerIds`, per window as each renderer keeps its own.
#[derive(Default)]
struct Handled(HashSet<(WindowId, PathBuf)>);

impl Global for Handled {}

/// The window's `open` and `deselected`, and the owner whose load it took.
#[derive(Default)]
struct Shown {
    open: bool,
    seen: Option<PathBuf>,
    deselected: HashSet<String>,
}

/// The dialog, opened when a load with tasks lands for an owner this window has not handled.
pub fn view(window: &mut Window, cx: &mut App) -> AnyElement {
    let here = window.window_handle().window_id();
    let shown = window.use_keyed_state("interrupted-resume", cx, |_, _| Shown::default());
    let loaded = cx.try_global::<Interrupted>().and_then(|store| {
        let ids = store.tasks.as_ref()?.iter().map(|t| t.session.id.clone());
        Some((store.owner.clone()?, ids.collect::<Vec<_>>()))
    });
    let Some((owner, ids)) = loaded else {
        return div().into_any_element();
    };
    let count = ids.len();
    if shown.read(cx).seen.as_ref() != Some(&owner) {
        let handled = cx
            .try_global::<Handled>()
            .is_some_and(|h| h.0.contains(&(here, owner.clone())));
        let open = !handled && count > 0;
        if open {
            log::info!(target: LOG, "found {count} interrupted task(s) to offer for resume");
        }
        shown.update(cx, |s, _| {
            s.seen = Some(owner.clone());
            s.open |= open;
        });
    }
    if count == 0 {
        return div().into_any_element();
    }
    let (open, deselected) = {
        let s = shown.read(cx);
        (s.open, s.deselected.clone())
    };
    let selected: Vec<String> = ids
        .iter()
        .filter(|id| !deselected.contains(*id))
        .cloned()
        .collect();
    let content = content(&ids, &selected, &shown, window, cx);
    let t = cx.global::<Translator>();
    let picked = selected.len();
    let confirm = match picked {
        0 => t.t("chat.interruptedResumeDialog.continueNone", &[]),
        n => t.t(
            "chat.interruptedResumeDialog.continueSelected",
            &[("count", Arg::Number(n as i64))],
        ),
    };
    let strings = (
        t.t("chat.interruptedResumeDialog.title", &[]).into(),
        confirm.into(),
        t.t("chat.interruptedResumeDialog.later", &[]).into(),
    );
    let description = t.t(
        "chat.interruptedResumeDialog.description",
        &[("count", Arg::Number(count as i64))],
    );
    let closing = shown.clone();
    ConfirmDialog::new(
        "interrupted-resume-dialog",
        open,
        strings,
        move |window, cx| {
            log::info!(target: LOG, "dialog closed; the owner is handled in this window");
            cx.default_global::<Handled>()
                .0
                .insert((window.window_handle().window_id(), owner.clone()));
            closing.update(cx, |s, cx| {
                s.open = false;
                cx.notify();
            });
        },
        move |window, cx| resume(selected.clone(), window, cx),
    )
    .cancel_first()
    .auto_focus_confirm()
    .max_width(460.0)
    .description(description)
    .content(content)
    .confirm_disabled(picked == 0)
    .into_any_element()
}

/// Sets `id` in or out of the selection; every press names the state it wants.
fn set(shown: &gpui::Entity<Shown>, ids: Vec<String>, on: bool, cx: &mut App) {
    shown.update(cx, |s, cx| {
        for id in ids {
            match on {
                true => s.deselected.remove(&id),
                false => s.deselected.insert(id),
            };
        }
        cx.notify();
    });
}

/// Select all with its counts, then the rows between two borders.
fn content(
    ids: &[String],
    selected: &[String],
    shown: &gpui::Entity<Shown>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let t = cx.global::<Translator>();
    let (total, count) = (ids.len(), selected.len());
    let state = match count {
        0 => Mark::Off,
        n if n == total => Mark::On,
        _ => Mark::Mixed,
    };
    let all_label = t.t("chat.interruptedResumeDialog.selectAll", &[]);
    let counts = t.t(
        "chat.interruptedResumeDialog.selectedCount",
        &[
            ("selected", Arg::Text(&count.to_string())),
            ("total", Arg::Text(&total.to_string())),
        ],
    );
    let appearance = cx.global::<Appearance>();
    let (title_fg, desc_fg, border) = (
        appearance.token("confirm-title"),
        appearance.token("confirm-desc"),
        appearance.token("border-default"),
    );
    let all = ids.to_vec();
    let target = shown.clone();
    let toggle: Toggle = Rc::new(move |on, _, cx| set(&target, all.clone(), on, cx));
    let focus = check::focus("interrupted-select-all-focus".into(), window, cx);
    let select_all = check::label(
        "interrupted-select-all".into(),
        (&focus, state != Mark::On, false),
        toggle.clone(),
        window,
        cx,
    );
    let all_box = check::checkbox(
        "interrupted-select-all-box".into(),
        (state, all_label.clone()),
        &focus,
        toggle,
        cx,
    );
    let header = div()
        .flex()
        .items_center()
        .justify_between()
        .px(css(8.0))
        .pb(css(8.0))
        .text_size(css(text_size(13.0, cx)))
        .child(
            select_all
                .flex()
                .items_center()
                .gap(css(10.0))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(title_fg)
                .child(all_box)
                .child(all_label),
        )
        .child(
            div()
                .text_size(css(text_size(12.0, cx)))
                .text_color(desc_fg)
                .child(counts),
        );
    let rows = rows(selected, shown, window, cx);
    div()
        .child(header)
        .child(
            div()
                .border_t(css(1.0))
                .border_b(css(1.0))
                .border_color(border)
                .py(css(4.0))
                .children(rows),
        )
        .into_any_element()
}

/// One `label` per task: checkbox, vendor icon, title, then source, activity and time.
fn rows(
    selected: &[String],
    shown: &gpui::Entity<Shown>,
    window: &mut Window,
    cx: &mut App,
) -> Vec<AnyElement> {
    let Some(tasks) = cx.global::<Interrupted>().tasks.clone() else {
        return Vec::new();
    };
    let t = cx.global::<Translator>();
    let sessions: Vec<_> = tasks.iter().map(|task| task.session.clone()).collect();
    let dialogues = t.t("ccAgent.sidebar.dialogues", &[]);
    let labels: HashMap<String, String> = lists::labels(&sessions, &[], &dialogues);
    let now = jiff::Timestamp::now().as_millisecond();
    let appearance = cx.global::<Appearance>();
    let (title_fg, desc_fg) = (
        appearance.token("confirm-title"),
        appearance.token("confirm-desc"),
    );
    let (s13, s12) = (text_size(13.0, cx), text_size(12.0, cx));
    let texts: Vec<(String, String)> = tasks
        .iter()
        .map(|task| {
            let label = labels.get(&task.session.id).map(String::as_str);
            (title::display(&task.session, t), meta(task, label, now, t))
        })
        .collect();
    tasks
        .iter()
        .zip(texts)
        .map(|(task, (name, meta))| {
            let session = &task.session;
            let checked = selected.contains(&session.id);
            let (id, target) = (session.id.clone(), shown.clone());
            let one = id.clone();
            let toggle: Toggle = Rc::new(move |on, _, cx| set(&target, vec![one.clone()], on, cx));
            let focus = check::focus(format!("interrupted-box-{id}-focus"), window, cx);
            let mark = if checked { Mark::On } else { Mark::Off };
            let checkbox = check::checkbox(
                format!("interrupted-box-{id}"),
                (mark, name.clone()),
                &focus,
                toggle.clone(),
                cx,
            );
            check::label(
                format!("interrupted-row-{id}"),
                (&focus, !checked, true),
                toggle,
                window,
                cx,
            )
            .flex()
            .items_center()
            .gap(css(10.0))
            .rounded(css(8.0))
            .p(css(8.0))
            .child(checkbox)
            .child(row::vendor(&session.agent_kind, 14.0, desc_fg))
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .when(!checked, |el| el.opacity(0.5))
                    .child(
                        div()
                            .truncate()
                            .text_size(css(s13))
                            .line_height(relative(1.5))
                            .text_color(title_fg)
                            .child(name),
                    )
                    .child(
                        div()
                            .mt(css(2.0))
                            .truncate()
                            .text_size(css(s12))
                            .line_height(relative(1.5))
                            .text_color(desc_fg)
                            .child(meta),
                    ),
            )
            .into_any_element()
        })
        .collect()
}

/// The row's second line: companion or source, activity, then when it stopped.
fn meta(task: &Task, label: Option<&str>, now: i64, t: &Translator) -> String {
    let when = match now - task.interrupted_at < 60_000 {
        true => t.t("chat.interruptedResumeDialog.interruptedJustNow", &[]),
        false => {
            let tr = |key: &str, n: i64| t.t(key, &[("count", Arg::Number(n))]);
            let time = row::relative(task.interrupted_at, now, &tr);
            t.t(
                "chat.interruptedResumeDialog.interruptedAgo",
                &[("time", Arg::Text(&time))],
            )
        }
    };
    [
        task.companion.as_deref().or(label),
        task.activity.as_deref(),
        Some(when.as_str()),
    ]
    .into_iter()
    .flatten()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join(" · ")
}

/// `handleContinue`: the picked tasks still interrupted now; their send comes with 10.0.
fn resume(picked: Vec<String>, window: &mut Window, cx: &mut App) {
    let State::Ready(db) = &cx.global::<LocalDb>().state else {
        log::warn!(target: LOG, "continue: the database is not ready");
        failed(picked.len(), window, cx);
        return;
    };
    let db = db.clone();
    let dir = cx.global::<Sessions>().owner_dir.clone();
    let boot_at = crate::cross_process_lock::started_ms() as i64;
    let still = db.call(move |conn| {
        let shadowed = dir.as_deref().map(shadowed).unwrap_or_default();
        candidates(conn, boot_at, &shadowed)
    });
    window
        .spawn(cx, async move |cx| {
            let still: Option<HashSet<String>> = match still.await {
                Ok(Ok(rows)) => Some(rows.into_iter().map(|p| p.session_id).collect()),
                Ok(Err(error)) => {
                    log::warn!(target: LOG, "continue: the recheck failed: {error}");
                    None
                }
                Err(_) => {
                    log::error!(target: LOG, "continue: the database thread dropped the recheck");
                    None
                }
            };
            let shown = cx.update(|window, cx| match still {
                None => failed(picked.len(), window, cx),
                Some(still) => {
                    let ids = picked.iter().filter(|id| still.contains(*id)).count();
                    if ids == 0 {
                        return;
                    }
                    log::warn!(target: LOG, "failed to continue {ids}/{ids} interrupted task(s): no send path before 10.0");
                    failed(ids, window, cx);
                }
            });
            if shown.is_err() {
                log::warn!(target: LOG, "continueFailed: its window closed before the toast");
            }
        })
        .detach();
}

/// The `continueFailed` toast for `count` tasks.
fn failed(count: usize, window: &mut Window, cx: &mut App) {
    let message = cx.global::<Translator>().t(
        "chat.interruptedResumeDialog.continueFailed",
        &[("count", Arg::Number(count as i64))],
    );
    toast::error(window, message, toast::Options::default(), cx);
}

mod check;

#[cfg(test)]
mod tests;
