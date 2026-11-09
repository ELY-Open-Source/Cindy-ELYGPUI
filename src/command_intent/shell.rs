//! The lexical helpers of `commandIntent.ts`: top-level splits, words and prefixes.

use crate::theme_import::color::{js_space, js_trim};

/// `ShellWord`: unquoted text, and whether an unquoted `<`, `>` or `(` or any quote was in it.
pub struct Word {
    pub text: String,
    pub meta: bool,
    pub quoted: bool,
}

/// `splitTopLevel`: segments outside quotes, trimmed and nonempty; none for an open quote.
pub fn split_top_level(input: &str, separators: &[&str]) -> Option<Vec<String>> {
    let chars: Vec<char> = input.chars().collect();
    let at = |i: usize| chars.get(i).copied();
    let starts = |i: usize, sep: &str| sep.chars().enumerate().all(|(k, c)| at(i + k) == Some(c));
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        if let Some(q) = quote {
            current.push(ch);
            if ch == q && (q != '"' || index == 0 || chars[index - 1] != '\\') {
                quote = None;
            }
            index += 1;
            continue;
        }
        if ch == '\\'
            && let Some(next) = at(index + 1)
        {
            current.push(ch);
            current.push(next);
            index += 2;
            continue;
        }
        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            current.push(ch);
            index += 1;
            continue;
        }
        if ch == '|' && at(index + 1) == Some('|') && !separators.contains(&"||") {
            return None;
        }
        let matched = separators.iter().find(|sep| starts(index, sep));
        let fd = index > 0 && chars[index - 1] == '>' || at(index + 1) == Some('>');
        match matched {
            Some(&"&") if fd => current.push(ch),
            Some(sep) => {
                segments.push(std::mem::take(&mut current));
                index += sep.chars().count();
                continue;
            }
            None => current.push(ch),
        }
        index += 1;
    }
    if quote.is_some() {
        return None;
    }
    segments.push(current);
    Some(
        segments
            .iter()
            .map(|s| js_trim(s).to_owned())
            .filter(|s| !s.is_empty())
            .collect(),
    )
}

/// `tokenize`: quote-aware words; none for an open quote.
pub fn tokenize(input: &str) -> Option<Vec<Word>> {
    let chars: Vec<char> = input.chars().collect();
    let mut words = Vec::new();
    let mut word = Word {
        text: String::new(),
        meta: false,
        quoted: false,
    };
    let mut has = false;
    let mut quote: Option<char> = None;
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        let next = chars.get(index + 1).copied();
        index += 1;
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            } else if q == '"' && ch == '\\' && next == Some('"') {
                word.text.push('"');
                index += 1;
            } else {
                word.text.push(ch);
            }
            continue;
        }
        if ch == '"' || ch == '\'' {
            (quote, has, word.quoted) = (Some(ch), true, true);
            continue;
        }
        if ch == '\\'
            && let Some(escaped) = next.filter(|&c| js_space(c) || "\"'\\<>();&|".contains(c))
        {
            word.text.push(escaped);
            has = true;
            index += 1;
            continue;
        }
        if js_space(ch) {
            let done = std::mem::replace(
                &mut word,
                Word {
                    text: String::new(),
                    meta: false,
                    quoted: false,
                },
            );
            if has {
                words.push(done);
            }
            has = false;
            continue;
        }
        if matches!(ch, '<' | '>' | '(') {
            word.meta = true;
        }
        word.text.push(ch);
        has = true;
    }
    if quote.is_some() {
        return None;
    }
    if has {
        words.push(word);
    }
    Some(words)
}

/// `/^[A-Za-z_][A-Za-z0-9_]*=/`.
fn assignment(token: &str) -> bool {
    let mut chars = token.chars();
    if !chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
    {
        return false;
    }
    for c in chars {
        if c == '=' {
            return true;
        }
        if !(c.is_ascii_alphanumeric() || c == '_') {
            return false;
        }
    }
    false
}

