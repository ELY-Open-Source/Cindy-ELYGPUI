//! `AutomationOriginBadge` and the plugin and device badges: words, tooltip, a click's route.

use icu_segmenter::GraphemeClusterSegmenter;

use crate::{
    chat_message::merge::{Fields, get},
    cindy_json::encode_uri_component,
    i18n::{Arg, Translator},
    rich_answer::js::{Js, Obj},
    theme_import::color::js_trim,
};

#[derive(Debug, PartialEq)]
pub enum Icon {
    Hook,
    Session,
    Scheduler,
    Plugin,
    Phone,
    Computer,
    /// `BotAvatar` for a teammate of this name.
    Bot(String),
}

#[derive(Debug, PartialEq)]
pub struct Label {
    /// `data-message-origin`.
    pub kind: &'static str,
    pub icon: Icon,
    pub text: String,
    pub tip: Option<String>,
    /// The route a click opens; none draws plain words.
    pub open: Option<String>,
}

fn text<'a>(record: &'a Obj, key: &str) -> Option<&'a str> {
    record.get(key)?.str().filter(|text| !text.is_empty())
}

/// `joinSourceTooltip`.
fn tooltip(lines: Vec<Option<String>>) -> Option<String> {
    let text = lines.into_iter().flatten().collect::<Vec<_>>().join("\n");
    (!text.is_empty()).then_some(text)
}

/// `isCrossDeviceSourceSessionId`: `deviceId::sessionId`.
fn cross_device(id: &str) -> bool {
    id.find("::")
        .is_some_and(|split| split > 0 && split < id.len() - 2)
}

/// `hookConnectionNameFromOrigin`: the schedule's name less its `Hook · `.
fn connection(origin: &Obj) -> Option<&str> {
    let name = js_trim(origin.get("scheduleName")?.str()?);
    let rest = name.strip_prefix("Hook").and_then(|rest| {
        let (gap, rest) = rest.split_once('·')?;
        js_trim(gap).is_empty().then_some(rest)
    });
    let name = js_trim(rest.unwrap_or(name));
    (!name.is_empty()).then_some(name)
}

/// `MessageSourcePluginBadge`.
fn plugin(plugin: &Obj, t: &Translator) -> Option<Label> {
    let id = text(plugin, "pluginId")?;
    let name = text(plugin, "name");
    let named = |key, name| t.t(key, &[("name", Arg::Text(name))]);
    let local = id.strip_prefix("local-app:");
    let app = id.strip_prefix("mcp-app:");
    let (words, ids, bare) = match (local, app) {
        (Some(bare), _) => ("localAgent.messageSource", "localApp", bare),
        (None, Some(bare)) => ("mcpApps.messageSource", "mcpApp", bare),
        (None, None) => ("chat.userMessage.pluginSentNamed", "plugin", id),
    };
    let own = local.is_none() && app.is_none();
    let label = match name {
        None if own => t.t("chat.userMessage.pluginSent", &[]),
        name => named(words, name.unwrap_or(bare)),
    };
    let ids = format!("chat.userMessage.sourceIds.{ids}");
    Some(Label {
        kind: "plugin",
        icon: Icon::Plugin,
        text: label,
        tip: tooltip(vec![
            own.then(|| t.t("chat.userMessage.pluginViewDetail", &[])),
            Some(t.t(&ids, &[("id", Arg::Text(bare))])),
        ]),
        open: own.then(|| format!("/plugins?ghost={}", encode_uri_component(id))),
    })
}

/// A task's message: the teammate or the task that sent it.
fn session(origin: &Obj, live: impl Fn(&str) -> Option<String>, t: &Translator) -> Option<Label> {
    let sender = text(origin, "senderSessionId");
    // `hasAutomationOriginBadge`: an Orca message names a task or draws no label.
    if sender.is_none() && origin.get("orca") == Some(&Js::Bool(true)) {
        return None;
    }
    let named = |key, name: &str| t.t(key, &[("name", Arg::Text(name))]);
    let id = |kind: &str, id| {
        let key = format!("chat.userMessage.sourceIds.{kind}");
        t.t(&key, &[("id", Arg::Text(id))])
    };
    let bot = text(origin, "senderBotId");
    let bot_name = bot.map(|id| text(origin, "senderBotName").unwrap_or(id));
    let title = sender.and_then(live);
    let title = title.as_deref().or(text(origin, "senderSessionTitle"));
    let label = match (bot_name, title) {
        (Some(name), _) => named("chat.userMessage.botSentNamed", name),
        (None, Some(title)) => named("chat.userMessage.sessionSentNamed", title),
        (None, None) => t.t("chat.userMessage.sessionSent", &[]),
    };
    let open = sender.filter(|id| !cross_device(id));
    Some(Label {
        kind: if bot.is_some() { "bot" } else { "session" },
        icon: bot_name.map_or(Icon::Session, |name| Icon::Bot(name.to_owned())),
        text: label,
        tip: tooltip(vec![
            open.map(|_| t.t("chat.userMessage.sessionViewSource", &[])),
            bot.map(|bot| id("teammate", bot)),
            sender.map(|sender| id("session", sender)),
        ]),
        open: open.map(|id| format!("/cc-agent/{}", encode_uri_component(id))),
    })
}

