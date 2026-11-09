//! `projectHistorySourceView`'s per-row flags.

use super::*;

/// Each row's activity and archivable flags, as `projectHistorySourceView` sets them.
pub(super) fn flags(rows: &[Row], preserve_mixed: bool) -> Vec<It> {
    let mut names = std::collections::HashMap::new();
    let (mut visible, mut settled, mut nested_calls) =
        (HashSet::new(), HashSet::new(), HashSet::new());
    let mcp_app = |r: &Row| truthy(meta(r).and_then(|m| m.get("mcpApp")));
    for row in rows {
        if text(row, "role") == "tool_use" {
            let tool = parsed(row);
            if let Some(id) = tool.tool_use_id {
                if !preserve_mixed && has_file_delivery(row) {
                    visible.insert(id.clone());
                }
                if mcp_app(row) {
                    visible.insert(id.clone());
                }
                if truthy(meta(row).and_then(|m| m.get("parentUuid"))) {
                    nested_calls.insert(id.clone());
                }
                names.insert(id, tool.tool_name);
            }
        }
        if text(row, "role") == "tool_result"
            && let Some(id) = result_id(row)
        {
            settled.insert(id.to_owned());
            if has_visible_history_result(content(row)) || mcp_app(row) {
                visible.insert(id.to_owned());
            }
        }
    }
    let mut out: Vec<It> = rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let role = text(row, "role");
            let m = meta(row);
            let tool = (role == "tool_use").then(|| parsed(row));
            let id = match &tool {
                Some(t) if t.tool_use_id.is_some() => t.tool_use_id.clone(),
                _ => result_id(row).map(str::to_owned),
            };
            let name = match &tool {
                Some(t) => t.tool_name.clone(),
                None => id
                    .as_ref()
                    .and_then(|i| names.get(i))
                    .cloned()
                    .unwrap_or_default(),
            };
            let parent = m
                .and_then(|m| m.get("parentUuid"))
                .and_then(Js::str)
                .is_some_and(|p| !p.is_empty());
            let nested = parent || id.as_ref().is_some_and(|i| nested_calls.contains(i));
            let thinking = role == "thinking"
                && match content(row) {
                    Js::Str(s) => !s.is_empty(),
                    c => {
                        truthy(field(c, "text"))
                            || truthy(field(c, "durationMs"))
                            || field(c, "isRedacted") == Some(&Js::Bool(true))
                    }
                };
            let settled_ghost =
                id.as_ref().is_some_and(|i| settled.contains(i)) && GHOST_CALL.is_match(&name);
            let tool_row = matches!(role, "tool_use" | "tool_result")
                && (is_detail_tool(&name)
                    || (!preserve_mixed && (EDITS.contains(&name.as_str()) || settled_ghost)))
                && !id.as_ref().is_some_and(|i| visible.contains(i));
            let activity = !nested && (thinking || tool_row);
            let flagged = |key: &str| truthy(m.and_then(|m| m.get(key)));
            let plain = !nested
                && role == "assistant"
                && content(row).str().is_some()
                && ![
                    "botCollaboration",
                    "botDirectMessage",
                    "systemCardType",
                    "goalCompletion",
                    "goalNotice",
                    "reviewRun",
                    "pluginModelRoute",
                ]
                .iter()
                .any(|k| flagged(k));
            let delivery = content(row)
                .str()
                .is_some_and(crate::message_rows::is_delivery_prose_text);
            It::Source {
                row: index,
                activity,
                archivable: activity || (plain && !delivery),
            }
        })
        .collect();
    // A mixed tool segment keeps its media in place, so none of it folds.
    let mut start = 0;
    while start < out.len() {
        let tool = |i: &It| matches!(i, It::Source { row, .. } if matches!(text(&rows[*row], "role"), "tool_use" | "tool_result"));
        if !tool(&out[start]) {
            start += 1;
            continue;
        }
        let end = (start..out.len())
            .find(|&i| !tool(&out[i]))
            .unwrap_or(out.len());
        let segment = &mut out[start..end];
        let media = segment.iter().any(|i| match i {
            It::Source { row, .. } => {
                text(&rows[*row], "role") == "tool_result"
                    && has_visible_history_result(content(&rows[*row]))
            }
            It::Group(..) => false,
        });
        if (preserve_mixed || media) && !segment.iter().all(View::activity) {
            for item in segment.iter_mut() {
                if let It::Source {
                    activity,
                    archivable,
                    ..
                } = item
                {
                    (*activity, *archivable) = (false, false);
                }
            }
        }
        start = end;
    }
    out
}
