//! Node 24's `path` for either platform: the calls Cindy's main process makes.

#[cfg(test)]
mod tests;

/// `normalizeString`: `.` and empty segments go, `..` takes the segment before it.
pub fn fold(
    path: &str,
    above_root: bool,
    separator: &str,
    is_separator: fn(char) -> bool,
) -> String {
    let mut kept: Vec<&str> = Vec::new();
    for segment in path.split(is_separator) {
        match segment {
            "" | "." => {}
            ".." if kept.last().is_some_and(|last| *last != "..") => {
                kept.pop();
            }
            ".." if !above_root => {}
            _ => kept.push(segment),
        }
    }
    kept.join(separator)
}

/// `path.join` on this host.
pub fn join(parts: &[&str]) -> String {
    match cfg!(windows) {
        true => win32::join(parts),
        false => posix::join(parts),
    }
}

/// `path.normalize` on this host.
pub fn normalize(path: &str) -> String {
    match cfg!(windows) {
        true => win32::normalize(path),
        false => posix::normalize(path),
    }
}

/// `path.basename` on this host.
pub fn basename(path: &str) -> &str {
    match cfg!(windows) {
        true => win32::basename(path),
        false => posix::basename(path),
    }
}

/// `path.resolve` on this host, from the folder `cwd`.
pub fn resolve(path: &str, cwd: &str) -> String {
    match cfg!(windows) {
        true => win32::resolve(path, cwd, |key| std::env::var(key).ok()),
        false => posix::resolve(path, cwd),
    }
}

/// `path.extname` on this host.
pub fn extname(path: &str) -> &str {
    match cfg!(windows) {
        true => win32::extname(path),
        false => posix::extname(path),
    }
}

/// `extname`'s scan from `start`: the last name's last dot, unless the name starts there.
fn ext(path: &str, start: usize, sep: fn(u8) -> bool) -> &str {
    let b = path.as_bytes();
    let (mut dot, mut end, mut part) = (None, None, start);
    let (mut trailing, mut state) = (true, 0);
    for i in (start..b.len()).rev() {
        if sep(b[i]) {
            if !trailing {
                part = i + 1;
                break;
            }
            continue;
        }
        if end.is_none() {
            trailing = false;
            end = Some(i + 1);
        }
        if b[i] != b'.' {
            if dot.is_some() {
                state = -1;
            }
        } else if dot.is_none() {
            dot = Some(i);
        } else if state != 1 {
            state = 1;
        }
    }
    let (Some(dot), Some(end)) = (dot, end) else {
        return "";
    };
    match state == 0 || (state == 1 && dot == end - 1 && dot == part + 1) {
        true => "",
        false => &path[dot..end],
    }
}

/// Where the name after the last separator run ends its folder, from `offset`.
fn folder_end(b: &[u8], offset: usize, sep: fn(u8) -> bool) -> Option<usize> {
    let mut trailing = true;
    for i in (offset..b.len()).rev() {
        match (sep(b[i]), trailing) {
            (true, false) => return Some(i),
            (true, true) => {}
            (false, _) => trailing = false,
        }
    }
    None
}

pub mod posix {
    /// `path.posix.resolve` of one path, from the folder `cwd`.
    pub fn resolve(path: &str, cwd: &str) -> String {
        let rooted = |path: &str| path.starts_with('/');
        if matches!(path, "" | ".") && rooted(cwd) {
            return cwd.to_owned();
        }
        let (full, absolute) = match (path.is_empty(), rooted(path)) {
            (true, _) => (format!("{cwd}/"), rooted(cwd)),
            (false, true) => (format!("{path}/"), true),
            (false, false) => (format!("{cwd}/{path}/"), rooted(cwd)),
        };
        let folded = super::fold(&full, !absolute, "/", |c| c == '/');
        match (absolute, folded.is_empty()) {
            (true, _) => format!("/{folded}"),
            (false, true) => ".".into(),
            (false, false) => folded,
        }
    }

