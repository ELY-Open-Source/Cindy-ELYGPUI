//! `projectMcpAppHistory`: an App row leaves the database only as its public history.

use crate::{
    i18n::Translator,
    rich_answer::js::{self, Js, Obj},
};

const TRUSTS: [&str; 4] = [
    "unverified",
    "cindy-official",
    "reviewed",
    "verified-publisher",
];

/// `mcpApps.pluginTrust.*` in the app's language, as Main's `t` reads them.
pub struct TrustLabels([String; 4]);

impl TrustLabels {
    pub fn new(t: &Translator) -> Self {
        TrustLabels(TRUSTS.map(|trust| t.t(&format!("mcpApps.pluginTrust.{trust}"), &[])))
    }

    fn of(&self, trust: &str) -> Option<&str> {
        let at = TRUSTS.iter().position(|t| *t == trust)?;
        Some(&self.0[at])
    }
}

fn text<'a>(o: &'a Obj, key: &str) -> Option<&'a str> {
    o.get(key)?.str()
}

/// `parseMcpAppRecord`.
pub fn record(content: &Js) -> Option<&Obj> {
    let r = content.obj()?.get("mcpApp")?.obj()?;
    let tool = r.get("tool")?.obj()?;
    let schema_object = tool.get("inputSchema")?.obj()?.get("type")?.str() == Some("object");
    let result_ok = !js::truthy(r.get("result"))
        || matches!(
            r.get("result")
                .and_then(Js::obj)
                .and_then(|o| o.get("content")),
            Some(Js::Arr(_))
        );
    (r.get("version") == Some(&Js::Num(1.0))
        && text(r, "serverId").is_some()
        && text(r, "serverRevision").is_some()
        && text(r, "resourceUri").is_some_and(|u| u.starts_with("ui://"))
        && text(tool, "name").is_some()
        && schema_object
        && matches!(r.get("arguments"), Some(Js::Obj(_)))
        && result_ok)
        .then_some(r)
}

/// `mcpAppHistoryText`: the result's text blocks.
fn history_text(r: &Obj) -> String {
    let blocks = r
        .get("result")
        .and_then(Js::obj)
        .and_then(|o| o.get("content"));
    let Some(Js::Arr(blocks)) = blocks else {
        return String::new();
    };
    blocks
        .iter()
        .filter_map(|b| {
            let b = b.obj()?;
            (b.get("type")?.str()? == "text").then_some(())?;
            b.get("text")?.str()
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// `parseMcpAppHistory`: a record, or an already public summary.
pub fn history(content: &Js) -> Option<(String, String)> {
    if let Some(r) = record(content) {
        let tool = r.get("tool")?.obj()?;
        let title = text(tool, "title").or(text(tool, "name"))?;
        return Some((title.to_owned(), history_text(r)));
    }
    let s = content.obj()?.get("mcpAppHistory")?.obj()?;
    (s.get("version") == Some(&Js::Num(1.0)))
        .then(|| Some((text(s, "title")?.to_owned(), text(s, "text")?.to_owned())))?
}

fn summary(title: String, text: String) -> Js {
    let inner = Obj::from([
        ("version".to_owned(), Js::Num(1.0)),
        ("title".to_owned(), Js::Str(title)),
        ("text".to_owned(), Js::Str(text)),
    ]);
    Js::Obj(Obj::from([("mcpAppHistory".to_owned(), Js::Obj(inner))]))
}

/// A text is the record it holds; one that does not parse holds none.
fn reparsed(content: &Js) -> Option<Js> {
    match content {
        Js::Str(s) => js::parse(s).ok(),
        other => Some(other.clone()),
    }
}

/// `projectMcpAppHistory(content, marked)`; unmarked rows keep their content.
pub fn project(content: Js, marked: bool, labels: &TrustLabels) -> Js {
    if !marked {
        return content;
    }
    let value = reparsed(&content);
    let (title, body) = value.as_ref().and_then(history).unwrap_or_default();
    let plugin = value.as_ref().and_then(record).and_then(|r| {
        r.get("tool")?
            .obj()?
            .get("_meta")?
            .obj()?
            .get("cindy/plugin")?
            .obj()
    });
    let trusted = plugin.and_then(|p| Some((text(p, "name")?, labels.of(text(p, "trust")?)?)));
    match trusted {
        Some((name, label)) => summary(format!("{name} · {label}"), body),
        None => summary(title, body),
    }
}

#[cfg(test)]
mod tests;
