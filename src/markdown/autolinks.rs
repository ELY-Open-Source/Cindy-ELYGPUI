//! `transformGfmAutolinkLiterals`: the web addresses and emails the tokenizer left in text.

use std::sync::LazyLock;

use markdown::mdast::{Link, Node, Text};
use regex::Regex;

use super::flanks::{punctuation, space};

/// A match of a finder's pattern; `nodes` is none where its replacer refuses the match.
struct Found {
    start: usize,
    end: usize,
    nodes: Option<Vec<Node>>,
}

/// The next match at or after a byte offset.
type Finder = fn(&str, usize) -> Option<Found>;

pub fn transform(root: &mut Node) {
    walk(root, url);
    walk(root, email);
}

/// `findAndReplace` with `ignore: ['link', 'linkReference']`.
fn walk(node: &mut Node, find: Finder) {
    if matches!(node, Node::Link(_) | Node::LinkReference(_)) {
        return;
    }
    let Some(children) = node.children_mut() else {
        return;
    };
    let mut index = 0;
    while index < children.len() {
        let replaced = match &children[index] {
            Node::Text(text) => replace(&text.value, find),
            _ => None,
        };
        match replaced {
            Some(nodes) => {
                let count = nodes.len();
                children.splice(index..=index, nodes);
                index += count;
            }
            None => {
                walk(&mut children[index], find);
                index += 1;
            }
        }
    }
}

fn text(value: &str) -> Node {
    Node::Text(Text {
        value: value.into(),
        position: None,
    })
}

fn link(url: String, label: &str) -> Node {
    Node::Link(Link {
        children: vec![text(label)],
        position: None,
        url,
        title: None,
        bare_path: false,
        raw: None,
    })
}

/// The handler's loop over one text: a refused match starts the search one unit on.
fn replace(value: &str, find: Finder) -> Option<Vec<Node>> {
    let mut nodes = Vec::new();
    let (mut from, mut start) = (0, 0);
    while let Some(found) = find(value, from) {
        let Some(new) = found.nodes else {
            from = found.start + 1;
            continue;
        };
        if start != found.start {
            nodes.push(text(&value[start..found.start]));
        }
        nodes.extend(new);
        start = found.end;
        from = found.end;
    }
    if start == 0 {
        return None;
    }
    if start < value.len() {
        nodes.push(text(&value[start..]));
    }
    Some(nodes)
}

/// `previous`: at the text's start or after whitespace or punctuation; an email after no slash.
fn previous(value: &str, at: usize, email: bool) -> bool {
    match value[..at].chars().next_back() {
        None => true,
        Some(ch) => ch.len_utf16() == 1 && (space(ch) || punctuation(ch)) && !(email && ch == '/'),
    }
}

// `/(https?:\/\/|www(?=\.))([-.\w]+)([^ \t\r\n]*)/gi`; `/i` folds no letter past ASCII here.
static URL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?:([Hh][Tt][Tt][Pp][Ss]?://)([-.0-9A-Za-z_]+)|([Ww][Ww][Ww]\.[-.0-9A-Za-z_]*))([^ \t\r\n]*)",
    )
    .expect("valid pattern")
});

fn url(value: &str, from: usize) -> Option<Found> {
    if from > value.len() {
        return None;
    }
    let found = URL.captures_at(value, from)?;
    let whole = found.get(0).expect("the match");
    let part = |n: usize| found.get(n).map_or("", |m| m.as_str());
    Some(Found {
        start: whole.start(),
        end: whole.end(),
        nodes: find_url(value, whole.start(), (part(1), part(2), part(3), part(4))),
    })
}

/// `findUrl`.
fn find_url(value: &str, at: usize, parts: (&str, &str, &str, &str)) -> Option<Vec<Node>> {
    let (protocol, domain, www, path) = parts;
    let (prefix, protocol, domain) = if www.is_empty() {
        ("", protocol, domain)
    } else {
        ("http://", "", www)
    };
    if !previous(value, at, false) || !correct_domain(domain) {
        return None;
    }
    let (url, trail) = split_url(&format!("{domain}{path}"));
    if url.is_empty() {
        return None;
    }
    let shown = format!("{protocol}{url}");
    let mut nodes = vec![link(format!("{prefix}{shown}"), &shown)];
    if !trail.is_empty() {
        nodes.push(text(&trail));
    }
    Some(nodes)
}

/// `isCorrectDomain`: two parts at least, the last two with a letter or digit and no underscore.
fn correct_domain(domain: &str) -> bool {
    let parts: Vec<&str> = domain.split('.').collect();
    let good = |part: &str| {
        part.is_empty() || !part.contains('_') && part.bytes().any(|b| b.is_ascii_alphanumeric())
    };
    parts.len() >= 2 && parts[parts.len() - 2..].iter().all(|part| good(part))
}

/// `splitUrl`: trailing punctuation leaves the address, less the parentheses that close its own.
fn split_url(url: &str) -> (String, String) {
    let kept = url.trim_end_matches(|ch: char| "!\"&'),.:;<>?]}".contains(ch));
    let mut trail = &url[kept.len()..];
    let mut url = kept.to_owned();
    let opening = url.matches('(').count();
    let mut closing = url.matches(')').count();
    while let Some(at) = trail.find(')').filter(|_| opening > closing) {
        url.push_str(&trail[..=at]);
        trail = &trail[at + 1..];
        closing += 1;
    }
    (url, trail.to_owned())
}

fn atext(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'+')
}

fn label(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')
}

fn run(bytes: &[u8], from: usize, class: fn(u8) -> bool) -> usize {
    from + bytes[from..].iter().take_while(|b| class(**b)).count()
}

/// `/(?<=^|\s|\p{P}|\p{S})([-.\w+]+)@([-\w]+(?:\.[-\w]+)+)/gu`.
fn email(value: &str, from: usize) -> Option<Found> {
    let bytes = value.as_bytes();
    let mut at = from;
    while at < bytes.len() {
        let Some(ch) = value.get(at..).and_then(|rest| rest.chars().next()) else {
            at += 1;
            continue;
        };
        let behind = value[..at].chars().next_back();
        if !behind.is_none_or(|ch| space(ch) || punctuation(ch)) {
            at += ch.len_utf8();
            continue;
        }
        let name = run(bytes, at, atext);
        if name == at || bytes.get(name) != Some(&b'@') {
            // No later start in this run ends it otherwise.
            at = name.max(at + ch.len_utf8());
            continue;
        }
        let mut end = run(bytes, name + 1, label);
        let mut dotted = false;
        while end > name + 1 && bytes.get(end) == Some(&b'.') {
            let next = run(bytes, end + 1, label);
            if next == end + 1 {
                break;
            }
            end = next;
            dotted = true;
        }
        if !dotted {
            at = name;
            continue;
        }
        return Some(Found {
            start: at,
            end,
            nodes: find_email(value, at, (name, end)),
        });
    }
    None
}

/// `findEmail`.
fn find_email(value: &str, at: usize, (name, end): (usize, usize)) -> Option<Vec<Node>> {
    let address = &value[at..end];
    let last = value.as_bytes()[end - 1];
    if !previous(value, at, true) || last.is_ascii_digit() || matches!(last, b'-' | b'_') {
        return None;
    }
    debug_assert_eq!(value.as_bytes()[name], b'@');
    Some(vec![link(format!("mailto:{address}"), address)])
}
