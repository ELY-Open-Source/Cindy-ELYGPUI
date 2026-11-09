//! `extractConverterOutputPaths`: outputs a converter's semantics fix, named or not.

use std::sync::LazyLock;

use regex::Regex;

use super::{EXT, TEMP_DIR, arguments, ends_with_separator, js_re::regex, unquote};
use crate::rich_answer::js::utf16_len;

static HEADLESS_BROWSER: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r#"--(?:print-to-pdf|screenshot)=(?:"([^"\r\n]+)"|'([^'\r\n]+)'|([^\s"'<>|]+))"#,
        true,
    )
});
static LIBREOFFICE: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r"^(?:soffice(?:\.bin)?|libreoffice)(?:\.(?:exe|com))?$",
        true,
    )
});
static POSITIONAL_OUTPUT: LazyLock<Regex> = LazyLock::new(|| {
    regex(
        r"^(?:wkhtmltopdf|wkhtmltoimage|weasyprint)(?:\.exe)?$",
        true,
    )
});
static SEGMENTS: LazyLock<Regex> = LazyLock::new(|| regex(r";|\r?\n|&&|\|\||\|", false));
static EXTENSION: LazyLock<Regex> = LazyLock::new(|| regex(r"^[a-z0-9]{1,8}$", false));

/// `commandArgumentBasename`.
fn basename(value: &str) -> String {
    let unquoted = unquote(value);
    unquoted
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(unquoted)
        .to_lowercase()
}

/// `fileStem`: the last name without its last extension.
fn stem(value: &str) -> &str {
    let name = value
        .trim_end_matches(['\\', '/'])
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(value);
    match name.rfind('.') {
        Some(dot) if dot > 0 => &name[..dot],
        _ => name,
    }
}

/// `isSynthesizedOutputCandidate`.
fn synthesized(value: &str) -> bool {
    let s = super::trim(value);
    (3..=512).contains(&utf16_len(s))
        && EXT.is_match(s)
        && !TEMP_DIR.is_match(s)
        && !s.contains(['<', '>', '|', '?', '*'])
}

/// `libreOfficeConvertOutputs`.
fn libreoffice(args: &[String]) -> Vec<String> {
    let Some(exec) = args.iter().position(|a| LIBREOFFICE.is_match(&basename(a))) else {
        return Vec::new();
    };
    let (mut convert_to, mut out_dir, mut inputs): (Option<&str>, Option<&str>, Vec<&str>) =
        (None, None, Vec::new());
    let mut index = exec + 1;
    while index < args.len() {
        let arg = args[index].as_str();
        if !arg.starts_with('-') {
            if EXT.is_match(arg) {
                inputs.push(arg);
            }
            index += 1;
            continue;
        }
        let (name, inline) = match arg.find('=') {
            Some(eq) => (&arg[..eq], Some(&arg[eq + 1..])),
            None => (arg, None),
        };
        let name = name.trim_start_matches('-').to_lowercase();
        if name == "convert-to" || name == "outdir" {
            let value = inline.or_else(|| args.get(index + 1).map(String::as_str));
            if inline.is_none() {
                index += 1;
            }
            match name.as_str() {
                "convert-to" => convert_to = value,
                _ => out_dir = value,
            }
        }
        index += 1;
    }
    let Some(convert_to) = convert_to.filter(|c| !c.is_empty()) else {
        return Vec::new();
    };
    if inputs.is_empty() {
        return Vec::new();
    }
    let ext = super::trim(convert_to.split(':').next().unwrap_or("")).to_lowercase();
    if !EXTENSION.is_match(&ext) {
        return Vec::new();
    }
    let dir = match out_dir.filter(|d| !d.is_empty()) {
        Some(d) if ends_with_separator(d) => d.to_owned(),
        Some(d) => format!("{d}/"),
        None => String::new(),
    };
    inputs
        .iter()
        .map(|input| format!("{dir}{}.{ext}", stem(input)))
        .collect()
}

/// `positionalConverterOutputs`: the last positional argument.
fn positional(args: &[String]) -> Vec<String> {
    let Some(exec) = args
        .iter()
        .position(|a| POSITIONAL_OUTPUT.is_match(&basename(a)))
    else {
        return Vec::new();
    };
    let positionals: Vec<&String> = args[exec + 1..]
        .iter()
        .filter(|a| !a.starts_with('-'))
        .collect();
    match positionals.len() {
        0 | 1 => Vec::new(),
        n => vec![positionals[n - 1].clone()],
    }
}

/// `extractConverterOutputPaths`.
pub(super) fn outputs(command: &str) -> Vec<String> {
    if command.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<String> = HEADLESS_BROWSER
        .captures_iter(command)
        .filter_map(|m| {
            m.get(1)
                .or(m.get(2))
                .or(m.get(3))
                .map(|v| v.as_str().to_owned())
        })
        .filter(|v| !v.is_empty())
        .collect();
    for segment in SEGMENTS.split(command) {
        let args: Vec<String> = arguments(segment, 0)
            .into_iter()
            .map(|a| a.value)
            .filter(|v| !v.is_empty())
            .collect();
        if args.is_empty() {
            continue;
        }
        out.extend(libreoffice(&args));
        out.extend(positional(&args));
    }
    out.retain(|v| synthesized(v));
    out
}
