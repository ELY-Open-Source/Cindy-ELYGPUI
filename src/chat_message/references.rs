//! `readAgentInputReferences`: a user row's inline references, in UTF-16 offsets.

use super::one_line;
use crate::{
    cindy_json::{decode_uri_component, js_to_number},
    rich_answer::js::{Js, Obj, utf16_len},
};

const SCHEMES: [&str; 2] = ["cindy", "xdt-maker"];

fn safe_integer(value: Option<&Js>) -> Option<f64> {
    value?
        .integer()
        .filter(|n| n.abs() <= 9_007_199_254_740_991.0)
}

fn non_empty(value: Option<&Js>) -> Option<&str> {
    value?.str().filter(|s| !s.is_empty())
}

fn units_slice(text: &str, start: usize, end: usize) -> String {
    let units: Vec<u16> = text.encode_utf16().skip(start).take(end - start).collect();
    String::from_utf16_lossy(&units)
}

fn control(text: &str) -> bool {
    text.chars()
        .any(|c| matches!(c, '\0'..='\u{1f}' | '\u{7f}' | '\u{2028}' | '\u{2029}'))
}

/// `stripDeepLinkPrefix`.
fn strip<'a>(href: &'a str, route: &str) -> Option<&'a str> {
    SCHEMES
        .iter()
        .find_map(|scheme| href.strip_prefix(&format!("{scheme}://{route}")))
}

/// `decodeBoundedComponent`; a missing part reads as the text `undefined`, as JavaScript coerces it.
fn decode(value: Option<&str>, max: usize) -> Option<String> {
    let decoded = decode_uri_component(value.unwrap_or("undefined"))?;
    (!decoded.is_empty() && utf16_len(&decoded) <= max).then_some(decoded)
}

/// `queryValue`: the first pair named `key`.
fn query_value(query: &str, key: &str, max: usize) -> Option<String> {
    for pair in query.split('&') {
        match pair.find('=') {
            Some(at) if at > 0 && &pair[..at] == key => return decode(Some(&pair[at + 1..]), max),
            _ => {}
        }
    }
    None
}

fn before_hash(rest: &str) -> &str {
    rest.find('#').map_or(rest, |at| &rest[..at])
}

/// `parseSessionHref`: the session and its `message` query.
fn session_href(href: &str) -> Option<(String, Option<String>)> {
    let rest = before_hash(strip(href, "session/")?);
    let query = rest.find('?');
    let raw = query.map_or(rest, |at| &rest[..at]).trim_end_matches('/');
    if raw.is_empty() {
        return None;
    }
    let session = decode_uri_component(raw).filter(|s| !s.is_empty())?;
    let mut message = None;
    if let Some(at) = query {
        for pair in rest[at + 1..].split('&') {
            let Some(eq) = pair
                .find('=')
                .filter(|eq| *eq > 0 && &pair[..*eq] == "message")
            else {
                continue;
            };
            let raw = &pair[eq + 1..];
            if !raw.is_empty() {
                message = decode_uri_component(raw).filter(|m| !m.is_empty());
            }
            break;
        }
    }
    Some((session, message))
}

fn project_href(href: &str) -> Option<String> {
    let rest = before_hash(strip(href, "project/")?);
    let raw = rest
        .find('?')
        .map_or(rest, |at| &rest[..at])
        .trim_end_matches('/');
    if raw.is_empty() {
        return None;
    }
    decode_uri_component(raw).filter(|d| !d.is_empty())
}

fn browser_tab_href(href: &str) -> Option<(String, String)> {
    let rest = strip(href, "browser-tab/").filter(|r| utf16_len(r) <= 5_000)?;
    let rest = before_hash(rest);
    let at = rest.find('?').filter(|at| *at > 0)?;
    let tab = decode(Some(&rest[..at]), 256)?;
    let url = query_value(&rest[at + 1..], "url", 4_096)?;
    let scheme = url::Url::parse(&url).ok()?.scheme().to_owned();
    matches!(scheme.as_str(), "http" | "https").then_some((tab, url))
}

