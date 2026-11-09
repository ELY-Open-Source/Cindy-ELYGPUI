//! `isExplicitOutputPath` and `transferDirectoryOutputs`.

use std::sync::LazyLock;

use regex::Regex;

use super::{
    EXT, Token, after, arguments, before, ends_with_separator, index::Index, js_re::regex,
};
use crate::theme_import::color::js_space;

const WRITERS: &str = "save|savefig|writeFileSync|writeFile|writeAllText|writeAllBytes|createWriteStream|write_text|write_bytes|to_csv|to_excel|to_json|to_parquet|imwrite|imsave|dump";
const PATH_KEYS: &str = "path_or_buf|excel_writer|path|filename|fname|fp|file";

static WRITE_CALL: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        &format!(
            r#"(?:\.|\b)(?:{WRITERS})\s*\(\s*(?:(?:{PATH_KEYS})\s*=\s*)?(?:[rubf]{{0,2}})?['"]$"#
        ),
        true,
    )
});
static WRITE_CALL_LATER_KEYWORD: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        &format!(
            r#"(?:\.|\b)(?:{WRITERS})\s*\(\s*[^();\r\n]+,\s*(?:{PATH_KEYS})\s*=\s*(?:[rubf]{{0,2}})?['"]$"#
        ),
        true,
    )
});
static OBJECT_FIRST_WRITE_CALL: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r#"\b(?:torch\.save|joblib\.dump)\s*\(\s*(?:[^();\r\n]|\([^()]*\))+,\s*(?:[rubf]{0,2})?['"]$"#,
        true,
    )
});
static PATH_WRITE_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| regex(r#"^\s*['"]?\s*\)\s*\.\s*write_(?:text|bytes)\s*\("#, true));
static OUTPUT_OPTION: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r#"(?:^|\s)(?:-o|--output(?:-file|-document)?|--outfile)(?:\s+|=)['"]?$"#,
        true,
    )
});
static REDIRECT: LazyLock<Regex> = LazyLock::new(|| regex(r#"(?:^|[^>])>{1,2}\s*['"]?$"#, false));
static SAVE_COMMAND: LazyLock<Regex> =
    LazyLock::new(|| regex(r#"(?:^|[;&|]\s*|\s)save\s+['"]?$"#, true));
static TEE_COMMAND: LazyLock<Regex> =
    LazyLock::new(|| regex(r#"(?:^|[|;&]\s*)tee(?:\.exe)?\b[^|;&\r\n]*['"]?$"#, true));
static OPEN_CALL: LazyLock<Regex> =
    LazyLock::new(|| regex(r#"\bopen\s*\(\s*(?:[rubf]{0,2})?['"]$"#, true));
static OPEN_MODE: LazyLock<Regex> =
    LazyLock::new(|| regex(r#"^['"]\s*,\s*['"][wax][bt+]*['"]"#, true));
static OPEN_MODE_KEYWORD: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r#"^['"]\s*,\s*[^();\r\n]*\bmode\s*=\s*['"][wax][bt+]*['"]"#,
        true,
    )
});
static UNSAFE: LazyLock<Regex> = LazyLock::new(|| regex(r"[<>|?*]", false));

/// The text before a quote and trailing space, once both are stripped, if any space was.
fn before_switch(text: &str) -> Option<&str> {
    let text = text.strip_suffix(['\'', '"']).unwrap_or(text);
    let body = text.trim_end_matches(js_space);
    (body.len() < text.len()).then_some(body)
}

fn ends_with_any(text: &str, words: &[&str]) -> Option<usize> {
    words
        .iter()
        .find(|w| {
            text.len() >= w.len()
                && text.as_bytes()[text.len() - w.len()..].eq_ignore_ascii_case(w.as_bytes())
        })
        .map(|w| text.len() - w.len())
}

/// `endsWithDestinationSwitch`: `/-(?:Destination|LiteralDestination|Target)\s+['"]?$/i`.
fn ends_with_destination_switch(text: &str) -> bool {
    before_switch(text).is_some_and(|b| {
        ends_with_any(b, &["-LiteralDestination", "-Destination", "-Target"]).is_some()
    })
}

/// `endsWithTargetDirectoryOption`: `/(?:^|\s)(?:-t|--target-directory)(?:\s+|=)['"]?$/i`.
fn ends_with_target_directory_option(text: &str) -> bool {
    let unquoted = text.strip_suffix(['\'', '"']).unwrap_or(text);
    let option = match before_switch(text) {
        Some(body) => body,
        None => match unquoted.strip_suffix('=') {
            Some(body) => body,
            None => return false,
        },
    };
    ends_with_any(option, &["--target-directory", "-t"])
        .is_some_and(|at| option[..at].chars().next_back().is_none_or(js_space))
}

/// `isExplicitOutputPath`.
pub(super) fn is_output(index: &Index, token: &Token) -> bool {
    let command = index.command;
    let prefix = before(command, token.start, 240);
    let suffix = after(command, token.end, 80);
    let prefixed = [
        &*WRITE_CALL,
        &*WRITE_CALL_LATER_KEYWORD,
        &*OBJECT_FIRST_WRITE_CALL,
    ];
    if prefixed.iter().any(|r| r.is_match(&prefix))
        || PATH_WRITE_SUFFIX.is_match(&suffix)
        || index.powershell_output(token.start)
        || [&*OUTPUT_OPTION, &*REDIRECT, &*SAVE_COMMAND, &*TEE_COMMAND]
            .iter()
            .any(|r| r.is_match(&prefix))
    {
        return true;
    }
    if OPEN_CALL.is_match(&prefix)
        && (OPEN_MODE.is_match(&suffix) || OPEN_MODE_KEYWORD.is_match(&suffix))
    {
        return true;
    }
    let segment = index.segment(token.start, token.end);
    let last = segment
        .last_path
        .as_ref()
        .is_some_and(|l| l.start == token.start && l.end == token.end);
    let in_segment = &command[segment.start..token.start];
    if segment.copy_move_item {
        return match segment.explicit_destination {
            true => ends_with_destination_switch(in_segment),
            false => last,
        };
    }
    if segment.target_directory_transfer {
        return ends_with_target_directory_option(in_segment);
    }
    last && segment.transfer_command
}

/// `transferDirectoryOutputs`: a directory target becomes each source's name in it.
pub(super) fn transfer_outputs(index: &Index, destination: &Token) -> Vec<String> {
    if !ends_with_separator(&destination.path) {
        return vec![destination.path.clone()];
    }
    let segment = index.segment(destination.start, destination.end);
    let mut sources: Vec<String> = index
        .within(segment.start, segment.end)
        .filter(|t| t.start != destination.start && !ends_with_separator(&t.path))
        .map(|t| t.path.clone())
        .collect();
    for argument in arguments(&index.command[segment.start..segment.end], segment.start) {
        if argument.start == destination.start
            || !EXT.is_match(&argument.value)
            || UNSAFE.is_match(&argument.value)
        {
            continue;
        }
        if !sources.contains(&argument.value) {
            sources.push(argument.value);
        }
    }
    let outputs: Vec<String> = sources
        .iter()
        .filter_map(|source| {
            let name = source
                .strip_suffix(['\\', '/'])
                .unwrap_or(source)
                .rsplit(['\\', '/'])
                .next()?;
            (!name.is_empty()).then(|| format!("{}{name}", destination.path))
        })
        .collect();
    match outputs.is_empty() {
        true => vec![destination.path.clone()],
        false => outputs,
    }
}
