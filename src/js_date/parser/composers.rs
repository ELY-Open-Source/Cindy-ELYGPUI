//! `DayComposer`, `TimeComposer`, `TimeZoneComposer` and the legacy loop's step.

use super::{Fields, Kind, NONE, SMI_MAX, Scanner, Token, between};

/// `ReadMilliseconds`: the first three significant digits of the numeral.
pub fn read_milliseconds(token: Token) -> i32 {
    let Token::Number { value, len } = token else {
        unreachable!("a number token");
    };
    match len {
        1 => value * 100,
        2 => value * 10,
        3 => value,
        len => value / 10_i32.pow((len.min(super::MAX_SIGNIFICANT_DIGITS) - 3) as u32),
    }
}

pub struct Day {
    comp: [i32; 3],
    index: usize,
    named_month: i32,
    iso: bool,
}

impl Day {
    pub fn new() -> Self {
        Day {
            comp: [0; 3],
            index: 0,
            named_month: NONE,
            iso: false,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.index == 0
    }

    pub fn add(&mut self, n: i32) -> bool {
        let Some(slot) = self.comp.get_mut(self.index) else {
            return false;
        };
        *slot = n;
        self.index += 1;
        true
    }

    pub fn set_iso_date(&mut self) {
        self.iso = true;
    }

    pub fn is_month(x: i32) -> bool {
        between(x, 1, 12)
    }

    pub fn is_day(x: i32) -> bool {
        between(x, 1, 31)
    }

    /// V8 fills the missing parts with 1 first, so every form reads three parts.
    pub fn write(&mut self, out: &mut Fields) -> bool {
        if self.index < 1 {
            return false;
        }
        self.comp[self.index..].fill(1);
        let [a, b, c] = self.comp;
        let (mut year, month, day) = match self.named_month {
            NONE if self.iso || !Self::is_day(a) => (a, b, c),
            NONE => (c, a, b),
            named if !Self::is_day(a) => (a, named, b),
            named => (b, named, a),
        };
        if !self.iso {
            if between(year, 0, 49) {
                year += 2000;
            } else if between(year, 50, 99) {
                year += 1900;
            }
        }
        if !Self::is_month(month) || !Self::is_day(day) {
            return false;
        }
        out[0] = f64::from(year);
        out[1] = f64::from(month - 1);
        out[2] = f64::from(day);
        true
    }
}

pub struct Time {
    comp: [i32; 4],
    index: usize,
    hour_offset: i32,
}

impl Time {
    pub fn new() -> Self {
        Time {
            comp: [0; 4],
            index: 0,
            hour_offset: NONE,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.index == 0
    }

    pub fn is_minute(x: i32) -> bool {
        between(x, 0, 59)
    }

    pub fn is_hour(x: i32) -> bool {
        between(x, 0, 23)
    }

    pub fn is_second(x: i32) -> bool {
        between(x, 0, 59)
    }

    fn is_millisecond(x: i32) -> bool {
        between(x, 0, 999)
    }

    fn is_expecting(&self, n: i32) -> bool {
        (self.index == 1 && Self::is_minute(n))
            || (self.index == 2 && Self::is_second(n))
            || (self.index == 3 && Self::is_millisecond(n))
    }

    pub fn add(&mut self, n: i32) -> bool {
        let Some(slot) = self.comp.get_mut(self.index) else {
            return false;
        };
        *slot = n;
        self.index += 1;
        true
    }

    fn add_final(&mut self, n: i32) {
        if self.add(n) {
            self.comp[self.index..].fill(0);
            self.index = self.comp.len();
        }
    }

    pub fn write(&mut self, out: &mut Fields) -> bool {
        self.comp[self.index..].fill(0);
        self.index = self.comp.len();
        let [mut hour, minute, second, ms] = self.comp;
        if self.hour_offset != NONE {
            if !between(hour, 0, 12) {
                return false;
            }
            hour = hour % 12 + self.hour_offset;
        }
        let valid = Self::is_hour(hour)
            && Self::is_minute(minute)
            && Self::is_second(second)
            && Self::is_millisecond(ms);
        if !valid && (hour != 24 || minute != 0 || second != 0 || ms != 0) {
            return false;
        }
        out[3..7].copy_from_slice(&[hour, minute, second, ms].map(f64::from));
        true
    }
}

pub struct Zone {
    sign: i32,
    hour: i32,
    minute: i32,
}

impl Zone {
    pub fn new() -> Self {
        Zone {
            sign: NONE,
            hour: NONE,
            minute: NONE,
        }
    }

