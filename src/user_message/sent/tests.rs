use super::{
    Kind, Part, Pasted, Reference, Span, Token, inline_plain, label, parts, pasted_plain,
    text_starts, tokens, within,
};
use crate::{
    chat_quotes::{
        self,
        tests::{golden, number, object, segment, string, text},
    },
    i18n::Translator,
    rich_answer::js::Js,
};

fn span(value: &Js) -> Span {
    Span {
        start: number(value, "start").unwrap(),
        end: number(value, "end").unwrap(),
    }
}

fn list<T>(args: &[Js], at: usize, read: fn(&Js) -> T) -> Vec<T> {
    let items = args.get(at).and_then(Js::arr).unwrap_or_default();
    items.iter().map(read).collect()
}

fn pasted(value: &Js) -> Pasted {
    Pasted {
        span: span(value),
        display: string(value, "display").unwrap(),
    }
}

fn reference(value: &Js) -> Reference {
    let field = |key: &str| string(value, key);
    let kind = match field("kind").unwrap().as_str() {
        "message" => Kind::Message {
            text: field("text"),
        },
        "session" => Kind::Session {
            title: field("title"),
        },
        "project" => Kind::Project {
            name: field("name").unwrap(),
            working_dir: field("workingDir").unwrap(),
        },
        "browser-tab" => Kind::BrowserTab {
            title: field("title"),
            url: field("url").unwrap(),
        },
        "desktop-window" => Kind::DesktopWindow {
            title: field("title"),
            app_name: field("appName").unwrap(),
        },
        "plugin-resource" => Kind::PluginResource {
            label: field("label").unwrap(),
            resource_id: field("resourceId").unwrap(),
        },
        "bot" => Kind::Bot {
            name: field("name").unwrap(),
            bot_id: field("botId").unwrap(),
        },
        _ => Kind::Todo {
            title: field("title").unwrap(),
            todo_id: field("todoId").unwrap(),
        },
    };
    Reference {
        span: span(value),
        kind,
    }
}

#[test]
fn saved_ranges_cut_the_text_as_cindys() {
    golden("buildSentInlineTokens", |args| {
        let (pasted, slash) = (list(args, 1, pasted), list(args, 2, span));
        let references = list(args, 3, reference);
        let shown = |token: Token| match token {
            Token::Text(body) => object(vec![("kind", text("text")), ("text", text(&body))]),
            Token::Slash(body) => object(vec![("kind", text("slash")), ("text", text(&body))]),
            Token::Pasted {
                text: body,
                display,
            } => object(vec![
                ("kind", text("pasted")),
                ("text", text(&body)),
                ("display", text(display)),
            ]),
            Token::Reference { text: body, index } => object(vec![
                ("kind", text("agent-reference")),
                ("text", text(&body)),
                ("index", Js::Num(index as f64)),
            ]),
        };
        let found = tokens(args[0].str().unwrap(), &pasted, &slash, &references);
        Js::Arr(found.into_iter().map(shown).collect())
    });
}

#[test]
fn chips_read_as_their_words_as_cindys() {
    let t = Translator::new("en").unwrap();
    golden("sentAgentReferenceDisplayLabel", |args| {
        text(&label(&reference(&args[0]), &t))
    });
    golden("projectSentPastedPlainText", |args| {
        text(&pasted_plain(
            args[0].str().unwrap(),
            &list(args, 1, pasted),
        ))
    });
    golden("projectSentInlinePlainText", |args| {
        let (pasted, references) = (list(args, 1, pasted), list(args, 2, reference));
        let content = args[0].str().unwrap();
        text(&inline_plain(content, &pasted, &references, |r| {
            label(r, &t)
        }))
    });
}

#[test]
fn ranges_move_into_their_segment_as_cindys() {
    golden("locateChatQuoteTextSegmentStarts", |args| {
        let content = args[0].str().unwrap();
        let segments = match args.get(1) {
            Some(given) => given.arr().unwrap().iter().map(segment).collect(),
            None => chat_quotes::segments(content),
        };
        let starts = text_starts(content, &segments).into_iter();
        Js::Arr(
            starts
                .map(|at| at.map_or(Js::Null, |at| Js::Num(at as f64)))
                .collect(),
        )
    });
    golden("projectSentRanges", |args| {
        let start = args[1].integer().map(|at| at as usize);
        let moved = list(args, 0, span).into_iter().filter_map(|span| {
            let span = within(span, start, args[2].integer().unwrap() as usize)?;
            Some(object(vec![
                ("start", Js::Num(span.start)),
                ("end", Js::Num(span.end)),
            ]))
        });
        Js::Arr(moved.collect())
    });
}

#[test]
fn words_name_their_chips_as_cindys() {
    golden("sentTextParts", |args| {
        let shown = |part: Part| match part {
            Part::Text(body) => text(&body),
            Part::Command(label) => object(vec![("command", text(&label))]),
            Part::Agent(name) => object(vec![("agent", text(&name))]),
            Part::Dir { label, path } => object(vec![("dir", text(&label)), ("path", text(&path))]),
            Part::File { name, path } => object(vec![("file", text(&name)), ("path", text(&path))]),
        };
        // The golden joins neighbouring strings.
        let mut joined: Vec<Part> = Vec::new();
        for part in parts(args[0].str().unwrap(), args[1] == Js::Bool(true)) {
            match (joined.last_mut(), part) {
                (Some(Part::Text(last)), Part::Text(next)) => last.push_str(&next),
                (_, part) => joined.push(part),
            }
        }
        Js::Arr(joined.into_iter().map(shown).collect())
    });
}
