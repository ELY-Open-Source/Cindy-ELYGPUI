//! `regionalMoney.ts`'s readers and `codexSubscriptionValue.ts`'s stale estimate repair.

use std::sync::LazyLock;

use regex::Regex;

use crate::{
    rich_answer::js::{Js, Obj},
    theme_import::color::{JS_SPACE, js_trim},
};

const REASONS: [&str; 5] = [
    "fixed-fx",
    "legacy-usd",
    "subscription-value",
    "reference-price",
    "inferred-currency",
];

fn amount(value: Option<&Js>) -> Option<f64> {
    match value {
        Some(Js::Num(n)) if n.is_finite() && *n >= 0.0 => Some(*n),
        _ => None,
    }
}

/// `normalizeRegionalMoney`.
pub fn normalize(value: Option<&Js>) -> Option<Obj> {
    let raw = value?.obj()?;
    let amount = amount(raw.get("amount"))?;
    let currency = raw
        .get("currency")?
        .str()
        .filter(|c| matches!(*c, "CNY" | "USD"))?;
    let approximate = match raw.get("approximate")? {
        Js::Bool(b) => *b,
        _ => return None,
    };
    let kind = raw
        .get("kind")?
        .str()
        .filter(|k| matches!(*k, "actual-cost" | "value-estimate"))?;
    let mut out = Obj::new();
    out.insert("amount".into(), Js::Num(amount));
    out.insert("currency".into(), Js::Str(currency.into()));
    out.insert("approximate".into(), Js::Bool(approximate));
    out.insert("kind".into(), Js::Str(kind.into()));
    if let Some(Js::Arr(reasons)) = raw.get("estimateReasons") {
        let mut unique: Vec<Js> = Vec::new();
        for reason in reasons {
            if reason.str().is_some_and(|r| REASONS.contains(&r)) && !unique.contains(reason) {
                unique.push(reason.clone());
            }
        }
        if !unique.is_empty() {
            out.insert("estimateReasons".into(), Js::Arr(unique));
        }
    }
    Some(out)
}

/// `legacyUsdMoney`: an actual USD cost; `usdMoney` throws on an amount that is not finite.
pub fn legacy_usd(amount: f64) -> Result<Obj, String> {
    if !amount.is_finite() || amount < 0.0 {
        let shown = crate::cindy_json::js_number(amount);
        return Err(format!("invalid non-negative money amount: {shown}"));
    }
    let mut out = Obj::new();
    out.insert("amount".into(), Js::Num(amount));
    out.insert("currency".into(), Js::Str("USD".into()));
    out.insert("approximate".into(), Js::Bool(false));
    out.insert("kind".into(), Js::Str("actual-cost".into()));
    Ok(out)
}

pub fn amount_of(money: &Obj) -> f64 {
    match money.get("amount") {
        Some(Js::Num(n)) => *n,
        _ => unreachable!("a normalized amount"),
    }
}

/// USD per million tokens: input, output, cache read, cache write.
type Price = (f64, f64, Option<f64>, Option<f64>);

fn persisted_price(model: &str) -> Option<Price> {
    Some(match model {
        "gpt-5.6-sol" => (5.0, 30.0, Some(0.5), Some(6.25)),
        "gpt-5.6-terra" => (2.5, 15.0, Some(0.25), Some(3.125)),
        "gpt-5.6-luna" => (1.0, 6.0, Some(0.1), Some(1.25)),
        "gpt-5.5" => (5.0, 30.0, Some(0.5), None),
        "gpt-5.4" => (2.5, 15.0, Some(0.25), None),
        "gpt-5.4-mini" => (0.75, 4.5, Some(0.075), None),
        _ => return None,
    })
}

fn legacy_price(model: &str) -> Option<Price> {
    Some(match model {
        "gpt-5.5" | "gpt-5.4" => (2.0, 8.0, None, None),
        "gpt-5.4-mini" => (0.75, 4.5, None, None),
        _ => return None,
    })
}

static VARIANT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"\[[^\]]*\]{JS_SPACE}*$")).expect("valid pattern"));

/// `normalizeCodexSubscriptionValueModelId`; `trim` throws on a model that is not a string.
fn model_id(model: Option<&Js>) -> Result<String, String> {
    let text = match model {
        None | Some(Js::Null) => "",
        Some(Js::Str(s)) => s,
        Some(_) => return Err("(model ?? \"\").trim is not a function".into()),
    };
    let trimmed = js_trim(text);
    if trimmed.is_empty() {
        return Ok("unknown".into());
    }
    let stripped = VARIANT.replace(trimmed, "");
    Ok(match js_trim(&stripped) {
        "" => "unknown".into(),
        id => id.into(),
    })
}

fn tokens(details: &Obj, key: &str) -> f64 {
    match details.get(key) {
        Some(Js::Num(n)) if *n != 0.0 && !n.is_nan() => n.max(0.0),
        _ => 0.0,
    }
}

/// `resolveStaleCodexSubscriptionValueEstimate`: the corrected value of an estimate that priced cache as input.
pub fn stale_codex_estimate(
    raw: f64,
    details: Option<&Obj>,
    model: Option<&Js>,
) -> Result<Option<f64>, String> {
    let Some(details) = details.filter(|_| raw.is_finite() && raw > 0.0) else {
        return Ok(None);
    };
    let from_details = model_id(details.get("model"))?;
    let model = match from_details.as_str() {
        "unknown" => model_id(model)?,
        _ => from_details,
    };
    Ok(repaired(raw, details, &model))
}

fn repaired(raw: f64, details: &Obj, model: &str) -> Option<f64> {
    let price = persisted_price(model)?;
    let input = tokens(details, "inputTokens");
    let output = tokens(details, "outputTokens");
    let read = tokens(details, "cacheReadTokens");
    let create = tokens(details, "cacheCreateTokens");
    if read + create <= 0.0 || input + output + read + create <= 0.0 {
        return None;
    }
    let corrected = (input * price.0
        + output * price.1
        + read * price.2.unwrap_or(price.0)
        + create * price.3.unwrap_or(price.0))
        / 1e6;
    if !corrected.is_finite() || corrected <= 0.0 {
        return None;
    }
    for candidate in [Some(price), legacy_price(model)].into_iter().flatten() {
        let old = (input * candidate.0
            + output * candidate.1
            + read * candidate.0
            + create * candidate.0)
            / 1e6;
        if !old.is_finite() || old <= corrected {
            continue;
        }
        let tolerance = (old * 0.001).max(0.000001);
        if (raw - old).abs() <= tolerance {
            return Some(corrected);
        }
    }
    None
}