    pub fn dirname(path: &str) -> &str {
        if path.is_empty() {
            return ".";
        }
        let rooted = path.starts_with('/');
        match super::folder_end(path.as_bytes(), 1, |b| b == b'/') {
            None if rooted => "/",
            None => ".",
            Some(1) if rooted => "//",
            Some(end) => &path[..end],
        }
    }

    pub fn extname(path: &str) -> &str {
        super::ext(path, 0, |b| b == b'/')
    }

    pub fn normalize(path: &str) -> String {
        if path.is_empty() {
            return ".".into();
        }
        let (absolute, trailing) = (path.starts_with('/'), path.ends_with('/'));
        let folded = super::fold(path, !absolute, "/", |c| c == '/');
        match (folded.is_empty(), absolute, trailing) {
            (true, true, _) => "/".into(),
            (true, false, true) => "./".into(),
            (true, false, false) => ".".into(),
            (false, _, _) => {
                let root = if absolute { "/" } else { "" };
                let tail = if trailing { "/" } else { "" };
                format!("{root}{folded}{tail}")
            }
        }
    }

    pub fn join(parts: &[&str]) -> String {
        let kept: Vec<&str> = parts.iter().copied().filter(|p| !p.is_empty()).collect();
        match kept.is_empty() {
            true => ".".into(),
            false => normalize(&kept.join("/")),
        }
    }

    pub fn basename(path: &str) -> &str {
        let path = path.trim_end_matches('/');
        path.rsplit('/').next().unwrap_or(path)
    }
}

pub mod win32 {
    const RESERVED: [&str; 28] = [
        "CON",
        "PRN",
        "AUX",
        "NUL",
        "COM1",
        "COM2",
        "COM3",
        "COM4",
        "COM5",
        "COM6",
        "COM7",
        "COM8",
        "COM9",
        "LPT1",
        "LPT2",
        "LPT3",
        "LPT4",
        "LPT5",
        "LPT6",
        "LPT7",
        "LPT8",
        "LPT9",
        "COM\u{b9}",
        "COM\u{b2}",
        "COM\u{b3}",
        "LPT\u{b9}",
        "LPT\u{b2}",
        "LPT\u{b3}",
    ];

    fn separator(c: char) -> bool {
        c == '/' || c == '\\'
    }

    fn sep(b: u8) -> bool {
        b == b'/' || b == b'\\'
    }

    /// Where the run of separators, or of other bytes, that starts at `from` ends.
    fn run(b: &[u8], from: usize, want: bool) -> usize {
        from + b[from..].iter().take_while(|c| sep(**c) == want).count()
    }

