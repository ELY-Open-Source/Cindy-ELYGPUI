//! `commandDisplay.ts`: the command inside a known shell launcher, for display only.
#![cfg_attr(
    not(test),
    expect(dead_code, reason = "the activity line reads it in 8.8b3")
)]

use crate::theme_import::color::{js_space, js_trim};

struct Token {
    text: String,
    end: usize,
    quoted: bool,
    literal: bool,
}

fn token(text: String, end: usize) -> Token {
    Token {
        text,
        end,
        quoted: false,
        literal: false,
    }
}

fn skip_space(input: &[char], mut index: usize) -> Option<usize> {
    while input.get(index).is_some_and(|&c| js_space(c)) {
        index += 1;
    }
    (index < input.len()).then_some(index)
}

fn rest(input: &[char], from: usize) -> String {
    input[from.min(input.len())..].iter().collect()
}

/// `readShellToken`: a bare word, or a quoted one with `''`, `\"` and `` `" `` read as quotes.
fn shell_token(input: &[char], start: usize) -> Option<Token> {
    let mut index = skip_space(input, start)?;
    let quote = input[index];
    if quote != '"' && quote != '\'' {
        let from = index;
        while input.get(index).is_some_and(|&c| !js_space(c)) {
            index += 1;
        }
        return Some(token(input[from..index].iter().collect(), index));
    }
    index += 1;
    let mut text = String::new();
    while let Some(&ch) = input.get(index) {
        let next = input.get(index + 1).copied();
        if ch == quote {
            if quote == '\'' && next == Some('\'') {
                text.push('\'');
                index += 2;
                continue;
            }
            return Some(token(text, index + 1));
        }
        if quote == '"' && (ch == '\\' || ch == '`') && next == Some('"') {
            text.push('"');
            index += 2;
            continue;
        }
        text.push(ch);
        index += 1;
    }
    None
}

/// `readCommandRemainder`: the rest of the line, or one quoted token.
fn remainder(input: &[char], start: usize) -> Option<Token> {
    let index = skip_space(input, start)?;
    if input[index] != '"' && input[index] != '\'' {
        return Some(token(js_trim(&rest(input, index)).to_owned(), input.len()));
    }
    shell_token(input, index)
}

/// `readPosixShellToken`: one POSIX word, quoted and bare parts joined.
fn posix_token(input: &[char], start: usize) -> Option<Token> {
    let mut index = skip_space(input, start)?;
    let mut word = token(String::new(), 0);
    while let Some(&ch) = input.get(index).filter(|&&c| !js_space(c)) {
        index += 1;
        match ch {
            '\'' => {
                word.quoted = true;
                while let Some(&c) = input.get(index).filter(|&&c| c != '\'') {
                    word.text.push(c);
                    index += 1;
                }
                input.get(index)?;
                index += 1;
            }
            '"' => {
                word.quoted = true;
                while let Some(&c) = input.get(index).filter(|&&c| c != '"') {
                    if c == '\\' {
                        let next = *input.get(index + 1)?;
                        if matches!(next, '$' | '`' | '"' | '\\' | '\n') {
                            if next != '\n' {
                                word.text.push(next);
                            }
                            index += 2;
                            continue;
                        }
                    }
                    word.text.push(c);
                    index += 1;
                }
                input.get(index)?;
                index += 1;
            }
            '\\' => {
                let next = *input.get(index)?;
                if next != '\n' {
                    word.text.push(next);
                }
                // JS escapes one UTF-16 unit; a low surrogate stays a literal.
                word.literal |= next.len_utf16() == 2;
                index += 1;
            }
            _ => {
                word.literal = true;
                word.text.push(ch);
            }
        }
    }
    word.end = index;
    Some(word)
}

