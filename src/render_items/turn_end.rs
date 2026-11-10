//! What `buildRenderItems` adds when a turn ends: hidden subagent media and the files made.

use std::collections::HashSet;

use indexmap::IndexMap;

use super::{
    Build, Item, client_id, content, created_ms, field, is_completed_assistant,
    is_subagent_internal, role, text, time,
};
use crate::{
    generated_files::{self, File},
    local_path::{basename, drive, resolve_tool},
    rich_answer::js::{Js, truthy},
    tool_media, tool_use,
};

/// `pathKey`: separators as `/`, a Windows shape in lower case.
fn path_key(value: &str) -> String {
    let normalized = value.replace('\\', "/");
    match drive(value) || value.contains('\\') {
        true => normalized.to_lowercase(),
        false => normalized,
    }
}

/// `isGeneratedFilesTurnSealed`: nothing works after the turn's last terminal row.
pub fn is_turn_sealed(slice: &[&Js], has_following_user: bool) -> bool {
    if has_following_user {
        return true;
    }
    let terminal = |row: &&Js| {
        field(row, "turnCompleted") == Some(&Js::Bool(false)) || is_completed_assistant(row)
    };
    let Some(last) = slice.iter().rposition(terminal) else {
        return false;
    };
    !slice[last + 1..].iter().any(|row| match role(row) {
        "tool_use" => true,
        "user" => field(row, "isSyntheticTrigger") == Some(&Js::Bool(true)),
        "assistant" => !truthy(field(row, "systemCardType")),
        _ => false,
    })
}

impl<'a> Build<'a> {
    /// `originalTurnSlice`: the visible turn with the subagent rows it hides.
    fn turn_slice(&self, lo: usize, hi: usize) -> Vec<&'a Js> {
        let end = self.visible.get(hi).copied().unwrap_or(self.all.len());
        let rows = &self.all[self.visible[lo]..end];
        rows.iter().map(|(_, row)| *row).collect()
    }

    /// The media of the turn's hidden subagent results.
    fn hidden_media(&mut self, lo: usize, slice: &[&Js], anchor: &str) {
        let mut seen = HashSet::new();
        let mut items = Vec::new();
        let hidden = |row: &&&Js| role(row) == "tool_result" && is_subagent_internal(row);
        for row in slice.iter().filter(hidden) {
            for item in tool_media::extract(content(row)) {
                if !self.turn_shows(lo, &item.url) && seen.insert(item.url.clone()) {
                    items.push(item);
                }
            }
        }
        if !items.is_empty() {
            let key = format!("subagent-media-{anchor}");
            self.items.push(Item::ToolMedia { key, items });
        }
    }

    /// The turn's new files: the history's artifacts, then what its rows record.
    fn generated(&self, lo: usize, hi: usize, slice: &[&Js]) -> Vec<File> {
        let dir = self.opts.working_dir;
        let mut files: IndexMap<String, File> = IndexMap::new();
        if dir.is_empty() {
            return Vec::new();
        }
        let start = created_ms(self.row(lo));
        let end = self.visible.get(hi).and_then(|_| created_ms(self.row(hi)));
        let in_turn = |artifact: &&Js| {
            let at = time(field(artifact, "createdAt")).unwrap_or(f64::NAN);
            !(start.is_some_and(|start| at < start) || end.is_some_and(|end| at >= end))
        };
        let artifacts: Vec<&Js> = self.opts.history_artifacts.iter().filter(in_turn).collect();
        let key = |artifact: &Js| {
            let path = resolve_tool(text(artifact, "path").unwrap_or_default(), dir);
            (path_key(&path), path)
        };
        let ready = |artifact: &Js| truthy(field(artifact, "ready"));
        let edits = artifacts
            .iter()
            .filter(|a| truthy(field(a, "exclude")) && ready(a));
        let edited: HashSet<String> = edits.map(|a| key(a).0).collect();
        for artifact in &artifacts {
            let (normalized, path) = key(artifact);
            let from_command = text(artifact, "source") == Some("command");
            if truthy(field(artifact, "exclude")) {
                if text(artifact, "exclude") == Some("all") && ready(artifact) {
                    files.shift_remove(&normalized);
                }
                continue;
            }
            let tool_made = files.get(&normalized).is_some_and(|f| !f.from_command);
            if from_command && (edited.contains(&normalized) || tool_made) {
                continue;
            }
            let file = File {
                name: basename(&path).to_owned(),
                path,
                from_command,
                ready: field(artifact, "ready") != Some(&Js::Bool(false)),
                artifact: None,
                artifact_confirmed: false,
            };
            files.insert(normalized, file);
        }
        for file in generated_files::collect(slice, dir) {
            files.insert(path_key(&file.path), file);
        }
        for row in slice.iter().filter(|row| role(row) == "tool_use") {
            let name = text(row, "toolName").unwrap_or_default();
            let tool = tool_use::describe(name, field(row, "toolInput"));
            for raw in tool.source_candidates() {
                files.shift_remove(&path_key(&resolve_tool(raw, dir)));
            }
        }
        files.into_values().collect()
    }

    /// `flushTurnEnd` for the turn of visible rows `lo..hi`.
    pub(super) fn flush_turn_end(&mut self, lo: usize, hi: usize) {
        if hi <= lo || !truthy(field(self.row(lo), "clientId")) {
            return;
        }
        let anchor = client_id(self.row(lo));
        let slice = self.turn_slice(lo, hi);
        self.hidden_media(lo, &slice, &anchor);
        let files = self.generated(lo, hi, &slice);
        if files.is_empty() {
            return;
        }
        let starts = slice.iter().filter_map(|row| created_ms(row));
        self.items.push(Item::GeneratedFiles {
            key: format!("genfiles-{anchor}"),
            files,
            turn_start_ms: starts.reduce(f64::min),
            turn_end_ms: self.visible.get(hi).and_then(|_| created_ms(self.row(hi))),
            turn_sealed: is_turn_sealed(&slice, hi < self.visible.len()),
        });
    }
}
