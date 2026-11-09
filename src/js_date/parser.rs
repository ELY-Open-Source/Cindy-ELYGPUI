//! V8's `DateParser` (`src/date/dateparser*`): ES5 date-time strings, then the legacy forms.

use crate::theme_import::color::js_space;

/// The fields `DateParser::Parse` writes: year, 0-based month, day, time, UTC offset in seconds.
pub type Fields = [f64; 8];

const NONE: i32 = i32::MAX;
const MAX_SIGNIFICANT_DIGITS: i32 = 9;
/// `Smi::kMaxValue` with pointer compression, as Electron builds V8.
const SMI_MAX: u32 = (1 << 30) - 1;

fn between(x: i32, lo: i32, hi: i32) -> bool {
    (x.wrapping_sub(lo) as u32) <= (hi.wrapping_sub(lo) as u32)
}

fn line_terminator(c: u32) -> bool {
    matches!(c, 0x0a | 0x0d | 0x2028 | 0x2029)
}

fn white_space_or_line_terminator(c: u32) -> bool {
    char::from_u32(c).is_some_and(js_space)
}

struct Reader<'a> {
    units: &'a [u16],
    index: usize,
    ch: u32,
}

impl Reader<'_> {
    fn next(&mut self) {
        self.ch = self.units.get(self.index).map_or(0, |u| u32::from(*u));
        self.index += 1;
    }

    fn read_unsigned_numeral(&mut self) -> i32 {
        let (mut n, mut i) = (0, 0);
        while self.ch == u32::from(b'0') {
            self.next();
        }
        while self.is_digit() {
            if i < MAX_SIGNIFICANT_DIGITS {
                n = n * 10 + (self.ch - u32::from(b'0')) as i32;
            }
            i += 1;
            self.next();
        }
        n
    }

    fn read_word(&mut self, prefix: &mut [u32; 3]) -> i32 {
        let mut len = 0;
        while self.is_alpha_or_above() && !self.is_white_space() {
            if let Some(slot) = prefix.get_mut(len) {
                *slot = self.ch | 0x20;
            }
            self.next();
            len += 1;
        }
        len as i32
    }

    fn skip(&mut self, c: u8) -> bool {
        let hit = self.ch == u32::from(c);
        if hit {
            self.next();
        }
        hit
    }

    fn skip_parentheses(&mut self) -> bool {
        if self.ch != u32::from(b'(') {
            return false;
        }
        let mut balance = 0;
        loop {
            match self.ch {
                0x29 => balance -= 1,
                0x28 => balance += 1,
                _ => {}
            }
            self.next();
            if balance <= 0 || self.ch == 0 {
                return true;
            }
        }
    }

    fn is_digit(&self) -> bool {
        (u32::from(b'0')..=u32::from(b'9')).contains(&self.ch)
    }

    fn is_alpha_or_above(&self) -> bool {
        self.ch >= u32::from(b'A')
    }

    fn is_white_space(&self) -> bool {
        white_space_or_line_terminator(self.ch) && !line_terminator(self.ch)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Invalid,
    MonthName,
    TimeZoneName,
    TimeSeparator,
    AmPm,
}

#[derive(Clone, Copy)]
enum Token {
    Invalid,
    Unknown,
    WhiteSpace,
    End,
    Number { value: i32, len: i32 },
    Symbol(u8),
    Keyword { kind: Kind, value: i32, len: i32 },
}

impl Token {
    fn number(self) -> Option<i32> {
        match self {
            Token::Number { value, .. } => Some(value),
            _ => None,
        }
    }

    fn is_fixed(self, length: i32) -> bool {
        matches!(self, Token::Number { len, .. } if len == length)
    }

    fn is_symbol(self, c: u8) -> bool {
        matches!(self, Token::Symbol(s) if s == c)
    }

    fn sign(self) -> Option<i32> {
        match self {
            Token::Symbol(c @ (b'+' | b'-')) => Some(44 - i32::from(c)),
            _ => None,
        }
    }

    fn is_kind(self, want: Kind) -> bool {
        matches!(self, Token::Keyword { kind, .. } if kind == want)
    }

