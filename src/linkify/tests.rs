use super::{Link, find};
use crate::{
    deep_link::tests::{golden, object, text},
    rich_answer::js::{Js, utf16_len},
};

#[test]
fn links_are_found_as_cindys() {
    golden("findLinkifyMatches", |args| {
        let source = args[0].str().unwrap();
        let shown = |found: &super::Match| {
            let (kind, target) = match &found.link {
                Link::Url => ("url", None),
                Link::Img => ("img", None),
                Link::Session(target) => ("session", Some(target)),
                Link::Project(target) => ("project", Some(target)),
            };
            let matched = &source[found.range.clone()];
            let mut fields = vec![
                ("kind", text(kind)),
                (
                    "index",
                    Js::Num(utf16_len(&source[..found.range.start]) as f64),
                ),
                ("length", Js::Num(utf16_len(matched) as f64)),
                ("text", text(matched)),
            ];
            if let Some(target) = target {
                fields.push(("href", text(&target.href)));
                fields.extend(target.label.as_deref().map(|label| ("label", text(label))));
            }
            object(fields)
        };
        Js::Arr(find(source).iter().map(shown).collect())
    });
}