    /// The root `resolve` matches: the device, where the tail starts and whether it is absolute.
    fn root(path: &str) -> (String, usize, bool) {
        let (b, len) = (path.as_bytes(), path.len());
        if len >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
            let rooted = len > 2 && sep(b[2]);
            return (path[..2].to_owned(), if rooted { 3 } else { 2 }, rooted);
        }
        if !b.first().is_some_and(|c| sep(*c)) {
            return (String::new(), 0, false);
        }
        if len == 1 || !sep(b[1]) {
            return (String::new(), 1, true);
        }
        let host = run(b, 2, false);
        let share = run(b, host, true);
        if host == 2 || host == len || share == len {
            return (String::new(), 0, true);
        }
        let first = &path[2..host];
        match first == "." || first == "?" {
            true => (format!("\\\\{first}"), 4, true),
            false => {
                let end = run(b, share, false);
                (format!("\\\\{first}\\{}", &path[share..end]), end, true)
            }
        }
    }

    /// `path.win32.resolve` of one path, from the folder `cwd`; `env` reads a drive's own folder.
    pub fn resolve(path: &str, cwd: &str, env: impl Fn(&str) -> Option<String>) -> String {
        if matches!(path, "" | ".") && cwd.bytes().next().is_some_and(sep) {
            // Node on Windows leaves the slashes, which a Windows folder never holds.
            return cwd.replace('/', "\\");
        }
        let (mut device, end, mut absolute) = root(path);
        let mut tail = match path.is_empty() {
            true => String::new(),
            false => format!("{}\\", &path[end..]),
        };
        if !absolute || device.is_empty() {
            let mut from = match device.is_empty() {
                true => cwd.to_owned(),
                false => env(&format!("={device}"))
                    .filter(|folder| !folder.is_empty())
                    .unwrap_or_else(|| cwd.to_owned()),
            };
            let units: Vec<u16> = from.encode_utf16().take(3).collect();
            let drive = String::from_utf16_lossy(&units[..units.len().min(2)]);
            let other = drive.to_lowercase() != device.to_lowercase();
            if !device.is_empty() && other && units.get(2) == Some(&u16::from(b'\\')) {
                from = format!("{device}\\");
            }
            let (found, end, rooted) = root(&from);
            let same = device.is_empty() || found.is_empty();
            if same || found.to_lowercase() == device.to_lowercase() {
                if device.is_empty() {
                    device = found;
                }
                if !absolute {
                    tail = format!("{}\\{tail}", &from[end..]);
                    absolute = rooted;
                }
            }
        }
        let tail = super::fold(&tail, !absolute, "\\", separator);
        match (absolute, device.is_empty() && tail.is_empty()) {
            (true, _) => format!("{device}\\{tail}"),
            (false, true) => ".".into(),
            (false, false) => format!("{device}{tail}"),
        }
    }

    pub fn dirname(path: &str) -> &str {
        let (b, len) = (path.as_bytes(), path.len());
        match len {
            0 => return ".",
            1 if sep(b[0]) => return path,
            1 => return ".",
            _ => {}
        }
        let mut root_end = None;
        if sep(b[0]) {
            root_end = Some(1);
            let host = run(b, 2.min(len), false);
            if sep(b[1]) && host < len && host != 2 {
                let share = run(b, host, true);
                let end = run(b, share, false);
                if share < len && end == len {
                    return path;
                }
                if share < len {
                    root_end = Some(end + 1);
                }
            }
        } else if b[0].is_ascii_alphabetic() && b[1] == b':' {
            root_end = Some(if len > 2 && sep(b[2]) { 3 } else { 2 });
        }
        match (super::folder_end(b, root_end.unwrap_or(0), sep), root_end) {
            (Some(end), _) | (None, Some(end)) => &path[..end],
            (None, None) => ".",
        }
    }

    pub fn extname(path: &str) -> &str {
        let b = path.as_bytes();
        let drive = b.len() >= 2 && b[1] == b':' && b[0].is_ascii_alphabetic();
        super::ext(path, if drive { 2 } else { 0 }, sep)
    }

    /// `isWindowsReservedName`: with no colon, `slice(0, -1)` drops the last UTF-16 unit.
    fn reserved(path: &str, colon: Option<usize>) -> bool {
        let device = match (colon, path.chars().next_back()) {
            (Some(at), _) => &path[..at],
            (None, None) => "",
            (None, Some(last)) if last.len_utf16() == 1 => &path[..path.len() - last.len_utf8()],
            // Half a pair is left, which is no name.
            (None, Some(_)) => return false,
        };
        RESERVED.contains(&device.to_uppercase().as_str())
    }

    /// A UNC or device root after two separators: the device and where the root ends, or the result.
    fn double_root(path: &str, colon: Option<usize>) -> Result<Option<(String, usize)>, String> {
        let (b, len) = (path.as_bytes(), path.len());
        let run = |from: usize, want: bool| {
            from + b[from..].iter().take_while(|c| sep(**c) == want).count()
        };
        let host = run(2, false);
        if host == len || host == 2 {
            return Ok(None);
        }
        let first = &path[2..host];
        let share = run(host, true);
        if share == len {
            return Ok(None);
        }
        let end = run(share, false);
        if first == "." || first == "?" {
            let possible = colon.map_or("", |at| &path[4..=at]);
            let named = possible
                .strip_suffix(':')
                .is_some_and(|name| reserved(name, Some(name.len())));
            return Ok(Some(match named {
                true => (format!("\\\\?\\{possible}"), 4 + possible.len()),
                false => (format!("\\\\{first}"), 4),
            }));
        }
        if end == len {
            return Err(format!("\\\\{first}\\{}\\", &path[share..]));
        }
        Ok(Some((format!("\\\\{first}\\{}", &path[share..end]), end)))
    }

    pub fn normalize(path: &str) -> String {
        let (b, len) = (path.as_bytes(), path.len());
        match len {
            0 => return ".".into(),
            1 if b[0] == b'/' => return "\\".into(),
            1 => return path.into(),
            _ => {}
        }
        let colon = path.find(':');
        let (mut root_end, mut device, mut absolute) = (0, None, false);
        if sep(b[0]) {
            absolute = true;
            root_end = 1;
            if sep(b[1]) {
                root_end = 0;
                match double_root(path, colon) {
                    Ok(Some((name, end))) => (device, root_end) = (Some(name), end),
                    Ok(None) => {}
                    Err(whole) => return whole,
                }
            }
        } else if let Some(at) = colon.filter(|at| *at > 0) {
            if b[0].is_ascii_alphabetic() && at == 1 {
                absolute = len > 2 && sep(b[2]);
                (device, root_end) = (Some(path[..2].to_owned()), if absolute { 3 } else { 2 });
            } else if reserved(path, Some(at)) {
                (device, root_end) = (Some(path[..=at].to_owned()), at + 1);
            }
        }
        let mut tail = super::fold(&path[root_end..], !absolute, "\\", separator);
        if tail.is_empty() && !absolute {
            tail.push('.');
        }
        if !tail.is_empty() && sep(b[len - 1]) {
            tail.push('\\');
        }
        if !absolute && device.is_none() && colon.is_some() {
            // CVE-2024-36139: the tail must not read as a drive's path.
            let t = tail.as_bytes();
            let drive = t.len() >= 2 && t[0].is_ascii_alphabetic() && t[1] == b':';
            let mut colons = path.match_indices(':');
            if drive || colons.any(|(at, _)| at == len - 1 || sep(b[at + 1])) {
                return format!(".\\{tail}");
            }
        }
        let device = device.unwrap_or_default();
        if reserved(path, colon) {
            return format!(".\\{device}{tail}");
        }
        let root = if absolute { "\\" } else { "" };
        format!("{device}{root}{tail}")
    }

    pub fn join(parts: &[&str]) -> String {
        let kept: Vec<&str> = parts.iter().copied().filter(|p| !p.is_empty()).collect();
        let Some(first) = kept.first().map(|first| first.as_bytes()) else {
            return ".".into();
        };
        let mut joined = kept.join("\\");
        // Two slashes start a UNC path only when the first part has exactly two.
        let lead = first.iter().take(3).take_while(|c| sep(**c)).count();
        if !(lead == 2 && first.len() > 2) {
            let slashes = joined.bytes().take_while(|c| sep(*c)).count();
            if lead > 0 && slashes >= 2 {
                joined = format!("\\{}", &joined[slashes..]);
            }
        }
        let mut names = joined.split('\\').filter(|part| !part.is_empty());
        if names.any(|part| part.find(':').is_some_and(|at| reserved(part, Some(at)))) {
            return joined.replace('/', "\\");
        }
        normalize(&joined)
    }

    pub fn is_absolute(path: &str) -> bool {
        let b = path.as_bytes();
        b.first().is_some_and(|&c| sep(c))
            || (b.len() > 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && sep(b[2]))
    }

    pub fn basename(path: &str) -> &str {
        let b = path.as_bytes();
        let drive = b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':';
        let rest = path[if drive { 2 } else { 0 }..].trim_end_matches(separator);
        rest.rsplit(separator).next().unwrap_or(rest)
    }
}
