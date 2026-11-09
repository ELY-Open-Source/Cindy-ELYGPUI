//! `withHistoryArtifacts`: an outline row's files, its bookkeeping and long commands dropped.

use std::sync::{LazyLock, Mutex};

use indexmap::IndexMap;

use super::Row;
use crate::{
    command_output,
    message_rows::tool_use,
    rich_answer::js::{Js, Obj, truthy},
    tool_use::{Kind, describe},
};

/// `COMMAND_ARTIFACT_CACHE_LIMIT`.
const CACHE_LIMIT: usize = 2048;
/// The SDK bookkeeping an outline drops from `agentMeta`.
const BOOKKEEPING: [&str; 5] = [
    "uuid",
    "sdkSessionId",
    "requestId",
    "replyToClientId",
    "usage",
];
/// The command length an outline keeps, in UTF-16 units.
const COMMAND_UNITS: usize = 256;

/// `commandArtifactCache`: least recently used last out.
static CACHE: LazyLock<Mutex<IndexMap<String, Vec<String>>>> = LazyLock::new(Mutex::default);

/// `commandArtifactCacheKey`: the id, the command's length and its FNV-1a over UTF-16 units.
fn cache_key(id: &str, command: &str) -> String {
    let (mut hash, mut length) = (2_166_136_261u32, 0usize);
    for unit in command.encode_utf16() {
        hash = (hash ^ u32::from(unit)).wrapping_mul(16_777_619);
        length += 1;
    }
    format!("{id}\u{0}{length}\u{0}{hash}")
}

/// `cachedCommandArtifacts`.
fn command_paths(id: &str, command: &str) -> Vec<String> {
    let mut cache = CACHE.lock().expect("an unpoisoned artifact cache");
    cached(&mut cache, cache_key(id, command), || {
        command_output::candidates(command)
    })
}

/// A hit moves to the newest end; a miss past the limit drops the oldest.
fn cached(
    cache: &mut IndexMap<String, Vec<String>>,
    key: String,
    paths: impl FnOnce() -> Vec<String>,
) -> Vec<String> {
    if let Some(hit) = cache.shift_remove(&key) {
        cache.insert(key, hit.clone());
        return hit;
    }
    let paths = paths();
    if cache.len() >= CACHE_LIMIT {
        cache.shift_remove_index(0);
    }
    cache.insert(key, paths.clone());
    paths
}

/// An object spread or `Object.entries` of `value`; a cut pair's half in a string reads as U+FFFD.
fn spread(value: &Js) -> Obj {
    let mut out = Obj::new();
    match value {
        Js::Obj(o) => return o.clone(),
        Js::Arr(items) => out.extend(
            items
                .iter()
                .enumerate()
                .map(|(i, v)| (i.to_string(), v.clone())),
        ),
        Js::Str(s) => {
            let units = s.encode_utf16().collect::<Vec<_>>();
            for (i, unit) in units.iter().enumerate() {
                let c = char::from_u32(u32::from(*unit)).unwrap_or('\u{FFFD}');
                out.insert(i.to_string(), Js::Str(c.to_string()));
            }
        }
        _ => {}
    }
    out
}

/// `value.slice(0, 256)` in UTF-16 units; a cut pair's half reads as U+FFFD.
fn head(value: &str) -> String {
    let mut units = 0;
    let mut out = String::new();
    for c in value.chars() {
        let width = c.len_utf16();
        if units + width > COMMAND_UNITS {
            if units < COMMAND_UNITS {
                out.push('\u{FFFD}');
            }
            break;
        }
        units += width;
        out.push(c);
    }
    out
}

fn artifact(file: &File, row: &Obj, tool: Option<&str>) -> Js {
    let mut out = Obj::new();
    out.insert("path".into(), Js::Str(file.path.clone()));
    if let Some(exclude) = file.exclude {
        out.insert("exclude".into(), Js::Str(exclude.into()));
    }
    out.insert("source".into(), Js::Str(file.source.into()));
    if let Some(at) = row.get("createdAt") {
        out.insert("createdAt".into(), at.clone());
    }
    if let Some(tool) = tool {
        out.insert("toolUseId".into(), Js::Str(tool.to_owned()));
    }
    Js::Obj(out)
}

/// One `historyArtifacts` entry before it takes the row's time and tool.
struct File {
    path: String,
    source: &'static str,
    exclude: Option<&'static str>,
}

fn file(path: &str, source: &'static str, exclude: Option<&'static str>) -> File {
    File {
        path: path.to_owned(),
        source,
        exclude,
    }
}

/// `createdPathsFromDescriptor`'s files, then the edits that exclude a command's claim.
fn files(id: &str, kind: &Kind) -> Vec<File> {
    match kind {
        Kind::Command { command, .. } => command_paths(id, command)
            .iter()
            .map(|p| file(p, "command", None))
            .collect(),
        Kind::File {
            action: "create",
            file_path,
            ..
        } => vec![file(file_path, "tool", None)],
        Kind::File {
            action: "edit",
            file_path,
            ..
        } => vec![file(file_path, "tool", Some("command"))],
        Kind::FileChange(changes) => {
            let added = changes
                .iter()
                .filter(|c| c.action == "add")
                .map(|c| file(&c.path, "tool", None));
            let excluded = changes.iter().filter(|c| c.action != "add").map(|c| {
                let exclude = if matches!(c.action, "delete" | "move") {
                    "all"
                } else {
                    "command"
                };
                file(&c.path, "tool", Some(exclude))
            });
            added.chain(excluded).collect()
        }
        Kind::Mcp {
            created_path: Some(path),
            ..
        } => vec![file(path, "tool", None)],
        _ => Vec::new(),
    }
}

/// `withHistoryArtifacts`.
pub fn with_artifacts(mut row: Row) -> Row {
    let Js::Obj(fields) = &mut row else {
        return row;
    };
    let mut fields = std::mem::take(fields);
    if let Some(meta) = fields.get("agentMeta").filter(|m| truthy(Some(m))) {
        let mut rest = spread(meta);
        rest.retain(|k, _| !BOOKKEEPING.contains(&k.as_str()));
        fields.insert("agentMeta".into(), Js::Obj(rest));
    }
    if fields.get("role").and_then(Js::str) != Some("tool_use") {
        return Js::Obj(fields);
    }
    let body = fields.get("content").cloned().unwrap_or(Js::Null);
    let tool = tool_use(fields.get("toolUseId").and_then(Js::str), &body);
    let descriptor = describe(&tool.tool_name, Some(&tool.input));
    let id = fields.get("id").and_then(Js::str).unwrap_or_default();
    let files = files(id, &descriptor.kind);
    if matches!(descriptor.kind, Kind::Command { .. }) {
        let mut outlined = spread(&body);
        let input = spread(&tool.input)
            .into_iter()
            .map(|(k, v)| match (&v, k.as_str()) {
                (Js::Str(s), "command" | "displayCommand") => (k, Js::Str(head(s))),
                _ => (k, v),
            });
        outlined.insert("input".into(), Js::Obj(input.collect()));
        fields.insert("content".into(), Js::Obj(outlined));
    }
    if !files.is_empty() {
        let tool_id = tool.tool_use_id.as_deref();
        let artifacts = files
            .iter()
            .map(|f| artifact(f, &fields, tool_id))
            .collect();
        fields.insert("historyArtifacts".into(), Js::Arr(artifacts));
    }
    Js::Obj(fields)
}

#[cfg(test)]
mod tests;
