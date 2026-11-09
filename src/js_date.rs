//! `Date.parse` for the ISO strings `toISOString` writes; NaN otherwise.

use std::sync::LazyLock;

use regex::Regex;

static ISO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"^([+-][0-9]{6}|[0-9]{4})(?:-([0-9]{2})(?:-([0-9]{2}))?)?",
        r"(?:T([0-9]{2}):([0-9]{2})(?::([0-9]{2})(?:\.([0-9]+))?)?(Z|[+-][0-9]{2}:[0-9]{2})?)?$"
    ))
    .expect("valid pattern")
});

/// Days from 1970-01-01 to the first of `month` in `year`.
fn days_to_month(year: i64, month: i64) -> i64 {
    let (y, m) = if month <= 2 {
        (year - 1, month + 9)
    } else {
        (year, month - 3)
    };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * m + 2) / 5;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `Date.parse` on the ECMAScript date-time format, offset-less times read as UTC.
pub fn parse(text: &str) -> f64 {
    let Some(c) = ISO.captures(text) else {
        return f64::NAN;
    };
    let num = |i: usize, default: i64| {
        c.get(i)
            .map_or(default, |m| m.as_str().parse().unwrap_or(-1))
    };
    let year = num(1, 0);
    if c[1].len() == 7 && year == 0 && c[1].starts_with('-') {
        return f64::NAN;
    }
    let (month, day) = (num(2, 1), num(3, 1));
    let (hour, minute, second) = (num(4, 0), num(5, 0), num(6, 0));
    let ms = c.get(7).map_or(0, |m| {
        let digits = format!("{:0<3}", &m.as_str()[..m.len().min(3)]);
        digits.parse::<i64>().expect("three digits")
    });
    let valid = (1..=12).contains(&month)
        && (1..=31).contains(&day)
        && minute < 60
        && second < 60
        && (hour < 24 || (hour == 24 && minute == 0 && second == 0 && ms == 0));
    if !valid {
        return f64::NAN;
    }
    let offset = match c.get(8).map(|m| m.as_str()) {
        None | Some("Z") => 0,
        Some(o) => {
            let sign = if o.starts_with('-') { -1 } else { 1 };
            let (h, m): (i64, i64) = (
                o[1..3].parse().expect("digits"),
                o[4..6].parse().expect("digits"),
            );
            if h > 23 || m > 59 {
                return f64::NAN;
            }
            sign * (h * 60 + m)
        }
    };
    let days = days_to_month(year, month) + day - 1;
    let minutes = days * 1440 + hour * 60 + minute - offset;
    ((minutes * 60 + second) * 1000 + ms) as f64
}

#[cfg(test)]
mod tests {
    #[test]
    fn iso_strings_parse_as_v8_does() {
        // Values from Node 26's `Date.parse` with TZ=UTC.
        for (text, want) in [
            ("2021-02-30", 1_614_643_200_000.0),
            ("2026-01-01T24:00:00Z", 1_767_312_000_000.0),
            ("2026-01-01T00:00:00.1234Z", 1_767_225_600_123.0),
            ("2026-01-01T00:00Z", 1_767_225_600_000.0),
            ("2026-01-01T00:00:00", 1_767_225_600_000.0),
            ("2026", 1_767_225_600_000.0),
            ("+002026-01-01T00:00:00Z", 1_767_225_600_000.0),
            ("2026-01-01T00:00:00+05:30", 1_767_205_800_000.0),
            ("1969-12-31T23:59:59.999Z", -1.0),
        ] {
            assert_eq!(super::parse(text), want, "{text}");
        }
        for text in [
            "2021-13-01",
            "2026-01-01T24:00:01Z",
            "2026-01-01T00:00:00.Z",
            "",
            "x",
        ] {
            assert!(super::parse(text).is_nan(), "{text}");
        }
    }
}