    fn is_z(self) -> bool {
        matches!(
            self,
            Token::Keyword {
                kind: Kind::TimeZoneName,
                len: 1,
                value: 0
            }
        )
    }
}

const KEYWORDS: [(&[u8; 3], Kind, i32); 27] = [
    (b"jan", Kind::MonthName, 1),
    (b"feb", Kind::MonthName, 2),
    (b"mar", Kind::MonthName, 3),
    (b"apr", Kind::MonthName, 4),
    (b"may", Kind::MonthName, 5),
    (b"jun", Kind::MonthName, 6),
    (b"jul", Kind::MonthName, 7),
    (b"aug", Kind::MonthName, 8),
    (b"sep", Kind::MonthName, 9),
    (b"oct", Kind::MonthName, 10),
    (b"nov", Kind::MonthName, 11),
    (b"dec", Kind::MonthName, 12),
    (b"am\0", Kind::AmPm, 0),
    (b"pm\0", Kind::AmPm, 12),
    (b"ut\0", Kind::TimeZoneName, 0),
    (b"utc", Kind::TimeZoneName, 0),
    (b"z\0\0", Kind::TimeZoneName, 0),
    (b"gmt", Kind::TimeZoneName, 0),
    (b"cdt", Kind::TimeZoneName, -5),
    (b"cst", Kind::TimeZoneName, -6),
    (b"edt", Kind::TimeZoneName, -4),
    (b"est", Kind::TimeZoneName, -5),
    (b"mdt", Kind::TimeZoneName, -6),
    (b"mst", Kind::TimeZoneName, -7),
    (b"pdt", Kind::TimeZoneName, -7),
    (b"pst", Kind::TimeZoneName, -8),
    (b"t\0\0", Kind::TimeSeparator, 0),
];

/// `KeywordTable::Lookup`: only month names may run past three letters.
fn keyword(prefix: &[u32; 3], len: i32) -> (Kind, i32) {
    KEYWORDS
        .iter()
        .find(|(word, kind, _)| {
            word.iter().zip(prefix).all(|(w, p)| u32::from(*w) == *p)
                && (len <= 3 || *kind == Kind::MonthName)
        })
        .map_or((Kind::Invalid, 0), |(_, kind, value)| (*kind, *value))
}

struct Scanner<'a> {
    input: Reader<'a>,
    next: Token,
}

impl Scanner<'_> {
    fn scan(&mut self) -> Token {
        let input = &mut self.input;
        let start = input.index;
        if input.ch == 0 {
            return Token::End;
        }
        if input.is_digit() {
            let value = input.read_unsigned_numeral();
            let len = (input.index - start) as i32;
            return Token::Number { value, len };
        }
        for c in *b":-+.)" {
            if input.skip(c) {
                return Token::Symbol(c);
            }
        }
        if input.is_alpha_or_above() && !input.is_white_space() {
            let mut prefix = [0; 3];
            let len = input.read_word(&mut prefix);
            let (kind, value) = keyword(&prefix, len);
            return Token::Keyword { kind, value, len };
        }
        if white_space_or_line_terminator(input.ch) {
            input.next();
            return Token::WhiteSpace;
        }
        if !input.skip_parentheses() {
            input.next();
        }
        Token::Unknown
    }

    fn next(&mut self) -> Token {
        let token = self.next;
        self.next = self.scan();
        token
    }

    fn peek(&self) -> Token {
        self.next
    }

    fn skip_symbol(&mut self, c: u8) -> bool {
        let hit = self.next.is_symbol(c);
        if hit {
            self.next = self.scan();
        }
        hit
    }
}

mod composers;
use composers::{Day, Time, Zone};

