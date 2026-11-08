use std::{collections::BTreeMap, fs, io::ErrorKind, path::Path};

use anyhow::{Context as _, Result, bail};
use serde_json::{Value, value::RawValue};

/// A JSON object's fields kept as text, so one value `JSON.parse` takes but serde cannot
/// hold (such as `1e400`) never spoils its siblings.
pub type Fields = BTreeMap<String, Box<RawValue>>;

pub fn object(text: &str) -> Result<Fields> {
    if !crate::theme_import::color::js_trim(text).starts_with('{') {
        bail!("root must be an object");
    }
    Ok(serde_json::from_str(text)?)
}

/// A Cindy JSON file's fields; none when it is missing.
pub fn read(path: &Path, max_bytes: Option<u64>) -> Result<Option<Fields>> {
    let meta = match fs::metadata(path) {
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        other => other.with_context(|| path.display().to_string())?,
    };
    if let Some(max) = max_bytes.filter(|max| meta.len() > *max) {
        bail!("{}: file exceeds {max} byte limit", path.display());
    }
    let text = fs::read_to_string(path).with_context(|| path.display().to_string())?;
    object(&text)
        .with_context(|| path.display().to_string())
        .map(Some)
}

/// A JSON number as `JSON.parse` reads it: past f64 it is infinite.
pub fn number(raw: Option<&RawValue>) -> Option<f64> {
    let text = raw?.get();
    text.starts_with(|c: char| c == '-' || c.is_ascii_digit())
        .then(|| text.parse().ok())
        .flatten()
}

pub fn boolean(raw: Option<&RawValue>) -> Option<bool> {
    serde_json::from_str(raw?.get()).ok()
}

pub fn string(raw: Option<&RawValue>) -> Option<String> {
    serde_json::from_str(raw?.get()).ok()
}

/// JavaScript truthiness of a field.
pub fn truthy(raw: Option<&RawValue>) -> bool {
    let Some(raw) = raw else { return false };
    match raw.get() {
        "false" | "null" => false,
        text if text.starts_with('"') => string(Some(raw)).is_some_and(|s| !s.is_empty()),
        _ => number(Some(raw)).is_none_or(|n| n != 0.0 && !n.is_nan()),
    }
}

/// What `JSON.parse` then `JSON.stringify` keeps: numbers past f64 become null, and whole
/// numbers are integers.
pub fn roundtrip(text: &str) -> Result<Value> {
    let raw: Box<RawValue> = serde_json::from_str(text)?;
    lift(&raw)
}

fn lift(raw: &RawValue) -> Result<Value> {
    let text = raw.get();
    Ok(match text.as_bytes()[0] {
        b'{' => Value::Object(
            serde_json::from_str::<BTreeMap<String, Box<RawValue>>>(text)?
                .iter()
                .map(|(k, v)| Ok((k.clone(), lift(v)?)))
                .collect::<Result<_>>()?,
        ),
        b'[' => Value::Array(
            serde_json::from_str::<Vec<Box<RawValue>>>(text)?
                .iter()
                .map(|v| lift(v))
                .collect::<Result<_>>()?,
        ),
        b'-' | b'0'..=b'9' => js_value(text.parse()?),
        _ => serde_json::from_str(text)?,
    })
}

/// JavaScript truthiness of a parsed value.
pub fn js_truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|n| n != 0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(_) | Value::Object(_)) => true,
    }
}

/// `Number(text)` for a string.
pub fn js_to_number(text: &str) -> f64 {
    let text = crate::theme_import::color::js_trim(text);
    if text.is_empty() {
        return 0.0;
    }
    let radix = [
        ("0x", 16),
        ("0X", 16),
        ("0o", 8),
        ("0O", 8),
        ("0b", 2),
        ("0B", 2),
    ];
    if let Some((digits, radix)) = radix
        .iter()
        .find_map(|(p, r)| Some((text.strip_prefix(p)?, *r)))
    {
        return match digits.chars().all(|c| c.is_digit(radix)) && !digits.is_empty() {
            true => digits.chars().fold(0.0, |n, c| {
                n * f64::from(radix) + f64::from(c.to_digit(radix).unwrap_or(0))
            }),
            false => f64::NAN,
        };
    }
    let (sign, body) = match text.as_bytes()[0] {
        b'-' => (-1.0, &text[1..]),
        b'+' => (1.0, &text[1..]),
        _ => (1.0, text),
    };
    if body == "Infinity" {
        return sign * f64::INFINITY;
    }
    let (mantissa, exponent) = body.split_once(['e', 'E']).unwrap_or((body, "0"));
    let exponent = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
    let (int, frac) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    let valid = digits(int)
        && digits(frac)
        && !(int.is_empty() && frac.is_empty())
        && !exponent.is_empty()
        && digits(exponent);
    if valid {
        sign * body.parse::<f64>().unwrap_or(f64::NAN)
    } else {
        f64::NAN
    }
}

