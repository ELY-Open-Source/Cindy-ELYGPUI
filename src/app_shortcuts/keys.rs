//! Key names: gpui's keystrokes back to Cindy's `KeyboardEvent.code`, and Cindy's labels.

use gpui::Keystroke;

use super::Combo;

pub const MODIFIER_CODES: [&str; 11] = [
    "AltLeft",
    "AltRight",
    "ControlLeft",
    "ControlRight",
    "MetaLeft",
    "MetaRight",
    "ShiftLeft",
    "ShiftRight",
    "Fn",
    "FnLock",
    "CapsLock",
];

/// US-layout keys: gpui's unshifted name, the shifted name, and the code.
const PUNCTUATION: [(&str, &str, &str); 11] = [
    ("`", "~", "Backquote"),
    ("-", "_", "Minus"),
    ("=", "+", "Equal"),
    ("[", "{", "BracketLeft"),
    ("]", "}", "BracketRight"),
    ("\\", "|", "Backslash"),
    (";", ":", "Semicolon"),
    ("'", "\"", "Quote"),
    (",", "<", "Comma"),
    (".", ">", "Period"),
    ("/", "?", "Slash"),
];

const SHIFTED_DIGITS: [&str; 10] = [")", "!", "@", "#", "$", "%", "^", "&", "*", "("];

const NAMED: [(&str, &str); 16] = [
    ("space", "Space"),
    ("insert", "Insert"),
    ("tab", "Tab"),
    ("enter", "Enter"),
    ("escape", "Escape"),
    ("backspace", "Backspace"),
    ("delete", "Delete"),
    ("home", "Home"),
    ("end", "End"),
    ("pageup", "PageUp"),
    ("pagedown", "PageDown"),
    ("left", "ArrowLeft"),
    ("right", "ArrowRight"),
    ("up", "ArrowUp"),
    ("down", "ArrowDown"),
    ("add", "NumpadAdd"),
];

fn function_key(name: &str) -> bool {
    name.strip_prefix('F')
        .and_then(|n| n.parse::<u8>().ok())
        .is_some_and(|n| (1..=24).contains(&n))
}

/// The pressed key as Cindy codes it. gpui drops Shift for a shifted symbol, so it comes back.
pub fn pressed(keystroke: &Keystroke) -> Option<Combo> {
    let key = keystroke.key.as_str();
    let mut shift = keystroke.modifiers.shift;
    let code = if let [c] = key.as_bytes() {
        let c = *c as char;
        if c.is_ascii_lowercase() {
            format!("Key{}", c.to_ascii_uppercase())
        } else if c.is_ascii_digit() {
            format!("Digit{c}")
        } else if let Some(d) = SHIFTED_DIGITS.iter().position(|s| *s == key) {
            shift = true;
            format!("Digit{d}")
        } else {
            let (_, shifted, code) = PUNCTUATION
                .iter()
                .find(|(k, s, _)| *k == key || *s == key)?;
            shift |= *shifted == key;
            (*code).to_owned()
        }
    } else if key == "subtract" {
        "NumpadSubtract".into()
    } else if let Some((_, code)) = NAMED.iter().find(|(k, _)| *k == key) {
        (*code).into()
    } else if function_key(&key.to_ascii_uppercase()) {
        key.to_ascii_uppercase()
    } else {
        return None;
    };
    let m = keystroke.modifiers;
    Some(Combo {
        code,
        key: None,
        meta: m.platform,
        ctrl: m.control,
        alt: m.alt,
        shift,
    })
}

const DISPLAY: [(&str, &str); 28] = [
    ("Comma", ","),
    ("Period", "."),
    ("Semicolon", ";"),
    ("Quote", "'"),
    ("Slash", "/"),
    ("Backslash", "\\"),
    ("BracketLeft", "["),
    ("BracketRight", "]"),
    ("Backquote", "`"),
    ("Minus", "-"),
    ("Equal", "="),
    ("Space", "Space"),
    ("Tab", "Tab"),
    ("Enter", "Enter"),
    ("Escape", "Esc"),
    ("Backspace", "Backspace"),
    ("Delete", "Delete"),
    ("Home", "Home"),
    ("End", "End"),
    ("PageUp", "PgUp"),
    ("PageDown", "PgDn"),
    ("ArrowUp", "↑"),
    ("ArrowDown", "↓"),
    ("ArrowLeft", "←"),
    ("ArrowRight", "→"),
    ("NumpadAdd", "Num +"),
    ("NumpadSubtract", "Num -"),
    ("Numpad0", "Num 0"),
];

/// `displayKeyForCode`.
fn display_key(code: &str, key: Option<&str>) -> String {
    if let Some((_, label)) = DISPLAY.iter().find(|(c, _)| *c == code) {
        return (*label).into();
    }
    let tail = |prefix: &str, ok: fn(char) -> bool| {
        code.strip_prefix(prefix)
            .filter(|t| t.len() == 1 && t.chars().all(ok))
            .map(str::to_owned)
    };
    if let Some(letter) =
        tail("Key", |c| c.is_ascii_uppercase()).or_else(|| tail("Digit", |c| c.is_ascii_digit()))
    {
        return letter;
    }
    if function_key(code) {
        return code.into();
    }
    match key {
        Some(k) if k.encode_utf16().count() == 1 && k != " " => k.to_uppercase(),
        Some(k) if !k.is_empty() => k.into(),
        _ => code.into(),
    }
}

/// `formatAppShortcutCombo`: ⌃⌥⇧⌘ on darwin, `Ctrl+Alt+Shift+Meta+Key` elsewhere.
pub fn format(combo: &Combo, platform: &str) -> String {
    let darwin = platform == "darwin";
    let marks = if darwin {
        ["⌃", "⌥", "⇧", "⌘"]
    } else {
        ["Ctrl", "Alt", "Shift", "Meta"]
    };
    let held = [combo.ctrl, combo.alt, combo.shift, combo.meta];
    let mut parts: Vec<String> = marks
        .iter()
        .zip(held)
        .filter(|(_, on)| *on)
        .map(|(m, _)| (*m).into())
        .collect();
    parts.push(display_key(&combo.code, combo.key.as_deref()));
    parts.join(if darwin { "" } else { "+" })
}
