//! What `JSON.parse` gives JavaScript: numbers past f64 are `Infinity`, not `null`, a
//! repeated key keeps its first place and last value, and nesting has no limit.

use indexmap::IndexMap;

use crate::cindy_json::js_number;

mod stringify;
pub use stringify::{entries, stringify};

/// Recursion grows the stack on the heap, as V8 nests a million levels.
const RED_ZONE: usize = 64 * 1024;
const STACK: usize = 1024 * 1024;

#[derive(Debug, PartialEq)]
pub enum Js {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Js>),
    Obj(Obj),
}

/// Keys in insertion order; `values` gives JavaScript's property order.
pub type Obj = IndexMap<String, Js>;

impl Clone for Js {
    fn clone(&self) -> Self {
        stacker::maybe_grow(RED_ZONE, STACK, || match self {
            Js::Null => Js::Null,
            Js::Bool(b) => Js::Bool(*b),
            Js::Num(n) => Js::Num(*n),
            Js::Str(s) => Js::Str(s.clone()),
            Js::Arr(items) => Js::Arr(items.clone()),
            Js::Obj(fields) => Js::Obj(fields.clone()),
        })
    }
}

impl Drop for Js {
    fn drop(&mut self) {
        match self {
            Js::Arr(items) if !items.is_empty() => {
                let items = std::mem::take(items);
                stacker::maybe_grow(RED_ZONE, STACK, move || drop(items));
            }
            Js::Obj(fields) if !fields.is_empty() => {
                let fields = std::mem::take(fields);
                stacker::maybe_grow(RED_ZONE, STACK, move || drop(fields));
            }
            _ => {}
        }
    }
}

#[derive(Debug)]
pub struct SyntaxError;

/// `JSON.parse`; a lone surrogate escape fails here, as Rust strings cannot hold one.
pub fn parse(text: &str) -> Result<Js, SyntaxError> {
    let mut parser = Parser {
        text: text.as_bytes(),
        at: 0,
    };
    let value = parser.value()?;
    parser.space();
    match parser.at == parser.text.len() {
        true => Ok(value),
        false => Err(SyntaxError),
    }
}

struct Parser<'a> {
    text: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.text.get(self.at).copied()
    }

    fn space(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn skip(&mut self, byte: u8) -> bool {
        let hit = self.peek() == Some(byte);
        self.at += usize::from(hit);
        hit
    }

    fn eat(&mut self, byte: u8) -> bool {
        self.space();
        self.skip(byte)
    }

    fn value(&mut self) -> Result<Js, SyntaxError> {
        stacker::maybe_grow(RED_ZONE, STACK, || {
            self.space();
            match self.peek() {
                Some(b'{') => self.object(),
                Some(b'[') => self.array(),
                Some(b'"') => self.string().map(Js::Str),
                Some(b't') => self.literal(b"true", Js::Bool(true)),
                Some(b'f') => self.literal(b"false", Js::Bool(false)),
                Some(b'n') => self.literal(b"null", Js::Null),
                Some(b'-' | b'0'..=b'9') => self.number(),
                _ => Err(SyntaxError),
            }
        })
    }

    fn literal(&mut self, word: &[u8], value: Js) -> Result<Js, SyntaxError> {
        if !self.text[self.at..].starts_with(word) {
            return Err(SyntaxError);
        }
        self.at += word.len();
        Ok(value)
    }

    fn array(&mut self) -> Result<Js, SyntaxError> {
        self.at += 1;
        let mut items = Vec::new();
        if self.eat(b']') {
            return Ok(Js::Arr(items));
        }
        loop {
            items.push(self.value()?);
            if self.eat(b']') {
                return Ok(Js::Arr(items));
            }
            if !self.eat(b',') {
                return Err(SyntaxError);
            }
        }
    }

    fn object(&mut self) -> Result<Js, SyntaxError> {
        self.at += 1;
        let mut fields = Obj::new();
        if self.eat(b'}') {
            return Ok(Js::Obj(fields));
        }
        loop {
            self.space();
            if self.peek() != Some(b'"') {
                return Err(SyntaxError);
            }
            let key = self.string()?;
            if !self.eat(b':') {
                return Err(SyntaxError);
            }
            fields.insert(key, self.value()?);
            if self.eat(b'}') {
                return Ok(Js::Obj(fields));
            }
            if !self.eat(b',') {
                return Err(SyntaxError);
            }
        }
    }

    fn digits(&mut self) -> Result<(), SyntaxError> {
        let start = self.at;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.at += 1;
        }
        match self.at > start {
            true => Ok(()),
            false => Err(SyntaxError),
        }
    }

    fn number(&mut self) -> Result<Js, SyntaxError> {
        let start = self.at;
        self.skip(b'-');
        if !self.skip(b'0') {
            self.digits()?;
        }
        if self.skip(b'.') {
            self.digits()?;
        }
        if self.skip(b'e') || self.skip(b'E') {
            if !self.skip(b'+') {
                self.skip(b'-');
            }
            self.digits()?;
        }
        let text = std::str::from_utf8(&self.text[start..self.at]).expect("ASCII digits");
        Ok(Js::Num(text.parse().expect("a JSON number parses as f64")))
    }

    fn string(&mut self) -> Result<String, SyntaxError> {
        self.at += 1;
        let mut out = String::new();
        loop {
            let start = self.at;
            while matches!(self.peek(), Some(b) if b != b'"' && b != b'\\' && b >= 0x20) {
                self.at += 1;
            }
            out.push_str(std::str::from_utf8(&self.text[start..self.at]).expect("UTF-8 input"));
            match self.peek() {
                Some(b'"') => {
                    self.at += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.at += 1;
                    out.push(self.escape()?);
                }
                _ => return Err(SyntaxError),
            }
        }
    }

    fn escape(&mut self) -> Result<char, SyntaxError> {
        let byte = self.peek().ok_or(SyntaxError)?;
        self.at += 1;
        Ok(match byte {
            b'"' => '"',
            b'\\' => '\\',
            b'/' => '/',
            b'b' => '\u{8}',
            b'f' => '\u{c}',
            b'n' => '\n',
            b'r' => '\r',
            b't' => '\t',
            b'u' => {
                let unit = self.hex()?;
                if (0xD800..0xDC00).contains(&unit) && self.text[self.at..].starts_with(b"\\u") {
                    self.at += 2;
                    let low = self.hex()?;
                    if !(0xDC00..0xE000).contains(&low) {
                        return Err(SyntaxError);
                    }
                    char::from_u32(0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00))
                        .expect("a surrogate pair")
                } else {
                    char::from_u32(unit).ok_or(SyntaxError)?
                }
            }
            _ => return Err(SyntaxError),
        })
    }

    fn hex(&mut self) -> Result<u32, SyntaxError> {
        let digits = self.text.get(self.at..self.at + 4).ok_or(SyntaxError)?;
        if !digits.iter().all(u8::is_ascii_hexdigit) {
            return Err(SyntaxError);
        }
        self.at += 4;
        let digits = std::str::from_utf8(digits).expect("hex digits");
        Ok(u32::from_str_radix(digits, 16).expect("hex digits"))
    }
}

