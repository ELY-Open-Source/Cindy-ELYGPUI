//! `toolUseDescriptor.ts`'s `describeToolUse`, kept to what the row label reads.

use crate::{
    command_display,
    command_intent::{self, Intent, base_or},
    message_rows::unwrap_gateway,
    rich_answer::js::{Js, Obj, entries, non_empty, utf16_len},
    theme_import::color::{js_space, js_trim},
};

/// `MCP_OUTPUT_PATH_KEYS`: where an MCP tool names the file it makes.
const OUTPUT_PATH_KEYS: [&str; 4] = ["outPath", "out_path", "outputPath", "output_path"];

/// `MCP_SOURCE_CANDIDATE_MAX_CHARS`, in UTF-16 units.
const SOURCE_MAX_CHARS: usize = 512;

/// `ParsedToolName`; a plain name is the caller's own.
#[derive(Debug, PartialEq)]
pub enum Parsed {
    Plain,
    Mcp {
        server: String,
        tool: String,
    },
    Dynamic {
        namespace: Option<String>,
        tool: String,
    },
    Collab {
        tool: String,
    },
}

/// `parseToolName`: MCP, dynamic and collab names split into their parts.
pub fn parse_name(name: &str) -> Parsed {
    if let Some(rest) = name.strip_prefix("mcp__") {
        if let Some((server, tool)) = rest.split_once("__")
            && !server.is_empty()
            && !tool.is_empty()
        {
            return mcp(server, tool);
        }
        return Parsed::Plain;
    }
    let parts: Vec<&str> = name.split(':').collect();
    let tail = |from: usize| parts.get(from..).map(|p| p.join(":")).unwrap_or_default();
    match parts.as_slice() {
        ["mcp", server, _, ..] if !server.is_empty() && !tail(2).is_empty() => {
            mcp(server, &tail(2))
        }
        ["dynamic", ns, _, ..] if !ns.is_empty() && !tail(2).is_empty() => Parsed::Dynamic {
            namespace: Some((*ns).to_owned()),
            tool: tail(2),
        },
        ["dynamic", tool] if !tool.is_empty() => Parsed::Dynamic {
            namespace: None,
            tool: (*tool).to_owned(),
        },
        ["collab", ..] if !tail(1).is_empty() => Parsed::Collab { tool: tail(1) },
        _ => Parsed::Plain,
    }
}

fn mcp(server: &str, tool: &str) -> Parsed {
    Parsed::Mcp {
        server: server.to_owned(),
        tool: tool.to_owned(),
    }
}

/// `humanizeToolToken`: underscores and whitespace runs as one space, trimmed.
pub fn humanize(token: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in token.chars() {
        if c == '_' || js_space(c) {
            gap = true;
            continue;
        }
        if gap && !out.is_empty() {
            out.push(' ');
        }
        gap = false;
        out.push(c);
    }
    out
}

/// `truncateToolText`: past `max` UTF-16 units, cut to `max - 3` and add `...`.
pub fn truncate(text: &str, max: usize) -> String {
    let text = js_trim(text);
    if text.encode_utf16().count() <= max {
        return text.to_owned();
    }
    let keep = max.saturating_sub(3);
    let mut out = String::new();
    let mut units = 0;
    for c in text.chars() {
        if units + c.len_utf16() > keep {
            // A pair cut in half shows as Chromium shows a lone surrogate.
            if units < keep {
                out.push('\u{FFFD}');
            }
            break;
        }
        units += c.len_utf16();
        out.push(c);
    }
    let kept = out.trim_end_matches(js_space).len();
    out.truncate(kept);
    out + "..."
}

/// A `file_change` entry, as its label shows it.
#[derive(Debug, PartialEq)]
pub struct Change {
    pub action: &'static str,
    pub path: String,
    pub file_name: String,
    pub move_file_name: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum Kind {
    Command {
        description: Option<String>,
        command: String,
        intent: Option<Intent>,
    },
    File {
        action: &'static str,
        file_path: String,
        file_name: String,
    },
    FileChange(Vec<Change>),
    Search {
        pattern: String,
    },
    Web {
        fetch: bool,
        target: String,
    },
    Todo,
    Task {
        description: Option<String>,
    },
    Mcp {
        server: String,
        tool_label: String,
        created_path: Option<String>,
        source_candidates: Vec<String>,
    },
    Dynamic {
        namespace: Option<String>,
        tool_label: String,
    },
    Collab {
        tool_label: String,
    },
    Generic,
}

/// `ToolUseDescriptor`.
#[derive(Debug, PartialEq)]
pub struct ToolUse {
    pub tool_name: String,
    pub kind: Kind,
}

/// `describeToolUse`.
pub fn describe(name: &str, input: Option<&Js>) -> ToolUse {
    let (tool_name, input) = unwrap_gateway(name, input);
    let inp = input.as_ref().and_then(Js::obj);
    let kind = match parse_name(&tool_name) {
        Parsed::Mcp { server, tool } => {
            let created_path = OUTPUT_PATH_KEYS
                .iter()
                .find_map(|key| non_empty(inp.and_then(|o| o.get(*key))));
            Kind::Mcp {
                server,
                tool_label: humanize(&tool),
                source_candidates: created_path.map_or_else(Vec::new, |made| sources(inp, made)),
                created_path: created_path.map(str::to_owned),
            }
        }
        Parsed::Dynamic { namespace, tool } => Kind::Dynamic {
            namespace,
            tool_label: humanize(&tool),
        },
        Parsed::Collab { tool } => Kind::Collab {
            tool_label: humanize(&tool),
        },
        Parsed::Plain => plain(&tool_name, inp),
    };
    ToolUse { tool_name, kind }
}

/// `mcpSourceCandidates`: the top-level strings a call that makes a file may have read.
fn sources(inp: Option<&Obj>, created_path: &str) -> Vec<String> {
    let values = inp.into_iter().flat_map(entries);
    values
        .filter_map(|(_, value)| non_empty(Some(value)))
        .filter(|text| *text != created_path && utf16_len(text) <= SOURCE_MAX_CHARS)
        .map(str::to_owned)
        .collect()
}

impl ToolUse {
    /// `createdPathsFromDescriptor`: the files this call made new.
    pub fn created_paths(&self) -> Vec<&str> {
        match &self.kind {
            Kind::File {
                action: "create",
                file_path,
                ..
            } => vec![file_path],
            Kind::FileChange(changes) => {
                let added = changes.iter().filter(|c| c.action == "add");
                added.map(|c| c.path.as_str()).collect()
            }
            Kind::Mcp {
                created_path: Some(path),
                ..
            } => vec![path],
            _ => Vec::new(),
        }
    }

