use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result, bail};
use serde_json::{Map, Value};

pub const FILE: &str = "cindy-elygpui-preferences.json";

/// UI preferences under Cindy's localStorage key names.
pub struct Preferences {
    path: PathBuf,
    values: Map<String, Value>,
}

impl gpui::Global for Preferences {}

impl Preferences {
    pub fn load(user_data: &Path) -> Result<Self> {
        let path = user_data.join(FILE);
        let raw = match fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Ok(Self {
                    path,
                    values: Map::new(),
                });
            }
            Err(error) => return Err(error).with_context(|| format!("read {}", path.display())),
        };
        match serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))? {
            Value::Object(values) => Ok(Self { path, values }),
            other => bail!("{} is not a JSON object: {other}", path.display()),
        }
    }

    pub fn text(&self, key: &str) -> Result<Option<&str>> {
        match self.values.get(key) {
            None => Ok(None),
            Some(Value::String(text)) => Ok(Some(text)),
            Some(other) => bail!("preference {key} is not a string: {other}"),
        }
    }

    /// `localStorage.setItem`: written beside the file, then renamed over it.
    pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
        let mut next = self.values.clone();
        next.insert(key.into(), value.into());
        self.save(next)?;
        log::debug!(target: "preferences", "{key} = {value}");
        Ok(())
    }

    /// `localStorage.removeItem`.
    pub fn remove(&mut self, key: &str) -> Result<()> {
        let mut next = self.values.clone();
        if next.remove(key).is_none() {
            return Ok(());
        }
        self.save(next)?;
        log::debug!(target: "preferences", "{key} removed");
        Ok(())
    }

    fn save(&mut self, next: Map<String, Value>) -> Result<()> {
        let text = serde_json::to_string_pretty(&Value::Object(next.clone()))?;
        let tmp = self.path.with_extension("json.tmp");
        if let Err(error) = fs::write(&tmp, text).and_then(|()| fs::rename(&tmp, &self.path)) {
            // Cleanup errors never hide the first.
            let _ = fs::remove_file(&tmp);
            return Err(error).with_context(|| format!("write {}", self.path.display()));
        }
        self.values = next;
        Ok(())
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

    #[test]
    fn writes_keep_other_keys_and_survive_a_reload() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE), r#"{"language":"ko"}"#).unwrap();
        let mut prefs = Preferences::load(dir.path()).unwrap();
        prefs.set("sidebar-width", "300").unwrap();
        let again = Preferences::load(dir.path()).unwrap();
        assert_eq!(again.text("sidebar-width").unwrap(), Some("300"));
        assert_eq!(again.text("language").unwrap(), Some("ko"));
        std::fs::remove_file(dir.path().join(FILE)).unwrap();
        std::fs::create_dir(dir.path().join(FILE)).unwrap();
        assert!(prefs.set("sidebar-rail", "true").is_err());
        assert_eq!(
            prefs.text("sidebar-rail").unwrap(),
            None,
            "a failed write changes nothing"
        );
        assert!(!dir.path().join(format!("{FILE}.tmp")).exists());
    }
}
