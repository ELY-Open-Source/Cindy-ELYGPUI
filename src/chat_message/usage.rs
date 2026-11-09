//! `normalizeTurnUsageDetails` with `usageCostDetails.ts`'s billing source and row grouping.

use super::money;
use crate::{
    rich_answer::js::{self, Js, Obj},
    theme_import::color::js_trim,
};

/// `normalizeUsageBillingSource`, into `out`.
fn billing_source(value: Option<&Js>, out: &mut Obj) -> Result<(), String> {
    let Some(raw) = value.and_then(Js::obj) else {
        return Ok(());
    };
    for key in ["providerId", "providerName"] {
        if let Some(v @ Js::Str(_)) = raw.get(key) {
            out.insert(key.into(), v.clone());
        }
    }
    let source = js::try_string(raw.get("billingSource"))?;
    if ["subscription", "cindy-ai", "provider", "unknown"].contains(&source.as_str()) {
        out.insert("billingSource".into(), raw["billingSource"].clone());
    }
    Ok(())
}

/// `sanitizeToken`.
fn token(value: Option<&Js>) -> f64 {
    match value {
        Some(Js::Num(n)) if n.is_finite() && *n > 0.0 => n.floor(),
        _ => 0.0,
    }
}

fn duration(value: Option<&Js>) -> Option<f64> {
    match value {
        Some(Js::Num(n)) if n.is_finite() && *n > 0.0 => Some(*n),
        _ => None,
    }
}

/// `sanitizeModel`.
fn model(value: Option<&Js>) -> Option<&str> {
    let text = js_trim(value?.str()?);
    (!text.is_empty()).then_some(text)
}

/// `sanitizePerModelCost`, grouped as `groupUsageCostRows` groups conversation rows.
fn per_model_cost(list: &[Js]) -> Result<Option<Vec<Js>>, String> {
    let mut groups: Vec<(Vec<Option<Js>>, Obj)> = Vec::new();
    for item in list {
        let Some(raw) = item.obj() else {
            continue;
        };
        let Some(name) = model(raw.get("model")) else {
            continue;
        };
        let legacy = match raw.get("costUsd") {
            Some(Js::Num(n)) if n.is_finite() && *n > 0.0 => {
                Some(money::legacy_usd(*n).expect("a finite amount"))
            }
            _ => None,
        };
        let Some(cost) = money::normalize(raw.get("money")).or(legacy) else {
            continue;
        };
        if money::amount_of(&cost) <= 0.0 {
            continue;
        }
        let mut row = Obj::new();
        row.insert("model".into(), Js::Str(name.into()));
        billing_source(Some(item), &mut row)?;
        let key = vec![
            Some(Js::Str(name.into())),
            row.get("providerId").cloned(),
            row.get("providerName").cloned(),
            row.get("billingSource").cloned(),
            cost.get("currency").cloned(),
            cost.get("kind").cloned(),
            cost.get("approximate").cloned(),
        ];
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, current)) => {
                let Some(Js::Obj(sum)) = current.get_mut("money") else {
                    unreachable!("a grouped money");
                };
                let total = money::amount_of(sum) + money::amount_of(&cost);
                sum.insert("amount".into(), Js::Num(total));
            }
            None => {
                row.shift_insert(1, "money".into(), Js::Obj(cost));
                groups.push((key, row));
            }
        }
    }
    Ok((!groups.is_empty()).then(|| groups.into_iter().map(|(_, row)| Js::Obj(row)).collect()))
}

/// `normalizeTurnUsageDetails`.
pub fn normalize(value: Option<&Js>) -> Result<Option<Obj>, String> {
    let Some(raw) = value.and_then(Js::obj) else {
        return Ok(None);
    };
    // Cindy reads every billing source before the early return.
    billing_source(value, &mut Obj::new())?;
    for item in raw
        .get("perModelCost")
        .and_then(Js::arr)
        .unwrap_or_default()
    {
        billing_source(Some(item), &mut Obj::new())?;
    }
    let number = |key: &str| raw.get(key).filter(|v| matches!(v, Js::Num(_)));
    let input = token(number("inputTokens"));
    let output = token(number("outputTokens"));
    let read = token(number("cacheReadTokens"));
    let create = token(number("cacheCreateTokens"));
    let total = input + output + read + create;
    let duration_ms = duration(number("durationMs"));
    let turn_duration = duration(number("turnDurationMs"));
    let additional: Option<Vec<Js>> = raw.get("additionalCosts").and_then(Js::arr).map(|list| {
        list.iter()
            .filter_map(|m| money::normalize(Some(m)).map(Js::Obj))
            .collect()
    });
    let additional = additional.filter(|a| !a.is_empty());
    if total <= 0.0 && turn_duration.is_none() && additional.is_none() {
        return Ok(None);
    }
    let mut out = Obj::new();
    billing_source(value, &mut out)?;
    if let Some(additional) = additional {
        out.insert("additionalCosts".into(), Js::Arr(additional));
    }
    for (key, n) in [
        ("inputTokens", input),
        ("outputTokens", output),
        ("cacheReadTokens", read),
        ("cacheCreateTokens", create),
        ("totalTokens", total),
    ] {
        out.insert(key.into(), Js::Num(n));
    }
    let denominator = input + read + create;
    let rate = match denominator > 0.0 {
        true => Js::Num(read / denominator),
        false => Js::Null,
    };
    out.insert("cacheHitRate".into(), rate);
    if let Some(d) = duration_ms {
        out.insert("durationMs".into(), Js::Num(d));
    }
    if let Some(d) = turn_duration {
        out.insert("turnDurationMs".into(), Js::Num(d));
    }
    if let Some(name) = model(raw.get("model")) {
        out.insert("model".into(), Js::Str(name.into()));
    }
    let mut models: Vec<Js> = Vec::new();
    for name in raw.get("models").and_then(Js::arr).unwrap_or_default() {
        if let Some(name) = model(Some(name)).map(|m| Js::Str(m.into()))
            && !models.contains(&name)
        {
            models.push(name);
        }
    }
    if !models.is_empty() {
        out.insert("models".into(), Js::Arr(models));
    }
    let rows = raw
        .get("perModelCost")
        .and_then(Js::arr)
        .map(per_model_cost);
    if let Some(rows) = rows.transpose()?.flatten() {
        out.insert("perModelCost".into(), Js::Arr(rows));
    }
    if raw.get("firstUserRound") == Some(&Js::Bool(true)) {
        out.insert("firstUserRound".into(), Js::Bool(true));
    }
    Ok(Some(out))
}
