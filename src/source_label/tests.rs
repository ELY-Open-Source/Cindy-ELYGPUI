use std::rc::Rc;

use super::{Icon, Label, initial, labels};
use crate::{
    chat_message::merge::Fields,
    i18n::Translator,
    rich_answer::js::{self, Js, Obj},
};

/// A label as the golden reads Cindy's off the DOM.
fn seen(label: Label) -> Js {
    let text = |text: &str| Js::Str(text.to_owned());
    let optional = |value: Option<String>| value.map_or(Js::Null, Js::Str);
    let icon = match &label.icon {
        Icon::Hook => "message-square",
        Icon::Session => "send",
        Icon::Scheduler => "timer",
        Icon::Plugin => "puzzle",
        Icon::Phone => "smartphone",
        Icon::Computer => "monitor",
        Icon::Bot(_) => "bot",
    };
    let mut out = Obj::new();
    out.insert("kind".into(), text(label.kind));
    out.insert("icon".into(), text(icon));
    if let Icon::Bot(name) = &label.icon {
        out.insert("initial".into(), Js::Str(initial(name)));
        out.insert("tint".into(), text("var(--bot-avatar-violet-bg)"));
    }
    out.insert("text".into(), Js::Str(label.text));
    out.insert("tip".into(), optional(label.tip));
    out.insert("open".into(), optional(label.open));
    Js::Obj(out)
}

#[test]
fn labels_are_the_ones_cindy_draws() {
    let all = include_str!("../../tests/fixtures/source-labels/expected.json");
    let all = js::parse(all).unwrap();
    let t = Translator::new("en").unwrap();
    let cases = all.arr().unwrap();
    for case in cases {
        let [input, expected] = case.arr().unwrap() else {
            panic!("case shape")
        };
        let [props, held] = input.arr().unwrap() else {
            panic!("input shape")
        };
        let fields = props.obj().unwrap().iter();
        let message: Fields = fields
            .map(|(key, value)| (key.clone(), Some(Rc::new(value.clone()))))
            .collect();
        let live = |id: &str| {
            let title = crate::theme_import::color::js_trim(held.obj()?.get(id)?.str()?);
            (!title.is_empty()).then(|| title.to_owned())
        };
        let found: Vec<Js> = labels(&message, true, live, &t)
            .into_iter()
            .map(seen)
            .collect();
        assert_eq!(&Js::Arr(found), expected, "{input:?}");
    }
    assert!(cases.len() > 40);
}

#[test]
fn a_blank_name_has_no_initial() {
    assert_eq!(initial(" \u{3000}"), "?");
    assert_eq!(initial(""), "?");
    assert_eq!(initial(" ada"), "A");
}
