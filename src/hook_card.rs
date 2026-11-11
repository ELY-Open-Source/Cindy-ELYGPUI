//! `HookTaskCard`'s words: the heading that names who spoke, and the context a message carries.

use crate::{
    i18n::{Arg, Translator},
    rich_answer::js::{self, Js, Obj, truthy},
    theme_import::color::js_trim,
};

/// `ImIcon`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Icon {
    Slack,
    Telegram,
    X,
    Message,
}

/// A line of context; a thread entry leads with its `[author]`.
#[derive(Debug, PartialEq)]
pub struct Line {
    pub author: Option<String>,
    pub text: String,
}

#[derive(Debug, PartialEq)]
pub struct Section {
    pub heading: String,
    pub lines: Vec<Line>,
}

/// The attached context behind one disclosure.
#[derive(Debug, PartialEq)]
pub struct Context {
    pub label: String,
    pub sections: Vec<Section>,
}

#[derive(Debug, PartialEq)]
pub struct Card {
    pub icon: Icon,
    pub label: String,
    pub title: Option<String>,
    pub context: Option<Context>,
}

/// `imLabel`: the channel's name; one not known reads as written.
fn im_label(im: &str, t: &Translator) -> String {
    let key = match im {
        "slack" => "settings.tina.prefs.providerSlack",
        "telegram" => "settings.tina.prefs.providerTelegram",
        "x" => "settings.tina.prefs.providerX",
        "feishu" => "settings.feishuBot.services.feishu",
        "lark" => "settings.feishuBot.services.lark",
        "discord" => "settings.about.social.discordLabel",
        "wechat" => "login.social.wechat",
        "wecom" => "settings.wecomBot.serviceName",
        "dingtalk" => "settings.dingtalkBot.serviceName",
        _ => return im.to_owned(),
    };
    t.t(key, &[])
}

/// A string field that is not empty.
fn text<'a>(record: Option<&'a Obj>, key: &str) -> Option<&'a str> {
    record?.get(key)?.str().filter(|text| !text.is_empty())
}

/// A string field, trimmed, that is not empty.
fn trimmed<'a>(record: Option<&'a Obj>, key: &str) -> Option<&'a str> {
    let text = js_trim(record?.get(key)?.str()?);
    (!text.is_empty()).then_some(text)
}

/// `hookTaskCardHeading`: the owner's message is Cindy's; another's names its speaker.
fn heading(im: &str, origin: Option<&Obj>, t: &Translator) -> (String, Option<String>) {
    let platform = im_label(im, t);
    let record = |key| origin?.get(key)?.obj();
    let sender = record("sender").filter(|sender| !truthy(sender.get("isOwner")));
    let Some(sender) = sender else {
        let args = [("platform", Arg::Text(&platform))];
        return (t.t("chat.threadContext.cindyFrom", &args), None);
    };
    let id = text(Some(sender), "id");
    let unknown = t.t("chat.threadContext.unknownSender", &[]);
    let name = trimmed(Some(sender), "name").or(id).unwrap_or(&unknown);
    let chat = record("chat");
    let group = trimmed(chat, "name");
    let mut args = vec![
        ("name", Arg::Text(name)),
        ("platform", Arg::Text(&platform)),
    ];
    let key = match (text(chat, "kind") == Some("direct"), group) {
        (true, _) => "chat.threadContext.guestFromDm",
        (false, Some(group)) => {
            args.push(("group", Arg::Text(group)));
            "chat.threadContext.memberFromGroup"
        }
        (false, None) => "chat.threadContext.memberFromPlatform",
    };
    (t.t(key, &args), id.map(|id| format!("user_id: {id}")))
}

/// `isImMessageCount`: a safe integer above zero, kept a JavaScript Number.
fn count(value: Option<&Js>) -> Option<f64> {
    let Some(Js::Num(n)) = value else {
        return None;
    };
    (n.fract() == 0.0 && *n > 0.0 && *n <= 9_007_199_254_740_991.0).then_some(*n)
}

/// A heading with its count, or the plain one while the count is not known.
fn counted(count: Option<f64>, with: &str, plain: &str, t: &Translator) -> String {
    match count {
        Some(count) => t.t(with, &[("count", Arg::Js(count))]),
        None => t.t(plain, &[]),
    }
}

/// The referenced messages and the group background, each with what it counts.
fn context(source: &Obj, group: Option<&str>, t: &Translator) -> Option<Context> {
    let entries = source
        .get("threadContext")
        .and_then(Js::arr)
        .unwrap_or_default();
    let snapshot = source.get("contextSnapshot").and_then(Js::obj);
    let filled = |text: &&str| !js_trim(text).is_empty();
    let reply = snapshot
        .and_then(|s| s.get("replyContext")?.str())
        .filter(filled);
    let group = group.filter(filled);
    if entries.is_empty() && reply.is_none() && group.is_none() {
        return None;
    }
    let stored = |key| count(snapshot.and_then(|snapshot| snapshot.get(key)));
    let replies = match reply {
        None => Some(entries.len() as f64),
        Some(_) => stored("replyMessageCount").map(|count| count + entries.len() as f64),
    };
    let grouped = match group {
        None => Some(0.0),
        Some(_) => stored("groupMessageCount"),
    };
    let mut sections = Vec::new();
    if !entries.is_empty() || reply.is_some() {
        let quoted = reply.map(|text| Line {
            author: None,
            text: text.to_owned(),
        });
        // React draws a string or a number child, and nothing for the rest.
        let field = |entry: &Js, key| match entry.obj().and_then(|entry| entry.get(key)) {
            Some(value @ (Js::Str(_) | Js::Num(_))) => js::string(Some(value)),
            _ => String::new(),
        };
        let said = entries.iter().map(|entry| Line {
            author: Some(format!("[{}]", field(entry, "author"))),
            text: field(entry, "text"),
        });
        sections.push(Section {
            heading: counted(
                replies,
                "chat.threadContext.referencedMessages",
                "chat.threadContext.referencedContext",
                t,
            ),
            lines: quoted.into_iter().chain(said).collect(),
        });
    }
    if let Some(group) = group {
        sections.push(Section {
            heading: counted(
                grouped,
                "chat.threadContext.groupBackgroundCount",
                "chat.threadContext.groupBackground",
                t,
            ),
            lines: vec![Line {
                author: None,
                text: group.to_owned(),
            }],
        });
    }
    Some(Context {
        label: counted(
            replies
                .zip(grouped)
                .map(|(replies, grouped)| replies + grouped),
            "chat.threadContext.attachedContextCount",
            "chat.threadContext.attachedContext",
            t,
        ),
        sections,
    })
}

/// The card of a message from a channel; `group` is `resolveHookGroupContext`'s.
pub fn card(source: &Obj, group: Option<&str>, t: &Translator) -> Card {
    let im = source.get("im").and_then(Js::str).unwrap_or_default();
    let (label, title) = heading(im, source.get("origin").and_then(Js::obj), t);
    Card {
        icon: match im {
            "slack" => Icon::Slack,
            "telegram" => Icon::Telegram,
            "x" => Icon::X,
            _ => Icon::Message,
        },
        label,
        title,
        context: context(source, group, t),
    }
}

#[cfg(test)]
mod tests;
