//! `collectCachedGeneratedFiles`: one result per first row, while that row lives.

use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
};

use super::{File, collect};
use crate::chat_message::merge::{Fields, Message};

struct Entry {
    working_dir: String,
    rows: Vec<Weak<Fields>>,
    files: Vec<File>,
}

impl Entry {
    fn is(&self, messages: &[&Message], working_dir: &str) -> bool {
        let same = |(held, row): (&Weak<Fields>, &&Message)| held.as_ptr() == Rc::as_ptr(row);
        self.working_dir == working_dir
            && self.rows.len() == messages.len()
            && self.rows.iter().zip(messages).all(same)
    }
}

thread_local! {
    static CACHE: RefCell<HashMap<*const Fields, Entry>> = RefCell::default();
}

/// `collect`, reused while the same rows come in the same order.
pub fn collect_cached(messages: &[&Message], working_dir: &str) -> Vec<File> {
    let Some(first) = messages.first() else {
        return Vec::new();
    };
    let key = Rc::as_ptr(first);
    let hit = CACHE.with_borrow(|cache| {
        let entry = cache.get(&key)?;
        entry.is(messages, working_dir).then(|| entry.files.clone())
    });
    if let Some(files) = hit {
        return files;
    }
    let rows: Vec<&Fields> = messages.iter().map(|row| &***row).collect();
    let files = collect(&rows, working_dir);
    let entry = Entry {
        working_dir: working_dir.to_owned(),
        rows: messages.iter().map(|row| Rc::downgrade(row)).collect(),
        files: files.clone(),
    };
    CACHE.with_borrow_mut(|cache| {
        // A `WeakMap` drops an entry with its key.
        cache.retain(|_, entry| entry.rows[0].strong_count() > 0);
        cache.insert(key, entry);
    });
    files
}

#[cfg(test)]
pub fn held() -> usize {
    CACHE.with_borrow(HashMap::len)
}
