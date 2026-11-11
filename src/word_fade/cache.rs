//! The module's two caches: a state per streamed message, and `splitWords`' answers.

use std::{cell::RefCell, rc::Rc};

use indexmap::IndexMap;

use super::Fade;
use crate::word_break;

/// `WORD_FADE_STATE_CACHE_MAX`.
const STATES: usize = 64;
/// `SPLIT_CACHE_MAX`.
const TEXTS: usize = 500;
/// `SPLIT_CACHE_MAX_BYTES`.
const BYTES: usize = 2 * 1024 * 1024;

/// `wordFadeStateCache`: the message asked for longest ago comes first.
#[derive(Default)]
pub struct States(IndexMap<String, Rc<RefCell<Fade>>>);

impl States {
    /// `getOrCreateWordFadeState`: a message with no key gets a state nothing keeps.
    pub fn get(&mut self, key: Option<&str>) -> Rc<RefCell<Fade>> {
        let Some(key) = key.filter(|key| !key.is_empty()) else {
            return Rc::default();
        };
        let state = self.0.shift_remove(key).unwrap_or_default();
        self.0.insert(key.to_owned(), state.clone());
        if self.0.len() > STATES {
            self.0.shift_remove_index(0);
        }
        state
    }

    /// `releaseWordFadeState`.
    pub fn release(&mut self, key: Option<&str>) {
        if let Some(key) = key {
            self.0.shift_remove(key);
        }
    }
}

/// `splitCache` with `splitCacheBytes`.
#[derive(Default)]
struct Split {
    held: IndexMap<String, (Rc<Vec<String>>, usize)>,
    bytes: usize,
}

thread_local! {
    static SPLIT: RefCell<Split> = RefCell::default();
}

fn units(text: &str) -> usize {
    text.encode_utf16().count()
}

/// `splitWords`: a text's words, kept for the next parse of a growing answer.
pub fn split_words(text: &str) -> Rc<Vec<String>> {
    SPLIT.with_borrow_mut(|cache| {
        if let Some(hit) = cache.held.shift_remove(text) {
            let words = hit.0.clone();
            cache.held.insert(text.to_owned(), hit);
            return words;
        }
        let words = Rc::new(word_break::split(text));
        let payload = words.iter().map(|word| units(word) * 2 + 8);
        let bytes = units(text) * 2 + payload.sum::<usize>();
        // One long growing tail must not push out every short text.
        if bytes > BYTES {
            return words;
        }
        cache.held.insert(text.to_owned(), (words.clone(), bytes));
        cache.bytes += bytes;
        while cache.held.len() > TEXTS || cache.bytes > BYTES {
            let Some((_, (_, gone))) = cache.held.shift_remove_index(0) else {
                break;
            };
            cache.bytes -= gone;
        }
        words
    })
}

#[cfg(test)]
pub fn held() -> (Vec<String>, usize) {
    SPLIT.with_borrow(|cache| (cache.held.keys().cloned().collect(), cache.bytes))
}
