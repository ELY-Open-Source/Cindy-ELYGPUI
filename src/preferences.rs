use std::{fs, io::ErrorKind, path::Path};

use anyhow::{Context as _, Result, bail};
use serde_json::{Map, Value};

pub const FILE: &str = "cindy-elygpui-preferences.json";

/// UI preferences under Cindy's localStorage key names.
pub struct Preferences(Map<String, Value>);

impl Preferences {
    pub fn load(user_data: &Path) -> Result<Self> {
        let path = user_data.join(FILE);
        let raw = match fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Self(Map::new())),
            Err(error) => return Err(error).with_context(|| format!("read {}", path.display())),
        };
        match serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))? {
            Value::Object(map) => Ok(Self(map)),
            other => bail!("{} is not a JSON object: {other}", path.display()),
        }
    }

    pub fn text(&self, key: &str) -> Result<Option<&str>> {
        match self.0.get(key) {
            None => Ok(None),
            Some(Value::String(text)) => Ok(Some(text)),
            Some(other) => bail!("preference {key} is not a string: {other}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FILE, Preferences};

    #[test]
    fn reads_strings_and_rejects_other_shapes() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            Preferences::load(dir.path())
                .unwrap()
                .text("language")
                .unwrap(),
            None
        );
        std::fs::write(dir.path().join(FILE), r#"{"language":"ko","n":1}"#).unwrap();
        let prefs = Preferences::load(dir.path()).unwrap();
        assert_eq!(prefs.text("language").unwrap(), Some("ko"));
        assert!(prefs.text("n").is_err());
        std::fs::write(dir.path().join(FILE), "[]").unwrap();
        assert!(Preferences::load(dir.path()).is_err());
    }
}
