//! What react-markdown hands `MarkdownRenderer.tsx`'s renderers: the element tree of a source.

mod build;
mod footer;
mod slug;
#[cfg(test)]
mod tests;
mod urls;

pub use urls::Urls;

/// A node of the tree. `Raw` is html, which `skipHtml` drops before a renderer sees it.
#[derive(Debug, Clone, PartialEq)]
pub enum Hast {
    Text(String),
    Element(Element),
    Raw,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    pub tag: &'static str,
    pub props: Vec<(&'static str, Prop)>,
    pub children: Vec<Hast>,
}

/// A property's value, as hast holds it.
#[derive(Debug, Clone, PartialEq)]
pub enum Prop {
    Text(String),
    Flag(bool),
    Number(u32),
    Words(Vec<String>),
}

impl Element {
    fn new(tag: &'static str, children: Vec<Hast>) -> Self {
        Self {
            tag,
            props: Vec::new(),
            children,
        }
    }

    fn with(mut self, name: &'static str, value: Prop) -> Self {
        self.props.push((name, value));
        self
    }

    fn text(self, name: &'static str, value: impl Into<String>) -> Self {
        self.with(name, Prop::Text(value.into()))
    }

    pub fn prop(&self, name: &str) -> Option<&Prop> {
        let found = self.props.iter().find(|(key, _)| *key == name);
        found.map(|(_, value)| value)
    }

    pub fn classed(&self, class: &str) -> bool {
        matches!(self.prop("className"), Some(Prop::Words(words)) if words.iter().any(|word| word == class))
    }
}

fn words(list: &[&str]) -> Prop {
    Prop::Words(list.iter().map(|word| (*word).to_owned()).collect())
}

fn text(value: impl Into<String>) -> Hast {
    Hast::Text(value.into())
}

/// The tree a renderer gets for `text`; `privileged` adds the session links.
pub fn render(text: &str, privileged: bool, urls: Urls) -> Vec<Hast> {
    let mut nodes = build::root(&super::transformed(text, privileged));
    finish(&mut nodes, &mut slug::Slugs::default(), urls);
    nodes
}

/// A node's text, as hast-util-to-string reads it.
pub fn string(node: &Hast, out: &mut String) {
    match node {
        Hast::Text(value) => out.push_str(value),
        Hast::Element(element) => element.children.iter().for_each(|child| string(child, out)),
        Hast::Raw => {}
    }
}

const HEADINGS: [&str; 6] = ["h1", "h2", "h3", "h4", "h5", "h6"];

/// The rehype plugins and react-markdown's `post`, in one walk in document order.
fn finish(nodes: &mut Vec<Hast>, slugs: &mut slug::Slugs, urls: Urls) {
    nodes.retain(|node| *node != Hast::Raw);
    for node in nodes {
        let Hast::Element(element) = node else {
            continue;
        };
        if HEADINGS.contains(&element.tag) && element.prop("id").is_none() {
            let mut heading = String::new();
            (element.children.iter()).for_each(|child| string(child, &mut heading));
            element.props.push(("id", Prop::Text(slugs.slug(&heading))));
        }
        if element.tag == "pre" {
            for child in &mut element.children {
                if let Hast::Element(code) = child {
                    code.props
                        .push(("dataFencedCode", Prop::Text(String::new())));
                }
            }
        }
        let address = match element.tag {
            "a" => "href",
            "img" => "src",
            _ => "",
        };
        for (name, value) in &mut element.props {
            if *name == address
                && let Prop::Text(url) = value
            {
                *url = urls.transform(url, name);
            }
        }
        finish(&mut element.children, slugs, urls);
    }
}
