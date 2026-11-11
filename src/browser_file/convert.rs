//! Node 24's `pathToFileURL`, for either platform.

use url::Host;

use crate::node_path::{posix, win32};

/// `lookup_table`: the ASCII a path may not carry into a URL as written.
const UNSAFE: &[u8] = b"\0\t\n\r \"#%?[\\]^|~";

/// `EncodePathChars`.
fn encoded(path: &str, windows: bool) -> String {
    let mut out = String::new();
    for c in path.chars() {
        match c {
            '\\' if windows => out.push('/'),
            c if c.is_ascii() && UNSAFE.contains(&(c as u8)) => {
                out.push_str(&format!("%{:02X}", c as u8));
            }
            c => out.push(c),
        }
    }
    out
}

/// The URL parser on a path: the controls at its end go, the rest of its set is escaped.
fn escaped(path: &str) -> String {
    let mut out = String::new();
    for c in path.trim_end_matches(|c| c < ' ').chars() {
        if c.is_ascii_graphic() && !"<>`{}".contains(c) {
            out.push(c);
            continue;
        }
        let mut bytes = [0; 4];
        for byte in c.encode_utf8(&mut bytes).bytes() {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// The URL parser's path state on a `file:` URL, which keeps a path's empty segments.
fn segments(path: &str) -> String {
    // ada asks only for a letter and a colon at the lone segment's start.
    let drive = |name: &str| {
        let b = name.as_bytes();
        b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
    };
    let mut kept: Vec<&str> = Vec::new();
    let mut names = path[1..].split('/').peekable();
    while let Some(name) = names.next() {
        if name == ".." && !matches!(&kept[..], [only] if drive(only)) {
            kept.pop();
        }
        match name {
            "." | ".." if names.peek().is_some() => {}
            "." | ".." => kept.push(""),
            name => kept.push(name),
        }
    }
    format!("/{}", kept.join("/"))
}

/// ada's `set_hostname` on a `file:` URL: `Err` where it refuses, `None` for no host.
fn hostname(raw: &str) -> Result<Option<String>, ()> {
    let clean: String = raw.chars().filter(|c| !"\t\n\r".contains(*c)).collect();
    let name = clean
        .split(['/', '\\', '?', '#'])
        .next()
        .unwrap_or_default();
    if name.is_empty() {
        return Ok(None);
    }
    let host = Host::parse(name).map_err(|_| ())?.to_string();
    Ok((host != "localhost").then_some(host))
}

/// A UNC path's URL: the server is the host.
fn unc(path: &str) -> Option<String> {
    let prefix = if path.starts_with("\\\\?\\UNC\\") {
        8
    } else {
        2
    };
    let end = prefix + path.get(prefix..)?.find('\\')?;
    if end == 2 {
        return None;
    }
    let host = hostname(&path[prefix..end]).ok()?.unwrap_or_default();
    Some(format!(
        "file://{host}{}",
        segments(&escaped(&encoded(&path[end..], true)))
    ))
}

/// `pathToFileURL(path).toString()` for an absolute path; `None` where Node throws.
pub fn to_url(path: &str, windows: bool, cwd: &str) -> Option<String> {
    if windows && path.starts_with("\\\\") {
        return unc(path);
    }
    let mut full = match windows {
        true => win32::resolve(path, cwd, |_| None),
        false => posix::resolve(path, "/"),
    };
    if windows && full.starts_with("\\\\") {
        return unc(&full);
    }
    // `resolve` drops the last separator, so it goes back.
    let (ends, sep) = match windows {
        true => (path.ends_with(['/', '\\']), '\\'),
        false => (path.ends_with('/'), '/'),
    };
    if ends && !full.ends_with(sep) {
        full.push('/');
    }
    // A drive's path gains the slash the parser gives it.
    let root = if windows { "/" } else { "" };
    let path = format!("{root}{}", escaped(&encoded(&full, windows)));
    Some(format!("file://{}", segments(&path)))
}
