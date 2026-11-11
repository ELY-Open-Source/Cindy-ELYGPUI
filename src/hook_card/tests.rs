use super::{Card, Icon, card};
use crate::{
    i18n::Translator,
    rich_answer::js::{self, Js, Obj},
};

/// A card as the golden reads Cindy's off the DOM.
fn seen(card: Card) -> Js {
    let text = |text: &str| Js::Str(text.to_owned());
    let optional = |value: Option<String>| value.map_or(Js::Null, Js::Str);
    let icon = match card.icon {
        Icon::Slack => "slack",
        Icon::Telegram => "send",
        Icon::X => "x",
        Icon::Message => "message-square",
    };
    let context = card.context.map_or(Js::Null, |context| {
        let sections = context.sections.into_iter().map(|section| {
            let lines = section.lines.into_iter().map(|line| {
                let whole = match &line.author {
                    Some(author) => format!("{author} {}", line.text),
                    None => line.text,
                };
                Js::Arr(vec![optional(line.author), Js::Str(whole)])
            });
            let mut out = Obj::new();
            out.insert("heading".into(), Js::Str(section.heading));
            out.insert("lines".into(), Js::Arr(lines.collect()));
            Js::Obj(out)
        });
        let mut out = Obj::new();
        out.insert("label".into(), Js::Str(context.label));
        out.insert("sections".into(), Js::Arr(sections.collect()));
        Js::Obj(out)
    });
    let mut out = Obj::new();
    out.insert("icon".into(), text(icon));
    out.insert("label".into(), Js::Str(card.label));
    out.insert("title".into(), optional(card.title));
    out.insert("context".into(), context);
    Js::Obj(out)
}

#[test]
fn cards_are_the_ones_cindy_draws() {
    let all = include_str!("../../tests/fixtures/hook-card/expected.json");
    let all = js::parse(all).unwrap();
    let t = Translator::new("en").unwrap();
    let cases = all.arr().unwrap();
    for case in cases {
        let [input, expected] = case.arr().unwrap() else {
            panic!("case shape")
        };
        let [source, group] = input.arr().unwrap() else {
            panic!("input shape")
        };
        let found = seen(card(source.obj().unwrap(), group.str(), &t));
        assert_eq!(&found, expected, "{input:?}");
    }
    assert!(cases.len() > 40);
}