impl Js {
    pub fn str(&self) -> Option<&str> {
        match self {
            Js::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn obj(&self) -> Option<&Obj> {
        match self {
            Js::Obj(o) => Some(o),
            _ => None,
        }
    }

    pub fn arr(&self) -> Option<&[Js]> {
        match self {
            Js::Arr(a) => Some(a),
            _ => None,
        }
    }

    /// `Number.isInteger`.
    pub fn integer(&self) -> Option<f64> {
        match self {
            Js::Num(n) if n.is_finite() && n.fract() == 0.0 => Some(*n),
            _ => None,
        }
    }
}

/// Runs deep recursion on a grown stack.
pub fn deep<R>(f: impl FnOnce() -> R) -> R {
    stacker::maybe_grow(RED_ZONE, STACK, f)
}

/// JavaScript truthiness; none is `undefined`.
pub fn truthy(value: Option<&Js>) -> bool {
    match value {
        None | Some(Js::Null) => false,
        Some(Js::Bool(b)) => *b,
        Some(Js::Num(n)) => *n != 0.0 && !n.is_nan(),
        Some(Js::Str(s)) => !s.is_empty(),
        Some(Js::Arr(_) | Js::Obj(_)) => true,
    }
}

/// `String(value)`; none is `undefined`.
pub fn string(value: Option<&Js>) -> String {
    stacker::maybe_grow(RED_ZONE, STACK, || match value {
        None => "undefined".into(),
        Some(Js::Null) => "null".into(),
        Some(Js::Bool(b)) => b.to_string(),
        Some(Js::Num(n)) => js_number(*n),
        Some(Js::Str(s)) => s.clone(),
        Some(Js::Arr(items)) => items
            .iter()
            .map(|v| match v {
                Js::Null => String::new(),
                v => string(Some(v)),
            })
            .collect::<Vec<_>>()
            .join(","),
        Some(Js::Obj(_)) => "[object Object]".into(),
    })
}

/// `readNonEmptyString`: a string, trimmed, when something is left.
pub fn non_empty(value: Option<&Js>) -> Option<&str> {
    let text = crate::theme_import::color::js_trim(value?.str()?);
    (!text.is_empty()).then_some(text)
}

/// The UTF-16 length JavaScript gives a string.
pub fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// `JSON.stringify(value).length`.
pub fn stringify_len(value: &Js) -> usize {
    stacker::maybe_grow(RED_ZONE, STACK, || match value {
        Js::Null => 4,
        Js::Bool(true) => 4,
        Js::Bool(false) => 5,
        Js::Num(n) if n.is_finite() => js_number(*n).len(),
        Js::Num(_) => 4,
        Js::Str(s) => quoted_len(s),
        Js::Arr(items) => {
            2 + items.iter().map(stringify_len).sum::<usize>() + items.len().saturating_sub(1)
        }
        Js::Obj(fields) => {
            2 + fields
                .iter()
                .map(|(k, v)| quoted_len(k) + 1 + stringify_len(v))
                .sum::<usize>()
                + fields.len().saturating_sub(1)
        }
    })
}

fn quoted_len(text: &str) -> usize {
    2 + text
        .encode_utf16()
        .map(|unit| match unit {
            0x22 | 0x5c | 0x08 | 0x0c | 0x0a | 0x0d | 0x09 => 2,
            0..0x20 => 6,
            _ => 1,
        })
        .sum::<usize>()
}

/// `Object.values`: array-index keys ascending, then the rest in insertion order.
pub fn values(fields: &Obj) -> impl Iterator<Item = &Js> {
    entries(fields).map(|(_, v)| v)
}

/// `JSON.stringify` of a string.
pub fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => out.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