/// An f64 as `JSON.stringify` writes it.
pub fn js_value(n: f64) -> Value {
    if !n.is_finite() {
        Value::Null
    } else if n.fract() == 0.0 && n.abs() < 9e15 {
        Value::from(n as i64)
    } else {
        Value::from(n)
    }
}

/// JavaScript's `Math.round`: halves go up.
pub fn js_round(value: f64) -> f64 {
    (value + 0.5).floor()
}

/// `Number.prototype.toString()`.
pub fn js_number(n: f64) -> String {
    if n.is_nan() {
        return "NaN".into();
    }
    if n.is_infinite() {
        return if n > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    if n == 0.0 {
        return "0".into();
    }
    let shortest = format!("{:e}", n.abs());
    let k = shortest
        .split_once('e')
        .expect("{:e} has an exponent")
        .0
        .replace('.', "")
        .len();
    let closest = format!("{:.*e}", k - 1, n.abs());
    let sci = if closest.parse() == Ok(n.abs()) {
        closest
    } else {
        shortest
    };
    let (mantissa, exp) = sci.split_once('e').expect("{:e} has an exponent");
    let digits = mantissa.replace('.', "");
    let k = digits.len() as i32;
    let point = exp.parse::<i32>().expect("{:e} exponent") + 1;
    let body = if k <= point && point <= 21 {
        format!("{digits}{}", "0".repeat((point - k) as usize))
    } else if 0 < point && point <= 21 {
        format!(
            "{}.{}",
            &digits[..point as usize],
            &digits[point as usize..]
        )
    } else if -6 < point && point <= 0 {
        format!("0.{}{digits}", "0".repeat(-point as usize))
    } else {
        let sign = if point > 0 { "+" } else { "-" };
        let tail = if k > 1 {
            format!(".{}", &digits[1..])
        } else {
            String::new()
        };
        format!("{}{tail}e{sign}{}", &digits[..1], (point - 1).abs())
    };
    if n < 0.0 { format!("-{body}") } else { body }
}

/// `String(value)`; none is `undefined`.
pub fn js_string(value: Option<&Value>) -> String {
    match value {
        None => "undefined".into(),
        Some(Value::Null) => "null".into(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Number(n)) => js_number(n.as_f64().expect("JSON numbers are f64")),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(items)) => items
            .iter()
            .map(|v| {
                if v.is_null() {
                    String::new()
                } else {
                    js_string(Some(v))
                }
            })
            .collect::<Vec<_>>()
            .join(","),
        Some(Value::Object(_)) => "[object Object]".into(),
    }
}

/// `Number.prototype.toFixed`: a tie rounds away from zero.
pub fn js_to_fixed(n: f64, digits: usize) -> String {
    if !n.is_finite() || n.abs() >= 1e21 {
        return js_number(n);
    }
    // The f64's exact decimal expansion, so only a true tie rounds up.
    let exact = format!("{:.1100}", n.abs());
    let (int, frac) = exact.split_once('.').expect("a precision prints a point");
    let mut kept: Vec<u8> = format!("{int}{}", &frac[..digits]).into_bytes();
    if frac.as_bytes()[digits] >= b'5' {
        let mut i = kept.len();
        loop {
            if i == 0 {
                kept.insert(0, b'1');
                break;
            }
            i -= 1;
            if kept[i] == b'9' {
                kept[i] = b'0';
            } else {
                kept[i] += 1;
                break;
            }
        }
    }
    let text = String::from_utf8(kept).expect("digits are ASCII");
    let (int, frac) = text.split_at(text.len() - digits);
    let body = if digits == 0 {
        int.to_owned()
    } else {
        format!("{int}.{frac}")
    };
    if n < 0.0 { format!("-{body}") } else { body }
}

