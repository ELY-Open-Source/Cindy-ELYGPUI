//! Node 24's `pathToFileURL`, for either platform.

use url::Host;

use crate::node_path::{fold, posix};

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

/// `path.posix.resolve` of an absolute path: folded, with no separator at its end.
fn resolved(path: &str) -> String {
    let folded = posix::normalize(path);
    match folded.strip_suffix('/') {
        Some(cut) if !cut.is_empty() => cut.to_owned(),
        _ => folded,
    }
}

fn sep(b: u8) -> bool {
    b == b'/' || b == b'\\'
}

/// The root `path.win32.resolve` matches: the device and where the tail starts.
fn root(path: &str) -> (String, usize) {
    let (b, len) = (path.as_bytes(), path.len());
    let run =
        |from: usize, want: bool| from + b[from..].iter().take_while(|c| sep(**c) == want).count();
    if len >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        let rooted = len > 2 && sep(b[2]);
        return (path[..2].to_owned(), if rooted { 3 } else { 2 });
    }
    if !b.first().is_some_and(|c| sep(*c)) {
        return (String::new(), 0);
    }
    if len == 1 || !sep(b[1]) {
        return (String::new(), 1);
    }
    let host = run(2, false);
    let share = run(host.min(len), true);
    if host == 2 || host == len || share == len {
        return (String::new(), 0);
    }
    let first = &path[2..host];
    match first == "." || first == "?" {
        true => (format!("\\\\{first}"), 4),
        false => {
            let end = run(share, false);
            (format!("\\\\{first}\\{}", &path[share..end]), end)
        }
    }
}

/// `path.win32.resolve` of an absolute path; one with no device takes the folder `cwd`'s.
fn resolved_win32(path: &str, cwd: &str) -> String {
    let (mut device, end) = root(path);
    if device.is_empty() {
        device = root(cwd).0;
    }
    let tail = fold(&path[end..], false, "\\", |c| c == '/' || c == '\\');
    format!("{device}\\{tail}")
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
        true => resolved_win32(path, cwd),
        false => resolved(path),
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
