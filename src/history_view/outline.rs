//! `historyViewOutline.ts`'s outline columns: bodies folded in SQL, before they leave the database.

/// `historyOutlineContent`: folded thinking, tool bodies and child prose; deliveries kept.
pub const CONTENT: &str = r#"CASE
    WHEN "messages"."role" = 'tool_result' AND NOT json_valid("messages"."content")
      AND NOT (length("messages"."content") <= 16384
        AND (instr("messages"."content", 'xdt-file://') > 0 OR instr("messages"."content", 'cindy-media://') > 0))
      THEN json_quote(CASE WHEN instr("messages"."content", '<tool_use_error>') > 0 THEN '<tool_use_error>' ELSE '' END)
    WHEN NOT json_valid("messages"."content") THEN "messages"."content"
    WHEN "messages"."role" = 'thinking' THEN json_object('isRedacted', json(CASE
      WHEN (json_type("messages"."content", '$') = 'text' AND length(json_extract("messages"."content", '$')) > 0)
        OR length(json_extract("messages"."content", '$.text')) > 0 OR json_extract("messages"."content", '$.isRedacted') = 1
      THEN 'true' ELSE 'false' END),
      'durationMs', json_extract("messages"."content", '$.durationMs'))
    WHEN "messages"."role" = 'tool_use' AND (
      COALESCE(json_extract("messages"."content", '$.input.outPath'), json_extract("messages"."content", '$.input.out_path'),
        json_extract("messages"."content", '$.input.outputPath'), json_extract("messages"."content", '$.input.output_path'),
        json_extract("messages"."content", '$.input.args.outPath'), json_extract("messages"."content", '$.input.args.out_path'),
        json_extract("messages"."content", '$.input.args.outputPath'), json_extract("messages"."content", '$.input.args.output_path')) IS NOT NULL
      ) THEN "messages"."content"
    WHEN "messages"."role" = 'tool_use' THEN json_object(
      'toolName', json_extract("messages"."content", '$.toolName'),
      'toolUseId', json_extract("messages"."content", '$.toolUseId'),
      'input', CASE WHEN json_extract("messages"."content", '$.toolName') = 'cindy_mcp_call_tool'
        THEN json_object('server', json_extract("messages"."content", '$.input.server'), 'tool', json_extract("messages"."content", '$.input.tool'))
        ELSE json_object('file_path', json_extract("messages"."content", '$.input.file_path'),
          'path', json_extract("messages"."content", '$.input.path'),
          'command', json_extract("messages"."content", '$.input.command'),
          'displayCommand', json_extract("messages"."content", '$.input.displayCommand'),
          'changes', json(COALESCE((SELECT json_group_array(json_object('path', json_extract(CASE WHEN json_valid(value) THEN value ELSE '{}' END, '$.path'),
            'kind', json_extract(CASE WHEN json_valid(value) THEN value ELSE '{}' END, '$.kind'),
            'move_path', json_extract(CASE WHEN json_valid(value) THEN value ELSE '{}' END, '$.move_path'),
            'movePath', json_extract(CASE WHEN json_valid(value) THEN value ELSE '{}' END, '$.movePath'),
            'diff', CASE WHEN json_type(CASE WHEN json_valid(value) THEN value ELSE '{}' END, '$.diff') = 'text' THEN '' END))
            FROM json_each("messages"."content", '$.input.changes')), '[]')))
        END)
    WHEN "messages"."role" = 'tool_result'
      AND instr("messages"."content", 'xdt_') = 0 AND instr("messages"."content", 'xdt-file:') = 0 AND instr("messages"."content", 'cindy-media:') = 0
      THEN json_quote(CASE WHEN instr("messages"."content", '<tool_use_error>') > 0 OR CASE
        WHEN json_valid(json_extract("messages"."content", '$')) THEN
          json_extract(json_extract("messages"."content", '$'), '$.ok') IS 0
          OR json_extract(json_extract("messages"."content", '$'), '$.success') IS 0
          OR lower(json_extract(json_extract("messages"."content", '$'), '$.status')) IN ('error', 'failed', 'failure')
        ELSE 0 END THEN '<tool_use_error>' ELSE '' END)
    WHEN "messages"."role" = 'assistant'
      AND length(CASE WHEN json_valid("messages"."agent_meta") THEN json_extract("messages"."agent_meta", '$.parentUuid') END) > 0 THEN json_quote('')
    ELSE "messages"."content" END"#;

/// `historyOutlineContentStamp`: length and tail, which change when a body is rewritten.
pub const STAMP: &str = r#"CASE
    WHEN typeof("messages"."content") <> 'text'
      OR instr(CAST("messages"."content" AS BLOB), x'00') > 0
      THEN length("messages"."content") || ':' || substr("messages"."content", -24)
    ELSE length("messages"."content") || ':' || hex(
      substr(CAST("messages"."content" AS BLOB), -96))
    END"#;

#[cfg(test)]
mod tests;
