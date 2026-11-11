//! mdast-util-to-hast 13: each Markdown node as its elements.

use std::collections::HashMap;

use markdown::mdast::{
    AlignKind, Code, Definition, ImageReference, Link, LinkReference, List, ListItem, Node, Table,
};

use super::{Element, Hast, Prop, footer, text, words};
use crate::theme_import::color::js_space;

/// What a handler returns: nothing, one node, or a list, which no break trims.
pub enum One {
    None,
    Node(Hast),
    Many(Vec<Hast>),
}

/// `createState`: the definitions by identifier, and the footnotes as they are called.
pub struct State<'a> {
    definitions: HashMap<String, &'a Definition>,
    pub footnotes: HashMap<String, &'a Node>,
    pub counts: HashMap<String, u32>,
    pub order: Vec<String>,
}

/// `toHast`: the root's children, then the footnotes.
pub fn root(tree: &Node) -> Vec<Hast> {
    let mut state = State {
        definitions: HashMap::new(),
        footnotes: HashMap::new(),
        counts: HashMap::new(),
        order: Vec::new(),
    };
    state.define(tree);
    let mut nodes = wrap(state.all(tree), false);
    if let Some(section) = footer::footer(&mut state) {
        nodes.push(text("\n"));
        nodes.push(section);
    }
    nodes
}

/// `wrap`: line feeds between nodes, and around them when `loose`.
pub fn wrap(nodes: Vec<Hast>, loose: bool) -> Vec<Hast> {
    let mut out = Vec::new();
    if loose {
        out.push(text("\n"));
    }
    let filled = !nodes.is_empty();
    for (index, node) in nodes.into_iter().enumerate() {
        if index > 0 {
            out.push(text("\n"));
        }
        out.push(node);
    }
    if loose && filled {
        out.push(text("\n"));
    }
    out
}

/// micromark-util-sanitize-uri's `normalizeUri`.
pub fn normalize_uri(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = String::with_capacity(value.len());
    let mut skip = 0;
    for (index, ch) in value.char_indices() {
        if skip > 0 {
            skip -= 1;
            out.push(ch);
            continue;
        }
        let escape = bytes.get(index + 1..index + 3);
        if ch == '%' && escape.is_some_and(|pair| pair.iter().all(u8::is_ascii_alphanumeric)) {
            skip = 2;
            out.push(ch);
        } else if matches!(ch, '!' | '#' | '$' | '&'..=';' | '=' | '?'..='Z' | '_' | 'a'..='z' | '~')
        {
            out.push(ch);
        } else {
            let mut units = [0; 4];
            for byte in ch.encode_utf8(&mut units).bytes() {
                out.push_str(&format!("%{byte:02X}"));
            }
        }
    }
    out
}

fn blank(ch: char) -> bool {
    matches!(ch, ' ' | '\t')
}

/// trim-lines: spaces and tabs around each line ending.
fn trim_lines(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    let mut first = true;
    loop {
        let line = if first {
            rest
        } else {
            rest.trim_start_matches(blank)
        };
        let Some(end) = line.find(['\n', '\r']) else {
            out.push_str(line);
            return out;
        };
        out.push_str(line[..end].trim_end_matches(blank));
        out.push_str(&line[end..=end]);
        rest = &line[end + 1..];
        first = false;
    }
}

fn element(tag: &'static str, children: Vec<Hast>) -> One {
    One::Node(Hast::Element(Element::new(tag, children)))
}

/// A link's or an image's title, where it has one.
fn titled(element: Element, title: Option<&String>) -> Element {
    match title {
        Some(title) => element.text("title", title),
        None => element,
    }
}

impl<'a> State<'a> {
    fn define(&mut self, node: &'a Node) {
        match node {
            Node::Definition(definition) => {
                let id = definition.identifier.to_uppercase();
                self.definitions.entry(id).or_insert(definition);
            }
            Node::FootnoteDefinition(definition) => {
                let id = definition.identifier.to_uppercase();
                self.footnotes.entry(id).or_insert(node);
            }
            _ => {}
        }
        for child in node.children().into_iter().flatten() {
            self.define(child);
        }
    }

