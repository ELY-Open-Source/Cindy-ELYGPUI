//! `SessionLinkChip` and `ProjectLinkChip`: what a deep link's chip says.

use std::collections::HashMap;

use ely_gpui_component::primitives::IconName;
use gpui::{App, SharedString};

use super::{
    SessionView,
    chip::{self, Chip, Stand},
};
use crate::{
    chat_message::user_content,
    chat_quotes::{self, Quote},
    deep_link,
    history_view::{LIVE, rows},
    i18n::Translator,
    localdb::{LocalDb, State, search::mcp_app::TrustLabels},
    rich_answer::js::Js,
    sessions::title::DRAFT,
    theme_import::color::{js_space, js_trim},
};

pub mod act;

/// What a click on a link chip does, by the link's address.
#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    Session(SharedString),
    Project(SharedString),
}

/// What the mounted chips read: a task's title, a message's text; a key held is a read asked.
#[derive(Default)]
pub struct Found {
    titles: HashMap<String, Option<String>>,
    texts: HashMap<(String, String), Option<String>>,
    /// `navigationRequestVersionRef` by chip: its latest open asked, which alone may navigate.
    opening: HashMap<String, u64>,
    /// The session chips the stream drew last.
    drawn: Vec<String>,
    /// Each mounted session chip as Cindy keys it and its hook reads it.
    marks: HashMap<String, Mark>,
}

/// A session chip's place in its message (line and byte) and its link's target.
type Mark = ((usize, usize), deep_link::Session);

/// Before the stream's rows draw: no chip is mounted yet.
pub fn begin(cx: &mut App) {
    if let Some(found) = held_mut(cx) {
        found.drawn.clear();
    }
}

/// A session chip was drawn at `place`; a new place or target ends what the chip asked.
pub fn drawn(id: &str, place: (usize, usize), href: &str, cx: &mut App) {
    if let Some(target) = deep_link::parse_session(href) {
        mounted(id, place, target, cx);
    }
}

/// `drawn` for what opens `target`: a chip, or a handoff card.
pub fn mounted(id: &str, place: (usize, usize), target: deep_link::Session, cx: &mut App) {
    let Some(found) = held_mut(cx) else {
        return;
    };
    found.drawn.push(id.to_owned());
    let mark = (place, target);
    let was = found.marks.insert(id.to_owned(), mark);
    if was.is_some_and(|was| Some(&was) != found.marks.get(id)) {
        log::info!(target: "sessionView", "link chip {id} changed: its ask ends");
        found.opening.remove(id);
    }
}

/// After the rows: a chip no longer drawn has unmounted, which ends what it asked.
pub fn end(cx: &mut App) {
    if let Some(found) = held_mut(cx) {
        let Found {
            opening,
            drawn,
            marks,
            ..
        } = found;
        opening.retain(|id, _| drawn.contains(id));
        marks.retain(|id, _| drawn.contains(id));
    }
}

/// A message folded: its link chips unmount at once.
pub fn folded(key: &str, cx: &mut App) {
    let prefix = format!("{key}-chip-");
    if let Some(found) = held_mut(cx) {
        found.opening.retain(|id, _| !id.starts_with(&prefix));
    }
}

/// A read a chip still lacks.
#[derive(Debug, PartialEq)]
pub enum Want {
    Title(String),
    Text(String, String),
}

/// `compactMessageLinkLabel`: one line of 240 UTF-16 units at most.
fn compact(text: &str) -> String {
    let words: Vec<&str> = text.split(js_space).filter(|w| !w.is_empty()).collect();
    let line = words.join(" ");
    let units: Vec<u16> = line.encode_utf16().collect();
    match units.len() <= 240 {
        true => line,
        false => format!("{}\u{2026}", String::from_utf16_lossy(&units[..239])),
    }
}

fn explicit(label: Option<&str>) -> Option<&str> {
    label.map(js_trim).filter(|label| !label.is_empty())
}

/// `SessionLinkChip`; `None` for a link that names no task, which stays words.
pub fn session(
    href: &str,
    label: Option<&str>,
    (found, t): (&Found, &Translator),
    wants: &mut Vec<Want>,
) -> Option<Chip> {
    let target = deep_link::parse_session(href)?;
    let (task, label) = (target.session_id, explicit(label));
    if let Some(message) = target.message_client_id {
        let text = label.map(str::to_owned).unwrap_or_else(|| {
            let key = (task, message);
            let known = found.texts.get(&key);
            let fallback = || t.t("chat.userMessage.messageLinkFallback", &[]);
            let full = known.cloned().flatten().unwrap_or_else(fallback);
            if known.is_none() {
                wants.push(Want::Text(key.0, key.1));
            }
            compact(&full)
        });
        let said = Quote {
            text,
            ..Default::default()
        };
        let act = Act::Session(href.to_owned().into());
        return Some(chip::quote(&said, Stand::Link).acts(act));
    }
    let display = label.map(str::to_owned).unwrap_or_else(|| {
        // A task of another device has no row here; its mirror comes with the device link.
        let known = found
            .titles
            .get(&task)
            .filter(|_| target.device_id.is_none());
        let title = known.cloned().flatten();
        if known.is_none() && target.device_id.is_none() {
            wants.push(Want::Title(task));
        }
        match title {
            // `projectDraftSessionTitle`.
            Some(title) if title == DRAFT => t.t("ccAgent.common.unnamedSession", &[]),
            Some(title) => title,
            None => t.t("chat.userMessage.sessionLinkFallback", &[]),
        }
    });
    let act = Act::Session(href.to_owned().into());
    Some(chip::link(&display, IconName::CornerDownRight.svg()).acts(act))
}

