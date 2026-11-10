use super::{collapse, display_text, hook_group_context, orca};
use crate::rich_answer::js::{self, Js, Obj};

/// Runs `run` on each recorded argument list of `name` in the golden.
fn golden(name: &str, run: impl Fn(&[Js]) -> Js) {
    let all = js::parse(include_str!(
        "../../tests/fixtures/user-message/expected.json"
    ))
    .unwrap();
    let cases = all.obj().unwrap()[name].arr().unwrap();
    assert!(!cases.is_empty(), "{name}");
    for case in cases {
        let [args, expected] = case.arr().unwrap() else {
            panic!("case shape")
        };
        assert_eq!(
            js::stringify(&run(args.arr().unwrap()), ""),
            js::stringify(expected, ""),
            "{name} {}",
            js::stringify(args, "")
        );
    }
}

fn text(text: &str) -> Js {
    Js::Str(text.to_owned())
}

fn content(source: &Js) -> &str {
    source.obj().unwrap()["content"].str().unwrap()
}

#[test]
fn orca_rows_parse_as_cindys() {
    golden("parseOrcaCommunicationContent", |args| {
        let Some(found) = orca(args[0].str().unwrap()) else {
            return Js::Null;
        };
        let fields = [("orcaSource", found.source), ("content", found.content)];
        Js::Obj(Obj::from_iter(
            fields.map(|(key, value)| (key.to_owned(), Js::Str(value))),
        ))
    });
}

#[test]
fn the_display_text_is_cindys() {
    golden("resolveUserDisplayText", |args| {
        let source = args[0].obj().unwrap();
        let origin = source.get("automationOrigin").and_then(Js::obj);
        let kind = origin
            .and_then(|origin| origin.get("kind"))
            .and_then(Js::str);
        let body = content(&args[0]);
        let given = |value: &Js| {
            let field = |key: &str| value.obj()?.get(key)?.str().map(str::to_owned);
            Some(super::Orca {
                source: field("orcaSource")?,
                content: field("content")?,
            })
        };
        let parsed = args.get(1).map_or_else(|| orca(body), given);
        text(&display_text(
            body,
            source.get("hookSource"),
            kind == Some("session"),
            parsed.as_ref(),
        ))
    });
}

#[test]
fn the_group_context_is_cindys() {
    golden("resolveHookGroupContext", |args| {
        let hook = args[0].obj().unwrap().get("hookSource");
        hook_group_context(content(&args[0]), hook).map_or(Js::Null, text)
    });
}

fn threshold(args: &[Js]) -> usize {
    args.get(1)
        .map_or(collapse::LONG, |n| n.integer().unwrap() as usize)
}

#[test]
fn the_estimates_are_cindys() {
    golden("shouldAutoCollapseUserMessageContent", |args| {
        Js::Bool(collapse::should_collapse(
            args[0].str().unwrap(),
            threshold(args),
        ))
    });
    golden("mayExceedVisualLineThreshold", |args| {
        Js::Bool(collapse::may_exceed(
            args[0].str().unwrap(),
            threshold(args),
        ))
    });
}
