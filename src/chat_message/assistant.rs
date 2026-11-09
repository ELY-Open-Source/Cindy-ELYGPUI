//! `mapServerMessages`'s last branch: any other row, with an assistant's costs and seals.

use super::{Out, base, bot, money, source, subagent, usage};
use crate::rich_answer::js::{self, Js, Obj};

fn positive(value: Option<&Js>) -> Option<f64> {
    match value {
        Some(Js::Num(n)) if *n > 0.0 => Some(*n),
        _ => None,
    }
}

/// `resolveEstimatedTurnCostUsd`.
fn estimated_cost(meta: &Obj, raw: f64, details: Option<&Obj>) -> Result<f64, String> {
    if meta.get("turnCostIsEstimate") != Some(&Js::Bool(true)) {
        return Ok(raw);
    }
    let repaired = money::stale_codex_estimate(raw, details, meta.get("model"))?;
    Ok(repaired
        .filter(|r| r.is_finite() && *r > 0.0)
        .unwrap_or(raw))
}

pub fn other(row: &Obj, meta: Option<&Obj>) -> Result<Out, String> {
    let role = row["role"].str().unwrap_or("");
    let assistant = role == "assistant";
    let field = |key: &str| meta.and_then(|m| m.get(key));
    let details = match assistant {
        true => usage::normalize(field("turnUsageDetails"))?,
        false => None,
    };
    let normalized = assistant
        .then(|| money::normalize(field("turnCost")))
        .flatten();
    let legacy = match (meta, positive(field("turnCostUsd"))) {
        (Some(m), Some(raw)) if assistant => Some(estimated_cost(m, raw, details.as_ref())?),
        _ => None,
    };
    let turn_money = match (normalized, legacy) {
        (Some(money), _) => Some(money),
        (None, Some(usd)) => Some(money::legacy_usd(usd)?),
        (None, None) => None,
    };
    let user_money = match meta.filter(|_| assistant) {
        Some(m) => match money::normalize(m.get("userTurnCost")) {
            Some(money) => Some(money),
            None => positive(m.get("userTurnCostUsd"))
                .map(money::legacy_usd)
                .transpose()?,
        },
        None => None,
    };
    let completed = match field("turnCompleted") {
        _ if !assistant => None,
        Some(Js::Bool(b)) => Some(*b),
        _ => (turn_money.as_ref().map_or(0.0, money::amount_of) > 0.0 || details.is_some())
            .then_some(true),
    };
    let content = match &row["content"] {
        Js::Str(s) => s.clone(),
        other => js::stringify(other, ""),
    };
    let mut out = base(row, &row["role"], Js::Str(content));
    if assistant {
        out.set("sourceGroup", source::group(meta));
    }
    let mut put = |key: &str, value: Option<Js>| out.set(key, value);
    if assistant && let Some(phase @ Js::Str(_)) = field("assistantPhase") {
        put("assistantPhase", Some(phase.clone()));
    }
    if assistant && js::truthy(field("failedSamplingAttempt")) {
        put("failedSamplingAttempt", Some(Js::Bool(true)));
    }
    if assistant && field("explicitDelivery") == Some(&Js::Bool(true)) {
        put("explicitDelivery", Some(Js::Bool(true)));
    }
    let options = match assistant {
        true => bot::starter_options(field("botStarterOptions")),
        false => Vec::new(),
    };
    if !options.is_empty() {
        put("botStarterOptions", Some(Js::Arr(options)));
    }
    if assistant {
        put("botLearning", field("botLearning").cloned());
    }
    if field("botPrivateReply") == Some(&Js::Bool(true)) {
        put("botPrivateReply", Some(Js::Bool(true)));
    }
    if assistant
        && let Some(reply @ Js::Str(r)) = field("replyToClientId")
        && !r.is_empty()
    {
        put("replyToClientId", Some(reply.clone()));
    }
    if assistant && field("turnCompleted") == Some(&Js::Bool(true)) {
        put(
            "botTaskResults",
            Some(Js::Arr(bot::task_results(field("botTaskResults")))),
        );
    }
    if role == "tool_result"
        && let Some(id @ Js::Str(i)) = row.get("toolUseId")
        && !i.is_empty()
    {
        put("toolUseId", Some(id.clone()));
    }
    if let Some(completed) = completed {
        put("turnCompleted", Some(Js::Bool(completed)));
    }
    if let Some(details) = details {
        put("turnUsageDetails", Some(Js::Obj(details)));
    }
    if let (Some(m), Some(user_money)) = (meta, user_money) {
        put("userTurnMoney", Some(Js::Obj(user_money)));
        if let Some(usd) = positive(m.get("userTurnCostUsd")) {
            put("userTurnCostUsd", Some(Js::Num(usd)));
        }
        let estimate = m.get("userTurnCostIsEstimate") == Some(&Js::Bool(true));
        put("userTurnCostIsEstimate", Some(Js::Bool(estimate)));
    }
    if let (Some(m), Some(cost)) = (meta, turn_money.filter(|c| money::amount_of(c) > 0.0)) {
        let usd = cost.get("currency") == Some(&Js::Str("USD".into()));
        let amount = money::amount_of(&cost);
        put("turnMoney", Some(Js::Obj(cost)));
        if usd {
            put("turnCostUsd", Some(Js::Num(amount)));
        }
        put(
            "turnCostIsEstimate",
            Some(Js::Bool(
                m.get("turnCostIsEstimate") == Some(&Js::Bool(true)),
            )),
        );
    }
    let mismatch = field("modelMismatch");
    let named = |key: &str| {
        mismatch
            .and_then(Js::obj)
            .and_then(|m| m.get(key))
            .and_then(Js::str)
            .is_some_and(|s| !s.is_empty())
    };
    if assistant && named("selected") && named("actual") {
        put("modelMismatch", Some(mismatch.expect("checked").clone()));
    }
    subagent(meta, &mut out);
    out.insert("isStreaming".into(), Js::Bool(false));
    Ok(out)
}
