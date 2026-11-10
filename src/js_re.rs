//! A JavaScript regex source without the `u` flag, as a Rust regex.

use regex::Regex;

use crate::theme_import::color::JS_SPACE;

/// JS `\s` inside a class.
fn space() -> &'static str {
    JS_SPACE.trim_start_matches('[').trim_end_matches(']')
}

/// `source` with JS `\s`, `\S` and ASCII `\b`; `fold` is `/i`, which folds ASCII letters only.
pub fn regex(source: &str, fold: bool) -> Regex {
    let mut out = String::new();
    let mut class = false;
    let mut chars = source.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                let next = chars.next().expect("a complete escape");
                match (next, class) {
                    ('s', true) => out.push_str(space()),
                    ('s', false) => out.push_str(JS_SPACE),
                    ('S', false) => out.push_str(&format!("[^{}]", space())),
                    ('b', false) => out.push_str(r"(?-u:\b)"),
                    ('S' | 'b' | 'd' | 'w' | 'W' | 'D' | 'B', _) => {
                        panic!("unported escape \\{next} in {source}")
                    }
                    _ => {
                        out.push('\\');
                        out.push(next);
                    }
                }
            }
            '[' if !class => {
                class = true;
                out.push('[');
                if chars.peek() == Some(&'^') {
                    out.push(chars.next().expect("peeked"));
                }
            }
            ']' if class => {
                class = false;
                out.push(']');
            }
            '-' if class && fold && out.ends_with(|p: char| p.is_ascii_alphabetic()) => {
                panic!("unported folded range in {source}")
            }
            '.' if !class => panic!("unported dot in {source}"),
            c if fold && c.is_ascii_alphabetic() => {
                let (lower, upper) = (c.to_ascii_lowercase(), c.to_ascii_uppercase());
                match class {
                    true => out.extend([lower, upper]),
                    false => out.push_str(&format!("[{lower}{upper}]")),
                }
            }
            c => out.push(c),
        }
    }
    Regex::new(&out).unwrap_or_else(|e| panic!("{source}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::regex;

    #[test]
    fn folding_spaces_and_boundaries_follow_js() {
        let r = regex(r"(?:^|\s)-o\b", true);
        assert!(r.is_match("x -O"));
        assert!(r.is_match("x\u{feff}-o"));
        assert!(!r.is_match("x \u{212a}"));
        assert!(!regex(r"\bk", true).is_match("\u{212a}"));
        assert!(regex(r"[^\s'][rubf]$", true).is_match("aB"));
        assert!(regex(r"\bé", false).is_match("aé"));
        assert!(!regex(r"\bé", false).is_match("é"));
    }
}