/// `^[A-Za-z]:[\\/]` or `^[\\/]{2}`: an absolute Windows path.
fn windows_absolute(binary: &str) -> bool {
    let b = binary.as_bytes();
    let slash = |i: usize| b.get(i).is_some_and(|&c| c == b'/' || c == b'\\');
    (b.first().is_some_and(u8::is_ascii_alphabetic) && b.get(1) == Some(&b':') && slash(2))
        || (slash(0) && slash(1))
}

/// `(?:^|[\\/])name$`, ASCII case-insensitive.
fn ends_with_file(binary: &str, name: &str) -> bool {
    let lower = binary.to_ascii_lowercase();
    lower
        .strip_suffix(name)
        .is_some_and(|head| head.is_empty() || head.ends_with('/') || head.ends_with('\\'))
}

fn powershell(binary: &str) -> bool {
    (ends_with_file(binary, "pwsh.exe") || ends_with_file(binary, "powershell.exe"))
        && windows_absolute(binary)
}

fn cmd(binary: &str) -> bool {
    if !ends_with_file(binary, "cmd") && !ends_with_file(binary, "cmd.exe") {
        return false;
    }
    !binary.contains(['\\', '/']) || windows_absolute(binary)
}

/// `name` bare or under one of `dirs`.
fn under(binary: &str, dirs: &[&str], names: &[&str]) -> bool {
    let name = dirs
        .iter()
        .find_map(|d| binary.strip_prefix(d).and_then(|r| r.strip_prefix('/')))
        .unwrap_or(binary);
    names.contains(&name)
}

fn posix_wrapper(input: &[char]) -> Option<String> {
    let mut executable = posix_token(input, 0)?;
    if under(&executable.text, &["/bin", "/usr/bin"], &["env"]) {
        executable = posix_token(input, executable.end)?;
    }
    let dirs = ["/bin", "/usr/bin", "/usr/local/bin"];
    if !under(&executable.text, &dirs, &["zsh", "bash", "sh"]) {
        return None;
    }
    let option =
        posix_token(input, executable.end).filter(|o| o.text == "-lc" || o.text == "-c")?;
    let script = posix_token(input, option.end).filter(|s| !s.text.is_empty())?;
    if !js_trim(&rest(input, script.end)).is_empty() {
        return None;
    }
    let bare = |c: char| c.is_ascii_alphanumeric() || "_./:@%+=,-".contains(c);
    let ok = if script.quoted {
        !script.literal
    } else {
        script.text.chars().all(bare)
    };
    ok.then_some(script.text)
}

/// A wrapper's command when nothing follows it.
fn whole(input: &[char], command: Option<Token>) -> Option<String> {
    let command = command?;
    if !js_trim(&rest(input, command.end)).is_empty() || command.text.is_empty() {
        return None;
    }
    Some(command.text)
}

fn powershell_wrapper(input: &[char]) -> Option<String> {
    let executable = shell_token(input, 0).filter(|e| powershell(&e.text))?;
    let mut index = executable.end;
    loop {
        let token = shell_token(input, index)?;
        let flag = token.text.strip_prefix('-');
        if flag.is_some_and(|f| f.eq_ignore_ascii_case("command") || f.eq_ignore_ascii_case("c")) {
            return whole(input, remainder(input, token.end));
        }
        index = token.end;
    }
}

fn cmd_wrapper(input: &[char]) -> Option<String> {
    let executable = shell_token(input, 0).filter(|e| cmd(&e.text))?;
    let option =
        shell_token(input, executable.end).filter(|o| o.text.eq_ignore_ascii_case("/c"))?;
    whole(input, remainder(input, option.end))
}

/// `normalizeDisplayCommand`.
pub fn normalize(command: &str) -> Option<String> {
    if js_trim(command).is_empty() {
        return None;
    }
    let input: Vec<char> = command.chars().collect();
    posix_wrapper(&input)
        .or_else(|| powershell_wrapper(&input))
        .or_else(|| cmd_wrapper(&input))
}

#[cfg(test)]
mod tests;
