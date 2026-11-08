#![cfg_attr(
    not(test),
    expect(dead_code, reason = "the tree's editors land in 6.3")
)]

use std::{
    fs,
    path::{Path, PathBuf},
};

use super::Layout;
use crate::cindy_json;

pub const FILE: &str = "layout.v1.json";

/// Cindy's `LayoutStore`: one tree for the app, healed on read, strict on write.
pub struct LayoutStore {
    path: PathBuf,
    layout: Layout,
}

impl gpui::Global for LayoutStore {}

impl LayoutStore {
    /// `ensurePersisted`: a missing or broken file is rewritten as the default tree.
    pub fn open(user_data: &Path) -> Self {
        let path = user_data.join(FILE);
        let read = match fs::read_to_string(&path) {
            Ok(text) => cindy_json::roundtrip(&text).map_err(|e| e.to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err("file missing".into()),
            Err(e) => Err(e.to_string()),
        };
        let (layout, fallback) = Layout::coerce(read.as_ref().ok());
        let store = LayoutStore { path, layout };
        if let Some(reason) = fallback {
            let read_error = read.err().map_or("null".into(), |e| format!("'{e}'"));
            log::warn!(
                target: "layout",
                "layout store falling back to default layout {{ path: '{}', readError: {read_error}, reason: '{reason}' }}",
                store.path.display()
            );
            store.persist();
        }
        store
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// `setLayout`: an invalid tree is the caller's bug and changes nothing. Ok says if it was saved.
    pub fn set(&mut self, layout: Layout) -> Result<bool, String> {
        layout.validate()?;
        self.layout = layout;
        Ok(self.persist())
    }

    /// `reset`: back to the default tree; says if it was saved.
    pub fn reset(&mut self) -> bool {
        self.layout = Layout::default_tree();
        self.persist()
    }

    /// Writes beside the file, then renames over it; a failure is logged and kept in memory.
    fn persist(&self) -> bool {
        let tmp = self.path.with_extension("json.tmp");
        let text =
            serde_json::to_string_pretty(&self.layout.to_value()).expect("a layout serializes");
        match fs::write(&tmp, text).and_then(|()| fs::rename(&tmp, &self.path)) {
            Ok(()) => true,
            Err(error) => {
                log::warn!(
                    target: "layout",
                    "layout store persist failed {{ path: '{}', error: '{error}' }}",
                    self.path.display()
                );
                // Cleanup errors never hide the first.
                let _ = fs::remove_file(&tmp);
                false
            }
        }
    }
}