/// `encodeURIComponent`: UTF-8 bytes outside its unreserved set as `%XX`.
pub fn encode_uri_component(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// `String(value)` of a `JSON.parse`d value, numbers past f64 included.
pub fn js_string_raw(raw: &RawValue) -> Result<String> {
    let text = raw.get();
    Ok(match text.as_bytes()[0] {
        b'[' => serde_json::from_str::<Vec<Box<RawValue>>>(text)?
            .iter()
            .map(|v| match v.get() {
                "null" => Ok(String::new()),
                _ => js_string_raw(v),
            })
            .collect::<Result<Vec<_>>>()?
            .join(","),
        b'{' => "[object Object]".into(),
        b'-' | b'0'..=b'9' => js_number(text.parse()?),
        _ => js_string(Some(&serde_json::from_str(text)?)),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{boolean, js_number, js_string, number, object, roundtrip, truthy};

    #[test]
    fn encodes_uri_components_as_javascript_does() {
        let cases = [
            ("abc-_.!~*'()", "abc-_.!~*'()"),
            ("a b/c?d#e&f=g", "a%20b%2Fc%3Fd%23e%26f%3Dg"),
            ("任务", "%E4%BB%BB%E5%8A%A1"),
            ("%", "%25"),
        ];
        for (text, want) in cases {
            assert_eq!(super::encode_uri_component(text), want);
        }
    }

    #[test]
    fn fixes_decimals_as_javascript_does() {
        use super::js_to_fixed;
        let cases = [
            (0.125, 2, "0.13"),
            (2.5, 0, "3"),
            (1.005, 2, "1.00"),
            (9.995, 2, "9.99"),
            (999.95, 1, "1000.0"),
            (0.0, 2, "0.00"),
            (99.996, 2, "100.00"),
            (-0.004, 2, "-0.00"),
            (-0.1, 0, "-0"),
            (-0.0, 2, "0.00"),
            (-1.005, 2, "-1.00"),
            (f64::INFINITY, 1, "Infinity"),
            (f64::NEG_INFINITY, 1, "-Infinity"),
            (f64::NAN, 2, "NaN"),
            (1e21, 2, "1e+21"),
        ];
        for (n, d, want) in cases {
            assert_eq!(js_to_fixed(n, d), want, "{n}");
        }
    }

    #[test]
    fn writes_numbers_and_strings_as_javascript_does() {
        let cases = [
            (0.1 + 0.2, "0.30000000000000004"),
            (1e21, "1e+21"),
            (123456789012345680000.0, "123456789012345680000"),
            (1e-7, "1e-7"),
            (0.000001, "0.000001"),
            (-2.5e-10, "-2.5e-10"),
            (-0.0, "0"),
            (400.0, "400"),
            (137839762462415.62, "137839762462415.62"),
            (f64::NAN, "NaN"),
        ];
        for (n, text) in cases {
            assert_eq!(js_number(n), text);
        }
        let parsed = roundtrip(r#"{"a":[1e400,null,[2,"x"]],"b":{},"c":1.0}"#).unwrap();
        assert_eq!(
            parsed,
            json!({"a": [null, null, [2, "x"]], "b": {}, "c": 1})
        );
        assert_eq!(js_string(parsed.get("a")), ",,2,x");
        assert_eq!(js_string(parsed.get("b")), "[object Object]");
        assert_eq!(js_string(None), "undefined");
        let cases = [
            ("", 0.0),
            (" 300 ", 300.0),
            ("1e3", 1000.0),
            (".5", 0.5),
            ("5.", 5.0),
            ("-Infinity", f64::NEG_INFINITY),
            ("0x1F", 31.0),
            ("+2", 2.0),
            ("1e400", f64::INFINITY),
        ];
        for (text, n) in cases {
            assert_eq!(super::js_to_number(text), n, "{text:?}");
        }
        for text in [
            "inf", "NaN", "1e", "e3", ".", "12px", "0x", "--1", "1_000", "+-1",
        ] {
            assert!(super::js_to_number(text).is_nan(), "{text:?}");
        }
    }

    #[test]
    fn reads_fields_as_javascript_does() {
        let fields =
            object(r#"{"a":1e400,"b":true,"c":0,"d":"","e":{"x":-1e999},"f":"0","g":-0.0}"#)
                .unwrap();
        let f = |k: &str| fields.get(k).map(AsRef::as_ref);
        assert_eq!(number(f("a")), Some(f64::INFINITY));
        assert_eq!(boolean(f("b")), Some(true));
        assert!(truthy(f("b")) && truthy(f("e")) && truthy(f("f")) && truthy(f("a")));
        assert!(!truthy(f("c")) && !truthy(f("d")) && !truthy(f("g")) && !truthy(f("missing")));
        assert!(object("[1]").is_err());
    }
}