    /// `all`: the children's nodes; what follows a break loses its leading spaces.
    pub fn all(&mut self, parent: &'a Node) -> Vec<Hast> {
        let mut out = Vec::new();
        let mut broke = false;
        for child in parent.children().into_iter().flatten() {
            match self.one(child) {
                One::None => {}
                One::Many(nodes) => out.extend(nodes),
                One::Node(mut node) => {
                    let head = match &mut node {
                        Hast::Element(element) => element.children.first_mut(),
                        other => Some(other),
                    };
                    if broke && let Some(Hast::Text(value)) = head {
                        *value = value.trim_start_matches(blank).to_owned();
                    }
                    out.push(node);
                }
            }
            broke = matches!(child, Node::Break(_));
        }
        out
    }

    fn one(&mut self, node: &'a Node) -> One {
        match node {
            Node::Paragraph(_) => element("p", self.all(node)),
            Node::Heading(heading) => element(
                super::HEADINGS[usize::from(heading.depth) - 1],
                self.all(node),
            ),
            Node::Blockquote(_) => element("blockquote", wrap(self.all(node), true)),
            Node::Emphasis(_) => element("em", self.all(node)),
            Node::Strong(_) => element("strong", self.all(node)),
            Node::Delete(_) => element("del", self.all(node)),
            Node::ThematicBreak(_) => element("hr", Vec::new()),
            Node::Break(_) => One::Many(vec![
                Hast::Element(Element::new("br", Vec::new())),
                text("\n"),
            ]),
            Node::Text(node) => One::Node(text(trim_lines(&node.value))),
            Node::InlineCode(code) => {
                let value = code.value.replace("\r\n", " ").replace(['\n', '\r'], " ");
                element("code", vec![text(value)])
            }
            Node::Code(code) => One::Node(fenced(code)),
            Node::Math(math) => One::Node(pre(math_code("math-display", &math.value))),
            Node::InlineMath(math) => {
                One::Node(Hast::Element(math_code("math-inline", &math.value)))
            }
            Node::Html(_) => One::Node(Hast::Raw),
            Node::Link(link) => One::Node(self.link(link, node)),
            Node::Image(image) => {
                let img = Element::new("img", Vec::new())
                    .text("src", normalize_uri(&image.url))
                    .text("alt", &image.alt);
                let mut img = titled(img, image.title.as_ref());
                let more = [
                    ("width", &image.width),
                    ("height", &image.height),
                    ("data-cindy-raw-local-image-src", &image.raw),
                ];
                for (name, value) in more {
                    if let Some(value) = value {
                        img = img.text(name, value);
                    }
                }
                One::Node(Hast::Element(img))
            }
            Node::List(list) => One::Node(self.list(list)),
            Node::Table(table) => One::Node(self.table(table)),
            Node::LinkReference(reference) => self.link_reference(reference, node),
            Node::ImageReference(reference) => self.image_reference(reference),
            Node::FootnoteReference(reference) => {
                One::Node(footer::reference(self, &reference.identifier))
            }
            Node::Definition(_) | Node::FootnoteDefinition(_) => One::None,
            other => panic!("a node this parse does not build: {other:?}"),
        }
    }

    fn link(&mut self, link: &'a Link, node: &'a Node) -> Hast {
        let a = Element::new("a", self.all(node)).text("href", normalize_uri(&link.url));
        let mut a = titled(a, link.title.as_ref());
        if link.bare_path {
            a = a.text("data-bare-path", "");
        }
        if let Some(raw) = &link.raw {
            a = a.text("data-cindy-raw-local-link-href", raw);
        }
        Hast::Element(a)
    }

    fn list(&mut self, list: &'a List) -> Hast {
        let loose = list.spread || list.children.iter().any(spread);
        let mut items = Vec::new();
        for child in &list.children {
            let Node::ListItem(item) = child else {
                panic!("a list holds items alone: {child:?}");
            };
            items.push(Hast::Element(self.item(item, child, loose)));
        }
        let tasks = items
            .iter()
            .any(|item| matches!(item, Hast::Element(li) if li.classed("task-list-item")));
        let mut out = Element::new(if list.ordered { "ol" } else { "ul" }, Vec::new());
        if let Some(start) = list.start.filter(|start| *start != 1) {
            out = out.with("start", Prop::Number(start));
        }
        if tasks {
            out = out.with("className", words(&["contains-task-list"]));
        }
        out.children = wrap(items, true);
        Hast::Element(out)
    }