/// An automation's message, or a hook channel's, which borrows the automation's form.
fn scheduler(origin: &Obj, t: &Translator) -> Label {
    let raw = origin.get("scheduleId").and_then(Js::str);
    if raw.is_some_and(|id| id.starts_with("hook:")) {
        let channel = connection(origin).unwrap_or("Hook");
        let args = [("channel", Arg::Text(channel))];
        return Label {
            kind: "hook",
            icon: Icon::Hook,
            text: t.t("chat.userMessage.hookChannelFrom", &args),
            tip: None,
            open: None,
        };
    }
    let id = raw.map(js_trim).filter(|id| !id.is_empty());
    let label = match text(origin, "scheduleName") {
        Some(name) => {
            let args = [("name", Arg::Text(name))];
            t.t("chat.userMessage.automationSentNamed", &args)
        }
        None => t.t("chat.userMessage.automationSent", &[]),
    };
    let ids = |id| t.t("chat.userMessage.sourceIds.automation", &[("id", id)]);
    Label {
        kind: "scheduler",
        icon: Icon::Scheduler,
        text: label,
        tip: tooltip(vec![
            id.map(|_| t.t("chat.userMessage.automationViewTask", &[])),
            id.map(|id| ids(Arg::Text(id))),
        ]),
        open: id.map(|id| format!("/cc-agent/scheduled?focus={}", encode_uri_component(id))),
    }
}

/// `MessageSourceDeviceBadge`, with no device list and no signed-in device yet.
fn device(device: &Obj, t: &Translator) -> Option<Label> {
    let id = text(device, "deviceId")?;
    let mobile = device.get("platform").and_then(Js::str) == Some("mobile");
    let key = match (mobile, text(device, "name").is_some()) {
        (true, true) => "chat.userMessage.deviceSentMobileNamed",
        (true, false) => "chat.userMessage.deviceSentMobile",
        (false, true) => "chat.userMessage.deviceSentDesktopNamed",
        (false, false) => "chat.userMessage.deviceSentDesktop",
    };
    let name = text(device, "name").unwrap_or_default();
    let path = "/settings?tab=remote-control&section=devices&device=";
    Some(Label {
        kind: "device",
        icon: if mobile { Icon::Phone } else { Icon::Computer },
        text: t.t(key, &[("name", Arg::Text(name))]),
        tip: tooltip(vec![
            Some(t.t("chat.userMessage.deviceViewSettings", &[])),
            Some(t.t(
                "chat.userMessage.sourceIds.device",
                &[("id", Arg::Text(id))],
            )),
        ]),
        open: Some(format!("{path}{}", encode_uri_component(id))),
    })
}

/// `botAvatarInitial`: the name's first grapheme in upper case.
pub fn initial(name: &str) -> String {
    let name = js_trim(name);
    let end = GraphemeClusterSegmenter::new().segment_str(name).nth(1);
    match end {
        Some(end) => name[..end].to_uppercase(),
        None => "?".into(),
    }
}

/// `MessageSourceLabels`: one sender at most, then the device; `live` is a task's held title.
pub fn labels(
    message: &Fields,
    live: impl Fn(&str) -> Option<String>,
    t: &Translator,
) -> Vec<Label> {
    let record = |key| get(message, key).and_then(Js::obj);
    let sender = match (record("sourcePlugin"), record("automationOrigin")) {
        (Some(source), _) => plugin(source, t),
        (None, Some(origin)) => match origin.get("kind").and_then(Js::str) {
            Some("session") => session(origin, live, t),
            Some("scheduler") => Some(scheduler(origin, t)),
            _ => None,
        },
        (None, None) => None,
    };
    let device = record("sourceDevice").and_then(|source| device(source, t));
    sender.into_iter().chain(device).collect()
}

#[cfg(test)]
mod tests;
