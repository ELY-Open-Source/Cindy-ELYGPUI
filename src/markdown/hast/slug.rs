//! `rehype-slug` on github-slugger 2: a heading's id from its text.

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::Deserialize;

use crate::v8_classes::within;

#[derive(Deserialize)]
struct Classes {
    dropped: Vec<[u32; 2]>,
}

/// The code points the slugger's regex drops, read off it by `sync-golden.mjs`.
static CLASSES: LazyLock<Classes> = LazyLock::new(|| {
    let text = include_str!("../../../resources/slug-classes.json");
    serde_json::from_str(text).expect("the bundled slug classes parse")
});

/// `BananaSlug`: the slugs one document has given out.
#[derive(Default)]
pub struct Slugs {
    occurrences: HashMap<String, u32>,
}

impl Slugs {
    /// `slug`: lower case, the dropped characters gone, spaces as hyphens, then made unique.
    pub fn slug(&mut self, value: &str) -> String {
        let kept = value.to_lowercase();
        let kept = kept.chars().filter(|ch| !within(&CLASSES.dropped, *ch));
        let original: String = kept.map(|ch| if ch == ' ' { '-' } else { ch }).collect();
        let mut result = original.clone();
        while self.occurrences.contains_key(&result) {
            let count = self.occurrences.entry(original.clone()).or_default();
            *count += 1;
            result = format!("{original}-{count}");
        }
        self.occurrences.insert(result.clone(), 0);
        result
    }
}
