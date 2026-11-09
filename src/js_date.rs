//! V8's `Date.parse` (`ParseDateTimeString`), `toISOString` and `Date.now`.

use std::sync::LazyLock;

use jiff::{
    civil::DateTime,
    tz::{AmbiguousOffset, TimeZone},
};

mod parser;

const MS_PER_DAY: f64 = 86_400_000.0;
/// `DateCache::kMaxTimeInMs`.
const MAX_TIME: f64 = 8.64e15;
/// `kMaxTimeBeforeUTCInMs`: a month past the largest time.
const MAX_TIME_BEFORE_UTC: f64 = MAX_TIME + 30.0 * MS_PER_DAY;

/// The host's zone, read once as V8's date cache does.
static LOCAL: LazyLock<TimeZone> = LazyLock::new(|| {
    let zone = TimeZone::system();
    log::info!(target: "js-date", "local time zone: {}", zone.iana_name().unwrap_or("unnamed"));
    zone
});

/// `Date.now()`: whole milliseconds since 1970.
pub fn now() -> f64 {
    let since = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the clock is after 1970");
    since.as_millis() as f64
}

/// `MakeDay`: days from 1970 to a date, the month allowed to run past a year.
fn make_day(year: f64, month: f64, date: f64) -> f64 {
    if !(-1e6..=1e6).contains(&year) || !(-1e7..=1e7).contains(&month) || !date.is_finite() {
        return f64::NAN;
    }
    let (mut y, mut m) = (year as i64, month as i64);
    y += m / 12;
    m %= 12;
    if m < 0 {
        m += 12;
        y -= 1;
    }
    let days = |y: i64| {
        let y = y + 399_999;
        365 * y + y / 4 - y / 100 + y / 400
    };
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let before = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334][m as usize];
    let from_month = before + i64::from(leap && m >= 2);
    (days(y) - days(1970) + from_month - 1) as f64 + date.trunc()
}

/// The zone's offset for a local wall time, ICU's `UCAL_TZ_LOCAL_FORMER` both ways.
fn local_offset_ms(local_ms: i64, zone: &TimeZone) -> i64 {
    let (days, rest) = (
        local_ms.div_euclid(86_400_000),
        local_ms.rem_euclid(86_400_000),
    );
    let (mut year, month, day) = civil(days);
    // Shifting whole 400-year cycles keeps the calendar; jiff stops at 9999.
    if year.abs() > 9999 {
        year -= (year - year.signum() * 9600).div_euclid(400) * 400;
    }
    let dt = DateTime::new(
        year as i16,
        month as i8,
        day as i8,
        (rest / 3_600_000) as i8,
        (rest / 60_000 % 60) as i8,
        (rest / 1000 % 60) as i8,
        0,
    )
    .expect("a valid civil time");
    let offset = match zone.to_ambiguous_timestamp(dt).offset() {
        AmbiguousOffset::Unambiguous { offset } => offset,
        AmbiguousOffset::Gap { before, .. } | AmbiguousOffset::Fold { before, .. } => before,
    };
    i64::from(offset.seconds()) * 1000
}

/// `Date.parse` in the host's zone.
pub fn parse(text: &str) -> f64 {
    parse_in(text, &LOCAL)
}

/// `ParseDateTimeString` with local times read in `zone`.
fn parse_in(text: &str, zone: &TimeZone) -> f64 {
    let units: Vec<u16> = text.encode_utf16().collect();
    let Some(f) = parser::parse(&units) else {
        return f64::NAN;
    };
    let day = make_day(f[0], f[1], f[2]);
    let time = f[3] * 3_600_000.0 + f[4] * 60_000.0 + f[5] * 1000.0 + f[6];
    let mut date = match day.is_finite() {
        true => time + day * MS_PER_DAY,
        false => f64::NAN,
    };
    if f[7].is_nan() {
        if !(-MAX_TIME_BEFORE_UTC..=MAX_TIME_BEFORE_UTC).contains(&date) {
            return f64::NAN;
        }
        date -= local_offset_ms(date as i64, zone) as f64;
    } else {
        date -= f[7] * 1000.0;
    }
    match (-MAX_TIME..=MAX_TIME).contains(&date) {
        true => date.trunc() + 0.0,
        false => f64::NAN,
    }
}

/// The date of a day count from 1970-01-01.
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}

/// `new Date(ms).toISOString()`; None where it throws a `RangeError`.
pub fn iso(ms: i64) -> Option<String> {
    if !(-8_640_000_000_000_000..=8_640_000_000_000_000).contains(&ms) {
        return None;
    }
    let (days, rest) = (ms.div_euclid(86_400_000), ms.rem_euclid(86_400_000));
    let (year, month, day) = civil(days);
    let year = match year {
        0..=9999 => format!("{year:04}"),
        _ => format!("{}{:06}", if year < 0 { '-' } else { '+' }, year.abs()),
    };
    let (h, m, s, milli) = (
        rest / 3_600_000,
        rest / 60_000 % 60,
        rest / 1000 % 60,
        rest % 1000,
    );
    Some(format!(
        "{year}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}.{milli:03}Z"
    ))
}

#[cfg(test)]
mod tests;
