//! `Date.parse` against Cindy's Electron (golden `js-date`) and `toISOString` against V8.

use jiff::tz::TimeZone;

use super::{iso, parse_in};
use crate::rich_answer::js::{self, Js};

#[test]
fn every_case_parses_as_electron_does() {
    let cases = js::parse(include_str!("../../tests/fixtures/js-date/cases.json")).unwrap();
    let expected = js::parse(include_str!("../../tests/fixtures/js-date/expected.json")).unwrap();
    let mut failures = Vec::new();
    for (zone, values) in expected.obj().unwrap()["zones"].obj().unwrap() {
        let tz = TimeZone::get(zone).unwrap();
        for (text, want) in cases.arr().unwrap().iter().zip(values.arr().unwrap()) {
            let text = text.str().unwrap();
            let got = parse_in(text, &tz);
            let same = match want {
                Js::Num(want) => got == *want,
                _ => got.is_nan(),
            };
            if !same {
                failures.push(format!(
                    "{zone} {text:?}: got {got}, want {}",
                    js::string(Some(want))
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

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
        assert_eq!(parse_in(text, &TimeZone::UTC), want, "{text}");
    }
    for text in [
        "2021-13-01",
        "2026-01-01T24:00:01Z",
        "2026-01-01T00:00:00.Z",
        "",
        "x",
    ] {
        assert!(parse_in(text, &TimeZone::UTC).is_nan(), "{text}");
    }
}

#[test]
fn iso_strings_match_v8() {
    // Values from Node 26's `toISOString`.
    for (ms, want) in [
        (0, "1970-01-01T00:00:00.000Z"),
        (-1, "1969-12-31T23:59:59.999Z"),
        (1_767_225_600_123, "2026-01-01T00:00:00.123Z"),
        (951_782_400_000, "2000-02-29T00:00:00.000Z"),
        (253_402_300_800_000, "+010000-01-01T00:00:00.000Z"),
        (-62_198_755_200_000, "-000001-01-01T00:00:00.000Z"),
        (8_640_000_000_000_000, "+275760-09-13T00:00:00.000Z"),
    ] {
        assert_eq!(iso(ms).as_deref(), Some(want), "{ms}");
    }
    assert_eq!(iso(8_640_000_000_000_001), None);
    assert_eq!(iso(-8_640_000_000_000_001), None);
    assert_eq!(iso(i64::MIN), None);
    assert_eq!(iso(i64::MAX), None);
    assert_eq!(
        iso(-8_640_000_000_000_000).as_deref(),
        Some("-271821-04-20T00:00:00.000Z")
    );
}