fn desktop_window_href(href: &str) -> Option<(f64, f64, String)> {
    let rest = strip(href, "desktop-window/").filter(|r| utf16_len(r) <= 1_000)?;
    let rest = before_hash(rest);
    let at = rest.find('?').filter(|at| *at > 0)?;
    let parts: Vec<&str> = rest[..at].split('/').collect();
    if parts.len() > 2 {
        return None;
    }
    let number = |i: usize| parts.get(i).map_or(f64::NAN, |p| js_to_number(p));
    let (pid, window) = (number(0), number(1));
    let app = query_value(&rest[at + 1..], "app", 200)?;
    let safe = |n: f64| n.is_finite() && n.fract() == 0.0 && n.abs() <= 9_007_199_254_740_991.0;
    (safe(pid) && pid > 0.0 && safe(window) && window >= 0.0).then_some((pid, window, app))
}

fn plugin_resource_href(href: &str) -> Option<[String; 3]> {
    let rest = strip(href, "plugin-resource/")
        .filter(|r| utf16_len(r) <= 1_500 && !r.contains('?') && !r.contains('#'))?;
    let parts: Vec<&str> = rest.split('/').collect();
    if parts.len() > 3 {
        return None;
    }
    let ghost = decode(parts.first().copied(), 32)?;
    let tool = decode(parts.get(1).copied(), 64)?;
    let resource = decode(parts.get(2).copied(), 256)?;
    let ghost_ok = ghost.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        && ghost
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    let tool_ok = tool.starts_with(|c: char| c.is_ascii_lowercase())
        && tool
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
    (ghost_ok && tool_ok && !control(&resource)).then_some([ghost, tool, resource])
}

fn bot_href(href: &str) -> Option<String> {
    let rest = strip(href, "bot/")?;
    if utf16_len(rest) > 256 || rest.contains(['/', '?', '#']) {
        return None;
    }
    decode(Some(rest), 128).filter(|id| !id.contains('/') && !control(id))
}

fn todo_href(href: &str) -> Option<(String, String)> {
    let rest = strip(href, "bot-todo/")
        .filter(|r| utf16_len(r) <= 512 && !r.contains('?') && !r.contains('#'))?;
    let parts: Vec<&str> = rest.split('/').collect();
    if parts.len() > 2 || parts.iter().any(|p| p.is_empty()) || parts.len() < 2 {
        return None;
    }
    let valid = |id: String| (!id.contains('/') && !control(&id)).then_some(id);
    Some((
        valid(decode(Some(parts[0]), 128)?)?,
        valid(decode(Some(parts[1]), 128)?)?,
    ))
}

fn host_snapshot(value: Option<&Js>) -> Option<Js> {
    let c = value?.obj()?;
    let availability = c.get("availability").and_then(Js::str).unwrap_or("");
    let activity = c.get("activity").and_then(Js::str).unwrap_or("");
    let active =
        safe_integer(c.get("activeDelegations")).filter(|n| (0.0..=10_000.0).contains(n))?;
    let valid = ["ready", "degraded", "failed", "unverified", "unavailable"]
        .contains(&availability)
        && ["working", "idle", "unknown"].contains(&activity);
    if !valid {
        return None;
    }
    let mut out = Obj::new();
    out.insert("availability".into(), Js::Str(availability.into()));
    out.insert("activity".into(), Js::Str(activity.into()));
    out.insert("activeDelegations".into(), Js::Num(active));
    if let Some(Js::Str(reason)) = c.get("reason") {
        let line = one_line(reason);
        let reason = units_slice(&line, 0, utf16_len(&line).min(240));
        if !reason.is_empty() {
            out.insert("reason".into(), Js::Str(reason));
        }
    }
    Some(Js::Obj(out))
}

fn short(value: Option<&Js>, max: usize) -> Option<&str> {
    non_empty(value).filter(|s| utf16_len(s) <= max)
}