    fn item(&mut self, item: &'a ListItem, node: &'a Node, loose: bool) -> Element {
        let mut results = self.all(node);
        let mut li = Element::new("li", Vec::new());
        let paragraph = |node: &Hast| matches!(node, Hast::Element(p) if p.tag == "p");
        if let Some(checked) = item.checked {
            // micromark reads a check only at the head of an item's first paragraph, before text.
            let Some(Hast::Element(paragraph)) = results.first_mut().filter(|head| paragraph(head))
            else {
                panic!("a task item starts with its paragraph");
            };
            paragraph.children.insert(0, text(" "));
            let input = Element::new("input", Vec::new())
                .text("type", "checkbox")
                .with("checked", Prop::Flag(checked))
                .with("disabled", Prop::Flag(true));
            paragraph.children.insert(0, Hast::Element(input));
            li = li.with("className", words(&["task-list-item"]));
        }
        let closed = results.last().is_some_and(|tail| loose || !paragraph(tail));
        for (index, child) in results.into_iter().enumerate() {
            let tight = !loose && paragraph(&child);
            if !(tight && index == 0) {
                li.children.push(text("\n"));
            }
            match child {
                Hast::Element(p) if tight => li.children.extend(p.children),
                child => li.children.push(child),
            }
        }
        if closed {
            li.children.push(text("\n"));
        }
        li
    }

    fn table(&mut self, table: &'a Table) -> Hast {
        let mut rows = Vec::new();
        for (index, row) in table.children.iter().enumerate() {
            let mut cells = Vec::new();
            let held = row.children().into_iter().flatten();
            let mut held = held.map(Some).chain(std::iter::repeat(None));
            for align in &table.align {
                let mut cell = Element::new(if index == 0 { "th" } else { "td" }, Vec::new());
                let side = match align {
                    AlignKind::Left => Some("left"),
                    AlignKind::Right => Some("right"),
                    AlignKind::Center => Some("center"),
                    AlignKind::None => None,
                };
                if let Some(side) = side {
                    cell = cell.text("align", side);
                }
                if let Some(Some(node)) = held.next() {
                    cell.children = self.all(node);
                }
                cells.push(Hast::Element(cell));
            }
            rows.push(Hast::Element(Element::new("tr", wrap(cells, true))));
        }
        let body = rows.split_off(1);
        let mut content = vec![Hast::Element(Element::new("thead", wrap(rows, true)))];
        if !body.is_empty() {
            content.push(Hast::Element(Element::new("tbody", wrap(body, true))));
        }
        Hast::Element(Element::new("table", wrap(content, true)))
    }

    fn link_reference(&mut self, reference: &'a LinkReference, node: &'a Node) -> One {
        let definition = self.defined(&reference.identifier);
        let a = Element::new("a", self.all(node)).text("href", normalize_uri(&definition.url));
        One::Node(Hast::Element(titled(a, definition.title.as_ref())))
    }

    fn image_reference(&mut self, reference: &'a ImageReference) -> One {
        let definition = self.defined(&reference.identifier);
        let img = Element::new("img", Vec::new())
            .text("src", normalize_uri(&definition.url))
            .text("alt", &reference.alt);
        One::Node(Hast::Element(titled(img, definition.title.as_ref())))
    }

    /// A reference's definition: micromark builds a reference only for a label it has defined.
    fn defined(&self, identifier: &str) -> &'a Definition {
        let found = self.definitions.get(&identifier.to_uppercase());
        found.unwrap_or_else(|| panic!("the reference {identifier:?} has no definition"))
    }
}

/// `listItemLoose`.
fn spread(item: &Node) -> bool {
    matches!(item, Node::ListItem(item) if item.spread)
}

fn pre(code: Element) -> Hast {
    Hast::Element(Element::new("pre", vec![Hast::Element(code)]))
}

/// `code`: the block's text ends in a line feed; its class is the info's first word.
fn fenced(code: &Code) -> Hast {
    let value = match code.value.as_str() {
        "" => String::new(),
        value => format!("{value}\n"),
    };
    let mut out = Element::new("code", vec![text(value)]);
    if let Some(lang) = code.lang.as_deref().filter(|lang| !lang.is_empty()) {
        let word = lang.split(js_space).next().unwrap_or_default();
        out = out.with("className", words(&[&format!("language-{word}")]));
    }
    pre(out)
}

/// mdast-util-math's `hProperties` and `hChildren`.
fn math_code(class: &'static str, value: &str) -> Element {
    Element::new("code", vec![text(value)]).with("className", words(&["language-math", class]))
}
