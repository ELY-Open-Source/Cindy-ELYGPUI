//! `formatIslandToolDetail` with `createLocalizedToolRowWording`: a tool call in one line.

use crate::{
    command_intent::Intent,
    i18n::{Arg, Translator},
    rich_answer::js::{Js, Obj},
    theme_import::color::js_trim,
    tool_use::{self, Change, Kind, truncate},
};

/// `TOOL_ROW_TEXT_MAX_CHARS`.
const TEXT_MAX: usize = 60;
/// `ISLAND_DETAIL_MAX_CHARS`.
const ISLAND_MAX: usize = 80;

/// `INTENT_ROW_VERB_KEY`: the action's own name, but for nine older verbs.
fn intent_verb(t: &Translator, action: &str) -> String {
    let verb = match action {
        "list" => "listed",
        "search" => "searched",
        "fetch" => "fetched",
        "install" => "installedDeps",
        "test" => "ranTests",
        "build" => "built",
        "lint" => "linted",
        "typecheck" => "typechecked",
        other => other,
    };
    t.t(&format!("chat.agentActionRow.verb.{verb}"), &[])
}

/// `TOOL_ROW_VERB_I18N_KEY`, for the verbs a one-line label uses.
fn verb(t: &Translator, key: &str) -> String {
    let key = match key {
        "edit" => "chat.agentActionRow.verb.edited",
        "create" => "chat.agentActionRow.verb.created",
        "delete" => "chat.agentActionRow.fileChange.deleted",
        "rename" => "chat.agentActionRow.fileChange.renamed",
        "update" => "chat.agentActionRow.verb.updated",
        "search" => "chat.agentActionRow.verb.searched",
        "fetch" => "chat.agentActionRow.verb.fetched",
        "updateTodos" => "agentIsland.native.updatingTasks",
        "read" => "chat.agentActionRow.verb.read",
        other => unreachable!("no row verb {other}"),
    };
    t.t(key, &[])
}

/// `joinVerb`.
fn join(verb: String, target: &str) -> String {
    if target.is_empty() {
        verb
    } else {
        format!("{verb} {target}")
    }
}

fn intent_label(t: &Translator, intent: &Intent) -> String {
    let verb = intent_verb(t, intent.action);
    match &intent.target {
        Some(target) => join(verb, &truncate(target, TEXT_MAX)),
        None => verb,
    }
}

fn change_label(t: &Translator, change: &Change) -> String {
    let key = match change.action {
        "add" => "create",
        "delete" => "delete",
        "move" => "rename",
        "update" => "edit",
        _ => "update",
    };
    let target = match &change.move_file_name {
        Some(to) => format!("{} → {to}", change.file_name),
        None => change.file_name.clone(),
    };
    join(verb(t, key), &target)
}

/// `formatToolRowText(...).label` for the kinds whose label the island shows.
fn label(t: &Translator, tool_name: &str, kind: &Kind) -> String {
    match kind {
        Kind::File { action, file_name } => join(verb(t, action), file_name),
        Kind::FileChange(changes) => match changes.as_slice() {
            [one] => change_label(t, one),
            many => {
                let count = many.len().to_string();
                let files = t.t(
                    "chat.agentActionRow.fileChange.files",
                    &[("count", Arg::Text(&count))],
                );
                format!("{} {files}", verb(t, "update"))
            }
        },
        Kind::Search { pattern } => join(verb(t, "search"), &truncate(pattern, TEXT_MAX)),
        Kind::Web { fetch, target } => {
            let key = if *fetch { "fetch" } else { "search" };
            join(verb(t, key), &truncate(target, TEXT_MAX))
        }
        Kind::Todo => verb(t, "updateTodos"),
        Kind::Task { description } => description.clone().unwrap_or_else(|| tool_name.to_owned()),
        _ => unreachable!("commands and opaque tools take no row label here"),
    }
}

/// `firstNonEmptyString`.
fn first_text<'a>(values: impl IntoIterator<Item = Option<&'a Js>>) -> Option<&'a str> {
    values
        .into_iter()
        .find_map(|v| Some(js_trim(v?.str()?)).filter(|s| !s.is_empty()))
}

/// `permissionQuestionDetail`: a tool-side question, with how many follow.
fn question(input: &Obj) -> Option<String> {
    let questions = input.get("questions")?.arr()?;
    let first = questions.iter().find_map(Js::obj)?;
    let text = first_text([first.get("question"), first.get("header")])?;
    Some(match questions.len() - 1 {
        0 => text.to_owned(),
        extra => format!("{text} (+{extra})"),
    })
}

/// `formatCommandDetail` without a permission prompt's forced command.
fn command_detail(
    t: &Translator,
    description: Option<&str>,
    command: &str,
    intent: Option<&Intent>,
) -> Option<String> {
    let command = js_trim(command);
    let description = description.map(js_trim).unwrap_or("");
    if !description.is_empty() {
        if command.is_empty() || description.contains(command) {
            return Some(description.to_owned());
        }
        return Some(format!("{description} · $ {command}"));
    }
    if let Some(intent) = intent {
        return Some(intent_label(t, intent));
    }
    (!command.is_empty()).then(|| format!("$ {command}"))
}

fn calling(t: &Translator) -> String {
    t.t("agentIsland.native.callingTool", &[])
}

/// `formatIslandToolDetail(name, input, { wording })`, with no fallback metadata.
fn island(t: &Translator, name: &str, input: Option<&Js>) -> Option<String> {
    let Some(fields) = input.and_then(Js::obj) else {
        return Some(truncate(&calling(t), ISLAND_MAX));
    };
    if let Some(question) = question(fields) {
        return Some(question);
    }
    let described = tool_use::describe(name, input);
    let opaque = matches!(
        described.kind,
        Kind::Generic | Kind::Mcp { .. } | Kind::Dynamic { .. } | Kind::Collab { .. }
    );
    if let Kind::Command {
        description,
        command,
        intent,
    } = &described.kind
    {
        return command_detail(t, description.as_deref(), command, intent.as_ref());
    }
    if opaque {
        let message = matches!(described.kind, Kind::Generic).then(|| fields.get("message"));
        let informative = first_text([
            message.flatten(),
            fields.get("toolTitle"),
            fields.get("toolDescription"),
        ]);
        return Some(match informative {
            Some(text) => truncate(text, ISLAND_MAX),
            None => calling(t),
        });
    }
    let label = label(t, &described.tool_name, &described.kind);
    if label.is_empty() {
        return None;
    }
    Some(match described.kind {
        Kind::Task { .. } => truncate(&label, ISLAND_MAX),
        _ => label,
    })
}

/// The candidate's `activity`: the island's line, else the raw tool name.
pub fn activity(t: &Translator, name: &str, input: Option<&Js>) -> String {
    island(t, name, input).unwrap_or_else(|| name.to_owned())
}

#[cfg(test)]
mod tests;