/// `readReference`.
fn read(value: &Js, source: &str) -> Option<(f64, f64, Obj)> {
    let c = value.obj()?;
    let (start, end) = (safe_integer(c.get("start"))?, safe_integer(c.get("end"))?);
    let href = non_empty(c.get("href"))?;
    if start < 0.0 || end <= start || end > utf16_len(source) as f64 {
        return None;
    }
    let span = units_slice(source, start as usize, end as usize);
    if span != href && !(span.starts_with('[') && span.ends_with(&format!("]({href})"))) {
        return None;
    }
    let kind = c.get("kind").and_then(Js::str);
    let mut out = Obj::new();
    let mut put = |key: &str, value: Js| {
        out.insert(key.into(), value);
    };
    let s = |text: &str| Js::Str(text.into());
    put("kind", s(kind.unwrap_or("")));
    put("start", Js::Num(start));
    put("end", Js::Num(end));
    put("href", s(href));
    match kind {
        Some("message") => {
            let (session, message) = session_href(href)?;
            put("sessionId", Js::Str(session));
            put("messageClientId", Js::Str(message?));
            if let Some(text @ Js::Str(_)) = c.get("text") {
                put("text", text.clone());
            }
            if c.get("truncated") == Some(&Js::Bool(true)) {
                put("truncated", Js::Bool(true));
            }
        }
        Some("session") => {
            let (session, None) = session_href(href)? else {
                return None;
            };
            put("sessionId", Js::Str(session));
            if let Some(title) = non_empty(c.get("title")) {
                put("title", s(title));
            }
        }
        Some("project") if non_empty(c.get("name")).is_some() => {
            let dir = project_href(href)?;
            put("name", c["name"].clone());
            put("workingDir", Js::Str(dir));
        }
        Some("browser-tab") => {
            let (tab, url) = browser_tab_href(href)?;
            put("tabId", Js::Str(tab));
            put("url", Js::Str(url));
            if let Some(title) = non_empty(c.get("title")) {
                put("title", s(title));
            }
        }
        Some("desktop-window") => {
            let (pid, window, app) = desktop_window_href(href)?;
            put("windowId", Js::Num(window));
            put("pid", Js::Num(pid));
            put("appName", Js::Str(app));
            if let Some(title) = non_empty(c.get("title")) {
                put("title", s(title));
            }
        }
        Some("plugin-resource")
            if short(c.get("pluginName"), 128).is_some()
                && short(c.get("label"), 128).is_some() =>
        {
            let [ghost, tool, resource] = plugin_resource_href(href)?;
            put("ghostId", Js::Str(ghost));
            put("tool", Js::Str(tool));
            put("resourceId", Js::Str(resource));
            put("pluginName", Js::Str(one_line(c["pluginName"].str()?)));
            put("label", Js::Str(one_line(c["label"].str()?)));
            if let Some(description) = short(c.get("description"), 256) {
                put("description", Js::Str(one_line(description)));
            }
        }
        Some("bot") if short(c.get("name"), 128).is_some() => {
            let bot = bot_href(href)?;
            put("botId", Js::Str(bot));
            put("name", Js::Str(one_line(c["name"].str()?)));
            if let Some(snapshot) = host_snapshot(c.get("hostSnapshot")) {
                put("hostSnapshot", snapshot);
            }
        }
        Some("todo") => {
            let title = short(c.get("title"), 300)?;
            let (bot, todo) = todo_href(href)?;
            put("botId", Js::Str(bot));
            put("todoId", Js::Str(todo));
            put("title", Js::Str(one_line(title)));
        }
        _ => return None,
    }
    Some((start, end, out))
}

/// `readAgentInputReferences`: valid references by position, overlaps dropped.
pub fn read_all(value: Option<&Js>, source: &str) -> Vec<Js> {
    let Some(Js::Arr(items)) = value else {
        return Vec::new();
    };
    let mut references: Vec<(f64, f64, Obj)> =
        items.iter().filter_map(|v| read(v, source)).collect();
    references.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let mut previous = 0.0;
    let mut out = Vec::new();
    for (start, end, reference) in references {
        if start < previous {
            continue;
        }
        previous = end;
        out.push(Js::Obj(reference));
    }
    out
}
