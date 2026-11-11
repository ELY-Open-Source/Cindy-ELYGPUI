//! The breadth-first walks: files whose path ends with `/<href>`.

use std::collections::{HashMap, HashSet, VecDeque};

use super::Options;
use crate::node_path::{basename, join};

/// `DEFAULT_IGNORE_DIRS`.
const IGNORED: [&str; 16] = [
    ".git",
    "node_modules",
    "__pycache__",
    ".cache",
    ".vscode",
    ".idea",
    "coverage",
    ".sivi",
    ".cursor",
    "build",
    "dist",
    "out",
    ".next",
    ".turbo",
    ".vite",
    ".pnpm-store",
];

/// `normalizeForSuffix`: one separator, and no case on Windows.
fn fold(path: &str) -> String {
    let slashed = path.replace('\\', "/");
    match cfg!(windows) {
        true => slashed.to_lowercase(),
        false => slashed,
    }
}

fn suffix(href: &str) -> String {
    format!("/{}", fold(href).trim_start_matches('/'))
}

/// `namedIgnoredDirs`: the ignored folders `href` passes through, each with its parent.
fn named(href: &str) -> HashSet<String> {
    let folded = fold(href);
    let segments: Vec<&str> = folded.split('/').filter(|s| !s.is_empty()).collect();
    let inner = 1..segments.len().saturating_sub(1);
    let pairs = inner.filter(|at| IGNORED.contains(&segments[*at]));
    pairs
        .map(|at| format!("{}/{}", segments[at - 1], segments[at]))
        .collect()
}

/// `ignoredDirPair`.
fn pair(parent: &str, name: &str) -> String {
    format!("{}/{}", fold(basename(parent)), fold(name))
}

enum Entry {
    Folder,
    File,
    Other,
}

/// `readdir` with file types: every entry, or the folder's failure.
fn entries(dir: &str) -> std::io::Result<Vec<(String, Entry)>> {
    let read = std::fs::read_dir(dir)?.map(|entry| {
        let entry = entry?;
        let kind = entry.file_type()?;
        let kind = match (kind.is_dir(), kind.is_file()) {
            (true, _) => Entry::Folder,
            (_, true) => Entry::File,
            _ => Entry::Other,
        };
        Ok((entry.file_name(), kind))
    });
    let mut all = read.collect::<std::io::Result<Vec<_>>>()?;
    // libuv's `scandir` sorts by bytes on POSIX; on Windows it keeps the system's order.
    if cfg!(unix) {
        all.sort_by(|(a, _), (b, _)| a.as_encoded_bytes().cmp(b.as_encoded_bytes()));
    }
    let named = all.into_iter();
    Ok(named
        .map(|(name, kind)| (name.to_string_lossy().into_owned(), kind))
        .collect())
}

/// `bfsFindBySuffix`; a folder that cannot be read is passed by, as Cindy passes it.
pub fn find(root: &str, href: &str, options: &Options) -> Vec<String> {
    let (suffix, named) = (suffix(href), named(href));
    let mut out = Vec::new();
    let mut queue = VecDeque::from([(root.to_owned(), 0)]);
    while out.len() < options.max_candidates
        && let Some((dir, depth)) = queue.pop_front()
    {
        let Ok(entries) = entries(&dir) else { continue };
        for (name, kind) in entries {
            if out.len() >= options.max_candidates {
                break;
            }
            let path = join(&[&dir, &name]);
            match kind {
                Entry::Folder => {
                    let skipped =
                        IGNORED.contains(&name.as_str()) && !named.contains(&pair(&dir, &name));
                    if depth < options.max_depth && !skipped {
                        queue.push_back((path, depth + 1));
                    }
                }
                Entry::File if fold(&path).ends_with(&suffix) && (options.allowed)(&path) => {
                    out.push(path);
                }
                _ => {}
            }
        }
    }
    out
}

struct Bucket {
    suffix: String,
    named: HashSet<String>,
    out: Vec<String>,
}

/// `bfsFindManyBySuffix`: each href gets what `find` gives it alone, from one walk.
pub fn find_many(root: &str, hrefs: &[&str], options: &Options) -> HashMap<String, Vec<String>> {
    let mut buckets: Vec<Bucket> = Vec::new();
    let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
    let mut order: Vec<&str> = Vec::new();
    for href in hrefs {
        if order.contains(href) {
            continue;
        }
        let suffix = suffix(href);
        let name = suffix.rsplit('/').next().expect("a split yields a piece");
        by_name
            .entry(name.to_owned())
            .or_default()
            .push(buckets.len());
        order.push(href);
        buckets.push(Bucket {
            suffix,
            named: named(href),
            out: Vec::new(),
        });
    }
    let named = buckets.iter().flat_map(|bucket| &bucket.named);
    let enterable: HashSet<String> = named.cloned().collect();
    let full = |buckets: &[Bucket]| (buckets.iter()).all(|b| b.out.len() >= options.max_candidates);
    // `crossed` lists the ignored folders entered on the way down.
    let mut queue = VecDeque::from([(root.to_owned(), 0, Vec::<String>::new())]);
    while !full(&buckets)
        && let Some((dir, depth, crossed)) = queue.pop_front()
    {
        let Ok(entries) = entries(&dir) else { continue };
        for (name, kind) in entries {
            let path = join(&[&dir, &name]);
            match kind {
                Entry::Folder => {
                    let ignored = IGNORED.contains(&name.as_str());
                    let pair = pair(&dir, &name);
                    if depth >= options.max_depth || (ignored && !enterable.contains(&pair)) {
                        continue;
                    }
                    let mut crossed = crossed.clone();
                    if ignored {
                        crossed.push(pair);
                    }
                    queue.push_back((path, depth + 1, crossed));
                }
                Entry::File => {
                    let folded = fold(&path);
                    for at in by_name.get(&fold(&name)).into_iter().flatten() {
                        let bucket = &buckets[*at];
                        if bucket.out.len() < options.max_candidates
                            && crossed.iter().all(|pair| bucket.named.contains(pair))
                            && folded.ends_with(&bucket.suffix)
                            && (options.allowed)(&path)
                        {
                            buckets[*at].out.push(path.clone());
                        }
                    }
                }
                Entry::Other => {}
            }
        }
    }
    let filled = order.into_iter().zip(buckets);
    filled
        .map(|(href, bucket)| (href.to_owned(), bucket.out))
        .collect()
}
