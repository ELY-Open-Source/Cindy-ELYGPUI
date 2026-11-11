//! mdast-util-to-hast's footnotes: the calls in the text and the section after it.

use super::build::{State, normalize_uri, wrap};
use super::{Element, Hast, Prop, text, words};

/// `clobberPrefix`.
const PREFIX: &str = "user-content-";

fn safe(id: &str) -> String {
    normalize_uri(&id.to_lowercase())
}

/// `footnoteReference`: the call, numbered by its footnote's first call.
pub fn reference(state: &mut State, identifier: &str) -> Hast {
    let id = identifier.to_uppercase();
    let place = state.order.iter().position(|known| *known == id);
    let counter = place.unwrap_or_else(|| {
        state.order.push(id.clone());
        state.order.len() - 1
    });
    let reuse = state.counts.entry(id.clone()).or_default();
    *reuse += 1;
    let again = match *reuse {
        1 => String::new(),
        count => format!("-{count}"),
    };
    let safe = safe(&id);
    let a = Element::new("a", vec![text((counter + 1).to_string())])
        .text("href", format!("#{PREFIX}fn-{safe}"))
        .text("id", format!("{PREFIX}fnref-{safe}{again}"))
        .with("dataFootnoteRef", Prop::Flag(true))
        .with("ariaDescribedBy", words(&["footnote-label"]));
    Hast::Element(Element::new("sup", vec![Hast::Element(a)]))
}

/// `footer`: the called footnotes in call order, each with its links back.
pub fn footer(state: &mut State) -> Option<Hast> {
    let mut items = Vec::new();
    let mut index = 0;
    // A footnote may call another, which joins the order while this runs.
    while let Some(id) = state.order.get(index).cloned() {
        index += 1;
        // micromark builds a call only for a footnote it has defined.
        let definition = *(state.footnotes.get(&id)).expect("a called footnote is defined");
        let mut content = state.all(definition);
        let safe = safe(&id);
        let mut backs = Vec::new();
        for again in 1..=state.counts.get(&id).copied().unwrap_or(0) {
            if again > 1 {
                backs.push(text(" "));
            }
            let (suffix, mut children) = (format!("-{again}"), vec![text("↩")]);
            let suffix = if again > 1 { suffix.as_str() } else { "" };
            if again > 1 {
                let sup = Element::new("sup", vec![text(again.to_string())]);
                children.push(Hast::Element(sup));
            }
            let back = Element::new("a", children)
                .text("href", format!("#{PREFIX}fnref-{safe}{suffix}"))
                .text("dataFootnoteBackref", "")
                .text("ariaLabel", format!("Back to reference {index}{suffix}"))
                .with("className", words(&["data-footnote-backref"]));
            backs.push(Hast::Element(back));
        }
        match content.last_mut() {
            Some(Hast::Element(tail)) if tail.tag == "p" => {
                match tail.children.last_mut() {
                    Some(Hast::Text(value)) => value.push(' '),
                    _ => tail.children.push(text(" ")),
                }
                tail.children.extend(backs);
            }
            _ => content.extend(backs),
        }
        let item = Element::new("li", wrap(content, true)).text("id", format!("{PREFIX}fn-{safe}"));
        items.push(Hast::Element(item));
    }
    if items.is_empty() {
        return None;
    }
    let label = Element::new("h2", vec![text("Footnotes")])
        .with("className", words(&["sr-only"]))
        .text("id", "footnote-label");
    let list = Element::new("ol", wrap(items, true));
    let section = Element::new(
        "section",
        vec![
            Hast::Element(label),
            text("\n"),
            Hast::Element(list),
            text("\n"),
        ],
    );
    let section = section
        .with("dataFootnotes", Prop::Flag(true))
        .with("className", words(&["footnotes"]));
    Some(Hast::Element(section))
}