/// The `env` options after which the command starts; none for an unsafe form.
fn skip_env(words: &[Word], mut start: usize) -> Option<usize> {
    while let Some(word) = words.get(start) {
        if word.meta {
            return None;
        }
        let token = word.text.as_str();
        if token == "--" {
            return Some(start + 1);
        }
        if matches!(token, "-u" | "--unset" | "-C" | "--chdir") {
            words.get(start + 1).filter(|value| !value.meta)?;
            start += 2;
            continue;
        }
        let plain = [
            "-i",
            "--ignore-environment",
            "-0",
            "--null",
            "-v",
            "--debug",
        ];
        if plain.contains(&token)
            || token.starts_with("--unset=")
            || token.starts_with("--chdir=")
            || assignment(token)
        {
            start += 1;
            continue;
        }
        if token.starts_with('-') {
            return None;
        }
        break;
    }
    Some(start)
}

/// `stripPrefixTokens`: argv without assignments, `sudo`, `env` and harmless redirects.
pub fn strip_prefix_tokens(words: &[Word]) -> Option<Vec<String>> {
    let mut start = 0;
    while let Some(word) = words.get(start)
        && !word.meta
        && (assignment(&word.text) || word.text == "sudo")
    {
        start += 1;
    }
    if let Some(word) = words.get(start)
        && word.text == "env"
        && !word.meta
    {
        start = skip_env(words, start + 1)?;
    }
    let mut out = Vec::new();
    let mut index = start;
    while let Some(word) = words.get(index) {
        index += 1;
        if !word.meta {
            out.push(word.text.clone());
            continue;
        }
        let token = word.text.as_str();
        if word.quoted || ["<(", ">(", "=(", "<>"].iter().any(|s| token.contains(s)) {
            return None;
        }
        if token == "<" {
            let target = words.get(index);
            index += 1;
            if target.is_some_and(|t| t.meta) {
                return None;
            }
            continue;
        }
        if let Some(after) = token.strip_prefix('<')
            && !after.contains('<')
            && !after.contains('>')
        {
            continue;
        }
        let inline = redirect_target(token)?;
        let next = if inline.is_empty() {
            index += 1;
            words.get(index - 1)
        } else {
            None
        };
        let target = if inline.is_empty() {
            next.map_or("", |w| w.text.as_str())
        } else {
            inline
        };
        let fd = target.strip_prefix('&').is_some_and(|rest| {
            rest == "-" || (!rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()))
        });
        if next.is_some_and(|w| w.quoted) || !(target == "/dev/null" || fd) {
            return None;
        }
    }
    Some(out)
}

/// `/^(?:\d?|&)(>>?)(.*)$/`: the target written after the operator.
fn redirect_target(token: &str) -> Option<&str> {
    let rest = token
        .strip_prefix('&')
        .or_else(|| token.strip_prefix(|c: char| c.is_ascii_digit()))
        .filter(|r| r.starts_with('>'))
        .unwrap_or(token);
    let rest = rest.strip_prefix('>')?;
    Some(rest.strip_prefix('>').unwrap_or(rest))
}

/// `positionals`: arguments past flags and the values of `value_flags`.
pub fn positionals<'a>(tokens: &'a [String], value_flags: &[&str]) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut index = 0;
    while let Some(token) = tokens.get(index) {
        if token == "--" {
            out.extend(tokens[index + 1..].iter().map(String::as_str));
            break;
        }
        if token.starts_with('-') && token.len() > 1 {
            if value_flags.contains(&token.as_str()) {
                index += 1;
            }
        } else {
            out.push(token.as_str());
        }
        index += 1;
    }
    out
}

/// `cliSubcommand`: the first argument that is no option, lowercased, with the rest after it.
pub fn subcommand<'a>(
    tokens: &'a [String],
    value_flags: &[&str],
) -> Option<(String, &'a [String])> {
    let mut index = 0;
    while let Some(token) = tokens.get(index) {
        if token == "--" {
            return None;
        }
        if token.starts_with('-') && token.len() > 1 {
            if value_flags.contains(&token.as_str()) {
                index += 1;
            }
            index += 1;
            continue;
        }
        return Some((token.to_lowercase(), &tokens[index + 1..]));
    }
    None
}

/// `basenameRemotePath`: the last part past `/` or `\`, trailing separators dropped.
pub fn basename(path: &str) -> &str {
    let trimmed = path.trim_end_matches(['/', '\\']);
    trimmed.rsplit(['/', '\\']).next().unwrap_or(trimmed)
}

/// `binaryName`: the lowercased basename.
pub fn binary_name(token: &str) -> String {
    let base = basename(token);
    if base.is_empty() { token } else { base }.to_lowercase()
}
