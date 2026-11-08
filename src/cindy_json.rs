use std::{collections::BTreeMap, fs, io::ErrorKind, path::Path};

use anyhow::{Context as _, Result, bail};
use serde_json::value::RawValue;

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

#[cfg(test)]
mod tests {
    use super::{boolean, number, object, truthy};

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