    /// `sourcePathCandidatesFromDescriptor`.
    pub fn source_candidates(&self) -> &[String] {
        match &self.kind {
            Kind::Mcp {
                source_candidates, ..
            } => source_candidates,
            _ => &[],
        }
    }
}

fn plain(name: &str, inp: Option<&Obj>) -> Kind {
    let field = |key: &str| non_empty(inp.and_then(|o| o.get(key))).map(str::to_owned);
    let or_generic = |kind: Option<Kind>| kind.unwrap_or(Kind::Generic);
    match name {
        "Bash" | "bash" => {
            let description = field("description");
            let command = field("command").unwrap_or_default();
            let intent = description
                .is_none()
                .then(|| command_intent::from_command(&command))
                .flatten();
            Kind::Command {
                description,
                command,
                intent,
            }
        }
        "exec" => {
            let raw = field("command").unwrap_or_default();
            let command = field("displayCommand")
                .or_else(|| command_display::normalize(&raw))
                .unwrap_or(raw);
            let actions = inp.and_then(|o| o.get("commandActions"));
            let intent = command_intent::from_actions(actions, Some(&command))
                .or_else(|| command_intent::from_command(&command));
            Kind::Command {
                description: None,
                command,
                intent,
            }
        }
        "file_change" => or_generic(file_changes(inp).map(Kind::FileChange)),
        "Read" | "read" | "ls" => or_generic(file("read", field("file_path").or(field("path")))),
        "Edit" | "MultiEdit" | "edit" => {
            or_generic(file("edit", field("file_path").or(field("path"))))
        }
        "Write" | "write" => or_generic(file("create", field("file_path").or(field("path")))),
        "Grep" | "Glob" | "grep" | "find" => {
            or_generic(field("pattern").map(|pattern| Kind::Search { pattern }))
        }
        "WebFetch" => or_generic(field("url").map(|target| Kind::Web {
            fetch: true,
            target,
        })),
        "WebSearch" | "web_search" => or_generic(field("query").map(|target| Kind::Web {
            fetch: false,
            target,
        })),
        "TodoWrite" | "update_plan" => Kind::Todo,
        "Task" | "Agent" => Kind::Task {
            description: field("description"),
        },
        _ => Kind::Generic,
    }
}

fn file(action: &'static str, path: Option<String>) -> Option<Kind> {
    let path = path?;
    Some(Kind::File {
        action,
        file_name: base_or(&path).to_owned(),
        file_path: path,
    })
}

/// `fileChangeDescriptor`: every change whole, else none.
fn file_changes(inp: Option<&Obj>) -> Option<Vec<Change>> {
    let changes = inp?.get("changes")?.arr().filter(|c| !c.is_empty())?;
    changes.iter().map(change).collect()
}

fn change(raw: &Js) -> Option<Change> {
    let change = raw.obj()?;
    let kind = change.get("kind")?.obj()?;
    let path = non_empty(change.get("path"))?;
    let kind_type = non_empty(kind.get("type"))?;
    change.get("diff")?.str()?;
    let moved = ["move_path", "movePath"]
        .iter()
        .find_map(|key| non_empty(kind.get(*key)))
        .or_else(|| {
            ["move_path", "movePath"]
                .iter()
                .find_map(|key| non_empty(change.get(*key)))
        });
    let action = match kind_type {
        _ if moved.is_some() => "move",
        "add" => "add",
        "delete" => "delete",
        "update" => "update",
        _ => "unknown",
    };
    Some(Change {
        action,
        path: path.to_owned(),
        file_name: base_or(path).to_owned(),
        move_file_name: moved.map(|m| base_or(m).to_owned()),
    })
}

#[cfg(test)]
mod tests;
