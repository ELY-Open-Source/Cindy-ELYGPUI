use super::{
    Local, Place, Reason, Target, Verdict, ambiguous, bare_file, inline_code, link, remote_lit,
    sandbox, split_line,
};
use crate::{
    deep_link::tests::{object, text},
    rich_answer::js::{self, Js},
};

/// Runs `run` on each recorded argument list of `name` in the local-paths golden.
pub fn golden(name: &str, run: impl Fn(&[Js]) -> Js) {
    let all = js::parse(include_str!(
        "../../tests/fixtures/local-paths/expected.json"
    ))
    .unwrap();
    let cases = all.obj().unwrap()[name].arr().unwrap();
    assert!(!cases.is_empty(), "{name}");
    let mut wrong = 0;
    for case in cases {
        let [args, expected] = case.arr().unwrap() else {
            panic!("case shape")
        };
        let port = js::stringify(&run(args.arr().unwrap()), "");
        let cindy = js::stringify(expected, "");
        if port != cindy {
            wrong += 1;
            eprintln!(
                "{name} {}\n  cindy {cindy}\n  port  {port}",
                js::stringify(args, "")
            );
        }
    }
    assert_eq!(wrong, 0, "{name}: of {}", cases.len());
}

/// An argument that may be absent.
pub fn optional(args: &[Js], at: usize) -> Option<&str> {
    args.get(at).and_then(|arg| arg.str())
}

/// The attached files of an argument: pairs, or Cindy's `{ name, path }`.
pub fn files(args: &[Js], at: usize) -> Vec<(String, String)> {
    let Some(list) = args.get(at).and_then(|arg| arg.arr()) else {
        return Vec::new();
    };
    let field = |value: &Js| value.str().unwrap().to_owned();
    let named = |file: &Js, key: &str| field(&file.obj().unwrap()[key]);
    list.iter()
        .map(|file| match file.arr() {
            Some(pair) => (field(&pair[0]), field(&pair[1])),
            None => (named(file, "name"), named(file, "path")),
        })
        .collect()
}

fn place(fields: &mut Vec<(&str, Js)>, at: Place) {
    for (key, value) in [("line", at.line), ("column", at.column)] {
        if let Some(value) = value {
            fields.push((key, Js::Num(f64::from(value))));
        }
    }
}

fn local(kind: Local) -> Js {
    text(match kind {
        Local::Image => "image",
        Local::Model => "model",
        Local::Text => "text",
    })
}

fn reason(reason: Reason) -> Js {
    text(match reason {
        Reason::Directory => "directory",
        Reason::PathLike => "path-like-unsupported",
        Reason::Scheme => "unsupported-scheme",
        Reason::Empty => "empty",
        Reason::NotATarget => "not-a-target",
    })
}

fn target(target: Target) -> Js {
    let mut fields = Vec::new();
    let mut push = |key: &'static str, value: String| fields.push((key, Js::Str(value)));
    let (kind, at) = match target {
        Target::External { href } => {
            push("href", href);
            ("external", None)
        }
        Target::Anchor { id, href } => {
            push("id", id);
            push("href", href);
            ("anchor", None)
        }
        Target::Audio { href } => {
            push("href", href);
            ("audio", None)
        }
        Target::LocalImageUrl { href } => {
            push("href", href);
            ("local-image-url", None)
        }
        Target::Resolved {
            href,
            path,
            kind,
            at,
        } => {
            push("href", href);
            push("absPath", path);
            fields.push(("localKind", local(kind)));
            ("resolved-local", Some(at))
        }
        Target::Candidate {
            href,
            original,
            kind,
            at,
        } => {
            push("href", href);
            push("originalHref", original);
            fields.push(("localKind", local(kind)));
            ("local-candidate", Some(at))
        }
        Target::Code { href, reason: why } => {
            push("href", href);
            fields.push(("reason", reason(why)));
            ("code-reference", None)
        }
        Target::Plain { href, reason: why } => {
            push("href", href);
            fields.push(("reason", reason(why)));
            ("plain-text", None)
        }
    };
    fields.insert(0, ("kind", text(kind)));
    place(&mut fields, at.unwrap_or_default());
    object(fields)
}

#[test]
fn line_suffixes_split_as_cindys() {
    golden("splitLocalLineSuffix", |args| {
        let (href, at) = split_line(args[0].str().unwrap());
        let mut fields = vec![("href", text(href))];
        place(&mut fields, at);
        object(fields)
    });
}

#[test]
fn shapes_read_as_cindys() {
    let first = |args: &[Js]| args[0].str().unwrap().to_owned();
    golden("looksLikeBareFileReference", |args| {
        Js::Bool(bare_file(&first(args)))
    });
    golden("normalizeSandboxHref", |args| text(sandbox(&first(args))));
    golden("isAmbiguousPathShape", |args| {
        Js::Bool(ambiguous(&first(args), optional(args, 1)))
    });
}

#[test]
fn a_remote_verdict_lights_as_cindys() {
    golden("decideRemoteLit", |args| {
        let verdict = optional(args, 0).map(|verdict| match verdict {
            "file" => Verdict::File,
            "directory" => Verdict::Directory,
            "nonfile" => Verdict::Nonfile,
            "unknown" => Verdict::Unknown,
            other => panic!("verdict {other}"),
        });
        match remote_lit(verdict, args[1].str().unwrap(), optional(args, 2)) {
            None => object(vec![("lit", Js::Bool(false))]),
            Some(directory) => {
                let kind = if directory { "directory" } else { "file" };
                object(vec![("lit", Js::Bool(true)), ("kind", text(kind))])
            }
        }
    });
}

#[test]
fn targets_classify_as_cindys() {
    golden("classifyMarkdownLinkTarget", |args| {
        target(link(optional(args, 0), &files(args, 1)))
    });
    golden("classifyInlineCodeTarget", |args| {
        inline_code(args[0].str().unwrap()).map_or(Js::Null, target)
    });
}
