//! `generatedFiles.ts`'s `collectGeneratedFiles`: the files a turn's tool calls made.

use std::collections::{HashMap, HashSet};

use indexmap::IndexMap;

use crate::{
    chat_message::merge::{self, Fields},
    command_output,
    local_path::{basename, drive, resolve_tool},
    rich_answer::js::Js,
    tool_use::{self, Kind, ToolUse},
};

mod artifact;
mod cache;

pub use artifact::{Artifact, failed};
pub use cache::collect_cached;

/// `GeneratedFileRef`: `ready` is false while the call that makes it still runs.
#[derive(Clone, Debug, PartialEq)]
pub struct File {
    pub path: String,
    pub name: String,
    pub from_command: bool,
    pub ready: bool,
    pub artifact: Option<Artifact>,
    pub artifact_confirmed: bool,
}

/// Runs of backslashes fold to one; the run a path starts with keeps `lead`.
fn fold(text: &str, lead: usize) -> String {
    let (mut out, mut run, mut cap) = (String::with_capacity(text.len()), 0, lead);
    for c in text.chars() {
        match c {
            '\\' => run += 1,
            _ => (run, cap) = (0, 1),
        }
        if run <= cap {
            out.push(c);
        }
    }
    out
}

/// `dedupeKeyForPath`: a Windows shape folds case and separators, a POSIX path stays.
fn key(abs: &str) -> String {
    match drive(abs) || abs.contains('\\') {
        true => fold(&abs.replace('/', "\\"), 2).to_lowercase(),
        false => abs.to_owned(),
    }
}

/// `canonicalizeWindowsShape`.
fn canonical(abs: String) -> String {
    match drive(&abs) {
        true => fold(&abs.replace('/', "\\"), 1),
        false => abs,
    }
}

fn text<'a>(row: &'a Fields, key: &str) -> Option<&'a str> {
    merge::get(row, key)?.str().filter(|text| !text.is_empty())
}

/// A `tool_use` row with its descriptor and the result that answers it.
struct Call<'a> {
    name: &'a str,
    input: Option<&'a Js>,
    result: Option<&'a str>,
    ready: bool,
    failed: bool,
    tool: ToolUse,
}

struct Turn<'a> {
    dir: &'a str,
    edited: HashSet<String>,
    files: IndexMap<String, File>,
}

impl Turn<'_> {
    fn abs(&self, raw: &str) -> String {
        canonical(resolve_tool(raw, self.dir))
    }

    fn key(&self, raw: &str) -> String {
        key(&self.abs(raw))
    }

    /// `addPath`: a tool's record outranks a command's guess.
    fn add(&mut self, call: &Call, raw: &str, from_command: bool) {
        if call.failed {
            return;
        }
        let abs = self.abs(raw);
        let key = key(&abs);
        if from_command && self.edited.contains(&key) {
            return;
        }
        if let Some(prev) = self.files.get_mut(&key) {
            prev.from_command &= from_command;
            prev.ready |= call.ready;
            return;
        }
        let file = File {
            name: basename(&abs).to_owned(),
            path: abs,
            from_command,
            ready: call.ready,
            artifact: None,
            artifact_confirmed: false,
        };
        self.files.insert(key, file);
    }

    /// A document tool's `outPath` with what the tool says of it.
    fn document(&mut self, call: &Call) {
        let Some(artifact) = artifact::extract(call.name, call.input, call.result) else {
            return;
        };
        let out = call.input.and_then(Js::obj).and_then(|o| o.get("outPath"));
        let Some(out) = out.and_then(Js::str).filter(|out| !out.is_empty()) else {
            return;
        };
        let abs = self.abs(out);
        let parsed = artifact::parse_result(call.result);
        let confirmed = parsed.is_some_and(|r| r.get("ok") == Some(&Js::Bool(true)));
        match self.files.get_mut(&key(&abs)) {
            Some(existing) if call.ready && !call.failed => {
                existing.artifact = Some(artifact);
                existing.artifact_confirmed |= confirmed;
                existing.ready = true;
            }
            Some(_) => {}
            None => {
                let file = File {
                    name: basename(&abs).to_owned(),
                    from_command: false,
                    ready: call.ready,
                    artifact: Some(artifact),
                    artifact_confirmed: confirmed,
                    path: abs.clone(),
                };
                self.files.insert(key(&abs), file);
            }
        }
    }
}

fn call<'a>(row: &'a Fields, results: &HashMap<&'a str, &'a str>) -> Option<Call<'a>> {
    let name = text(row, "toolName").filter(|_| text(row, "role") == Some("tool_use"))?;
    let input = merge::get(row, "toolInput");
    let id = text(row, "toolUseId");
    let result = id.and_then(|id| results.get(id).copied());
    Some(Call {
        name,
        input,
        result,
        ready: id.is_none() || result.is_some(),
        failed: failed(result),
        tool: tool_use::describe(name, input),
    })
}

fn calls<'a>(messages: &[&'a Fields]) -> Vec<Call<'a>> {
    let mut results = HashMap::new();
    let answers = |row: &&&Fields| text(row, "role") == Some("tool_result");
    for &row in messages.iter().filter(answers) {
        let content = merge::get(row, "content").and_then(Js::str);
        if let (Some(id), Some(content)) = (text(row, "toolUseId"), content) {
            results.insert(id, content);
        }
    }
    messages
        .iter()
        .filter_map(|row| call(row, &results))
        .collect()
}

/// `collectGeneratedFiles`: new files in order, one per path.
pub fn collect(messages: &[&Fields], working_dir: &str) -> Vec<File> {
    let calls = calls(messages);
    let mut turn = Turn {
        dir: working_dir,
        edited: HashSet::new(),
        files: IndexMap::new(),
    };
    for call in calls.iter().filter(|c| c.ready && !c.failed) {
        let edited: Vec<&str> = match &call.tool.kind {
            Kind::File {
                action: "edit",
                file_path,
                ..
            } => vec![file_path],
            Kind::FileChange(changes) => {
                let kept = changes.iter().filter(|c| c.action != "add");
                kept.map(|c| c.path.as_str()).collect()
            }
            _ => Vec::new(),
        };
        for raw in edited {
            let key = turn.key(raw);
            turn.edited.insert(key);
        }
    }
    for call in &calls {
        for raw in call.tool.created_paths() {
            turn.add(call, raw, false);
        }
        turn.document(call);
        match &call.tool.kind {
            Kind::Command { command, .. } => {
                for raw in command_output::candidates(command) {
                    turn.add(call, &raw, true);
                }
            }
            Kind::FileChange(changes) if call.ready && !call.failed => {
                let gone = |c: &&tool_use::Change| matches!(c.action, "delete" | "move");
                for change in changes.iter().filter(gone) {
                    let key = turn.key(&change.path);
                    turn.files.shift_remove(&key);
                }
            }
            _ => {}
        }
    }
    for call in &calls {
        for raw in call.tool.source_candidates() {
            let key = turn.key(raw);
            turn.files.shift_remove(&key);
        }
    }
    turn.files.into_values().collect()
}

#[cfg(test)]
pub(crate) mod tests;
