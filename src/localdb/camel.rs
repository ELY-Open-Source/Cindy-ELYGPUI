//! `mapper.ts`'s `messageToCamel` for the columns its readers share.

use super::search::mcp_app::TrustLabels;
use crate::rich_answer::js::{self, Js};

/// `agentMeta.mcpApp === true` over `JSON.parse`; a meta that does not parse is `null`.
fn mcp_app_marked(agent_meta: Option<&str>) -> bool {
    agent_meta
        .and_then(|m| js::parse(m).ok())
        .is_some_and(|m| m.obj().and_then(|o| o.get("mcpApp")) == Some(&Js::Bool(true)))
}

/// `messageToCamel`'s content: parsed JSON, else the text; App rows as their public history.
pub fn content(raw: String, row: (&str, &str, Option<&str>), labels: &TrustLabels) -> Js {
    let (role, client_id, agent_meta) = row;
    let mut content = js::parse(&raw).unwrap_or(Js::Str(raw));
    if role == "assistant" && client_id.starts_with("cloud-history:") {
        content = imported_reply_text(content);
    }
    let content = match &content {
        Js::Str(text) if role == "assistant" => {
            Js::Str(crate::sessions::preview::strip_internal_web_citations(text))
        }
        _ => content,
    };
    super::search::mcp_app::project(content, mcp_app_marked(agent_meta), labels)
}

/// `importedReplyText`: text blocks joined, anything else left as it is.
fn imported_reply_text(content: Js) -> Js {
    let joined = match &content {
        Js::Arr(blocks) if !blocks.is_empty() => blocks
            .iter()
            .map(|b| {
                let b = b.obj()?;
                (b.get("type")?.str()? == "text").then_some(())?;
                b.get("text")?.str()
            })
            .collect::<Option<Vec<_>>>()
            .map(|texts| texts.concat()),
        _ => None,
    };
    joined.map_or(content, Js::Str)
}