/// `ParseES5DateTime`: the token the legacy parser goes on from.
fn es5(s: &mut Scanner, day: &mut Day, time: &mut Time, tz: &mut Zone) -> Token {
    if let Some(sign) = s.peek().sign() {
        let sign_token = s.next();
        if !s.peek().is_fixed(6) {
            return sign_token;
        }
        let year = s.next().number().expect("a number");
        if sign < 0 && year == 0 {
            return sign_token;
        }
        day.add(sign * year);
    } else if s.peek().is_fixed(4) {
        day.add(s.next().number().expect("a number"));
    } else {
        return s.next();
    }
    if s.skip_symbol(b'-') {
        if !two_digits(s.peek(), Day::is_month) {
            return s.next();
        }
        day.add(s.next().number().expect("a number"));
        if s.skip_symbol(b'-') {
            if !two_digits(s.peek(), Day::is_day) {
                return s.next();
            }
            day.add(s.next().number().expect("a number"));
        }
    }
    if !s.peek().is_kind(Kind::TimeSeparator) {
        if !matches!(s.peek(), Token::End) {
            return s.next();
        }
    } else if !es5_time(s, time, tz) {
        return Token::Invalid;
    }
    if tz.is_empty() && time.is_empty() {
        tz.set(0);
    }
    day.set_iso_date();
    Token::End
}

/// The ES5 time after its `T`; false where V8 returns an invalid token.
fn es5_time(s: &mut Scanner, time: &mut Time, tz: &mut Zone) -> bool {
    s.next();
    let peek = s.peek();
    if !peek.is_fixed(2) || !peek.number().is_some_and(|h| between(h, 0, 24)) {
        return false;
    }
    let hour_is_24 = peek.number() == Some(24);
    time.add(s.next().number().expect("a number"));
    if !s.skip_symbol(b':') {
        return false;
    }
    let part = |t: Token, ok: fn(i32) -> bool| {
        t.is_fixed(2) && t.number().is_some_and(|n| ok(n) && !(hour_is_24 && n > 0))
    };
    if !part(s.peek(), Time::is_minute) {
        return false;
    }
    time.add(s.next().number().expect("a number"));
    if s.skip_symbol(b':') {
        if !part(s.peek(), Time::is_second) {
            return false;
        }
        time.add(s.next().number().expect("a number"));
        if s.skip_symbol(b'.') {
            let peek = s.peek();
            if peek.number().is_none_or(|n| hour_is_24 && n > 0) {
                return false;
            }
            time.add(composers::read_milliseconds(s.next()));
        }
    }
    if s.peek().is_z() {
        s.next();
        tz.set(0);
    } else if let Some(sign) = s.peek().sign() {
        s.next();
        tz.set_sign(sign);
        if s.peek().is_fixed(4) {
            let hour_min = s.next().number().expect("a number");
            let (hour, min) = (hour_min / 100, hour_min % 100);
            if !Time::is_hour(hour) || !Time::is_minute(min) {
                return false;
            }
            tz.set_absolute_hour(hour);
            tz.set_absolute_minute(min);
        } else {
            if !two_digits(s.peek(), Time::is_hour) {
                return false;
            }
            tz.set_absolute_hour(s.next().number().expect("a number"));
            if !s.skip_symbol(b':') || !two_digits(s.peek(), Time::is_minute) {
                return false;
            }
            tz.set_absolute_minute(s.next().number().expect("a number"));
        }
    }
    matches!(s.peek(), Token::End)
}

fn two_digits(t: Token, ok: fn(i32) -> bool) -> bool {
    t.is_fixed(2) && t.number().is_some_and(ok)
}

/// `DateParser::Parse` over a string's UTF-16 units.
pub fn parse(units: &[u16]) -> Option<Fields> {
    let mut input = Reader {
        units,
        index: 0,
        ch: 0,
    };
    input.next();
    let mut s = Scanner {
        input,
        next: Token::End,
    };
    s.next = s.scan();
    let (mut tz, mut time, mut day) = (Zone::new(), Time::new(), Day::new());
    let first = es5(&mut s, &mut day, &mut time, &mut tz);
    if matches!(first, Token::Invalid) {
        return None;
    }
    let mut has_read_number = !day.is_empty();
    let mut token = first;
    while !matches!(token, Token::End) {
        if !composers::legacy(
            &mut s,
            token,
            (&mut day, &mut time, &mut tz),
            &mut has_read_number,
        ) {
            return None;
        }
        token = s.next();
    }
    let mut out = [0.0; 8];
    (day.write(&mut out) && time.write(&mut out) && tz.write(&mut out)).then_some(out)
}