    pub fn set(&mut self, offset_in_hours: i32) {
        self.sign = if offset_in_hours < 0 { -1 } else { 1 };
        self.hour = offset_in_hours * self.sign;
        self.minute = 0;
    }

    pub fn set_sign(&mut self, sign: i32) {
        self.sign = if sign < 0 { -1 } else { 1 };
    }

    pub fn set_absolute_hour(&mut self, hour: i32) {
        self.hour = hour;
    }

    pub fn set_absolute_minute(&mut self, minute: i32) {
        self.minute = minute;
    }

    fn is_expecting(&self, n: i32) -> bool {
        self.hour != NONE && self.minute == NONE && Time::is_minute(n)
    }

    fn is_utc(&self) -> bool {
        self.hour == 0 && self.minute == 0
    }

    pub fn is_empty(&self) -> bool {
        self.hour == NONE
    }

    /// The offset in seconds, NaN for local time; V8 adds in `unsigned`.
    pub fn write(&self, out: &mut Fields) -> bool {
        if self.sign == NONE {
            out[7] = f64::NAN;
            return true;
        }
        let part = |v: i32| if v == NONE { 0 } else { v as u32 };
        let total = part(self.hour)
            .wrapping_mul(3600)
            .wrapping_add(part(self.minute).wrapping_mul(60));
        if total > SMI_MAX {
            return false;
        }
        let total = total as i32;
        out[7] = f64::from(if self.sign < 0 { -total } else { total });
        true
    }
}

/// One turn of `DateParser::Parse`'s legacy loop; false where it returns false.
pub fn legacy(
    s: &mut Scanner,
    token: Token,
    (day, time, tz): (&mut Day, &mut Time, &mut Zone),
    has_read_number: &mut bool,
) -> bool {
    match token {
        Token::Number { value: n, .. } => {
            *has_read_number = true;
            if s.skip_symbol(b':') {
                if s.skip_symbol(b':') {
                    if !time.is_empty() {
                        return false;
                    }
                    time.add(n);
                    time.add(0);
                } else {
                    if !time.add(n) {
                        return false;
                    }
                    if s.peek().is_symbol(b'.') {
                        s.next();
                    }
                }
            } else if s.skip_symbol(b'.') && time.is_expecting(n) {
                time.add(n);
                if s.peek().number().is_none() {
                    return false;
                }
                time.add_final(read_milliseconds(s.next()));
            } else if tz.is_expecting(n) {
                tz.set_absolute_minute(n);
            } else if time.is_expecting(n) {
                time.add_final(n);
                let peek = s.peek();
                if !matches!(peek, Token::End | Token::WhiteSpace)
                    && !peek.is_z()
                    && peek.sign().is_none()
                {
                    return false;
                }
            } else {
                if !day.add(n) {
                    return false;
                }
                s.skip_symbol(b'-');
            }
        }
        Token::Keyword { kind, value, .. } => {
            if kind == Kind::AmPm && !time.is_empty() {
                time.hour_offset = value;
            } else if kind == Kind::MonthName {
                day.named_month = value;
                s.skip_symbol(b'-');
            } else if kind == Kind::TimeZoneName && *has_read_number {
                tz.set(value);
            } else if *has_read_number || s.peek().number().is_some() {
                return false;
            }
        }
        t if t.sign().is_some() && (tz.is_utc() || !time.is_empty()) => {
            tz.set_sign(t.sign().expect("a sign"));
            let (n, length) = match s.peek() {
                Token::Number { value, len } => {
                    s.next();
                    (value, len)
                }
                _ => (0, 0),
            };
            *has_read_number = true;
            if s.peek().is_symbol(b':') {
                tz.set_absolute_hour(n);
                tz.set_absolute_minute(NONE);
            } else if matches!(length, 1 | 2) {
                tz.set_absolute_hour(n);
                tz.set_absolute_minute(0);
            } else if matches!(length, 3 | 4) {
                tz.set_absolute_hour(n / 100);
                tz.set_absolute_minute(n % 100);
            } else {
                return false;
            }
        }
        t if (t.sign().is_some() || t.is_symbol(b')')) && *has_read_number => return false,
        _ => {}
    }
    true
}