/// `projectDisplayName`: the folder's last name.
fn folder(dir: &str) -> &str {
    let trimmed = dir.trim_end_matches(['/', '\\']);
    match trimmed.rsplit(['/', '\\']).next() {
        Some(base) if !base.is_empty() => base,
        _ => dir,
    }
}

/// `ProjectLinkChip`; `None` for a link that names no folder.
pub fn project(href: &str, label: Option<&str>) -> Option<Chip> {
    let dir = deep_link::parse_project(href)?;
    let display = explicit(label).unwrap_or_else(|| folder(&dir));
    let act = Act::Project(href.to_owned().into());
    Some(chip::link(display, IconName::FolderOpen.svg()).acts(act))
}

/// A field that holds text which is not empty.
fn word<'a>(value: &'a Js, key: &str) -> Option<&'a str> {
    value.obj()?.get(key)?.str().filter(|text| !text.is_empty())
}

/// `sessionMessageDisplayText`: a user's or an assistant's words, never a tool's payload.
fn display_text(row: &Js) -> Option<String> {
    let said = |text: &str| Some(js_trim(text).to_owned()).filter(|text| !text.is_empty());
    let row = row.obj()?;
    let content = row.get("content")?;
    match row.get("role")?.str()? {
        "user" => {
            let parsed = user_content::parse(content);
            let text = match parsed.quotes_encoded {
                true => said(&chat_quotes::strip_marker_lines(&parsed.text)),
                false => said(&parsed.text),
            };
            let images = parsed
                .images
                .iter()
                .filter_map(|file| word(file, "originalName"));
            let files = parsed.files.iter().filter_map(|file| word(file, "name"));
            let names: Vec<&str> = images.chain(files).collect();
            text.or_else(|| (!names.is_empty()).then(|| names.join(" \u{b7} ")))
        }
        "assistant" => match content {
            Js::Str(text) => said(text),
            Js::Arr(blocks) => {
                let typed = |block: &&Js| word(block, "type") == Some("text");
                let texts = blocks.iter().filter(typed);
                said(
                    &texts
                        .filter_map(|block| word(block, "text"))
                        .collect::<String>(),
                )
            }
            _ => None,
        },
        _ => None,
    }
}

/// The mounted task's reads.
pub fn held(cx: &App) -> &Found {
    let shown = cx.global::<SessionView>().shown.as_ref();
    let content = shown.and_then(|shown| shown.content.as_ref());
    &content.expect("a row draws in mounted content").links
}

/// The open the chip `id` asked last, by its number.
#[cfg(test)]
pub fn asked(id: &str, cx: &App) -> Option<u64> {
    held(cx).opening.get(id).copied()
}

pub(super) fn held_mut(cx: &mut App) -> Option<&mut Found> {
    let shown = cx.global_mut::<SessionView>().shown.as_mut();
    shown?.content.as_mut().map(|content| &mut content.links)
}

/// `resolveSessionMessageText` on the database worker.
fn read_text(task: String, message: String, cx: &mut App) {
    let State::Ready(db) = &cx.global::<LocalDb>().state else {
        log::error!(target: "sessionView", "link text {task}/{message}: DbClient not ready");
        return;
    };
    let labels = TrustLabels::new(cx.global::<Translator>());
    let (session, id) = (task.clone(), format!("{LIVE}{message}"));
    let answer = db.call(move |conn| {
        let row = rows::anchor(conn, &session, &id, false, &labels);
        row.map(|row| display_text(&row)).map_err(|e| e.to_string())
    });
    cx.spawn(async move |cx| {
        let text = match answer.await {
            Ok(Ok(text)) => text,
            Ok(Err(error)) => {
                log::warn!(target: "sessionView", "link text {task}/{message}: {error}");
                None
            }
            Err(_) => {
                log::error!(target: "sessionView", "link text {task}/{message}: database closed");
                None
            }
        };
        log::info!(target: "sessionView", "link text {task}/{message}: {}", text.is_some());
        cx.update(|cx| {
            let slot = held_mut(cx).and_then(|found| found.texts.get_mut(&(task, message)));
            if let Some(slot) = slot {
                *slot = text;
            }
            cx.refresh_windows();
        });
    })
    .detach();
}

/// Reads what the chips lack, once per mounted task; an answer redraws them.
pub fn fetch(wants: Vec<Want>, cx: &mut App) {
    for want in wants {
        let Some(found) = held_mut(cx) else {
            return;
        };
        match want {
            Want::Title(task) => {
                if found.titles.contains_key(&task) {
                    continue;
                }
                found.titles.insert(task.clone(), None);
                super::get(task.clone(), cx, move |row, cx| {
                    let title = row.ok().flatten().map(|s| js_trim(&s.title).to_owned());
                    let slot = held_mut(cx).and_then(|found| found.titles.get_mut(&task));
                    if let Some(slot) = slot {
                        *slot = title.filter(|title| !title.is_empty());
                    }
                });
            }
            Want::Text(task, message) => {
                let key = (task.clone(), message.clone());
                if found.texts.contains_key(&key) {
                    continue;
                }
                found.texts.insert(key, None);
                read_text(task, message, cx);
            }
        }
    }
}

#[cfg(test)]
mod tests;
