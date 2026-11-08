//! `SessionInfoMeta.tsx`'s pieces and the task info preference of `sidebarFilterCore.ts`.

use gpui::App;
use serde_json::{Value, json};

use super::group::Session;
use crate::{
    cindy_json::{js_round, js_to_fixed, roundtrip},
    preferences::Preferences,
};

pub const TASK_INFO: &str = "cc-agent.sidebar.filter.taskInfo";
/// `TASK_INFO_VALUES`, in the menu's order.
pub const FIELDS: [&str; 5] = ["time", "pr", "worktree", "tokens", "cost"];

/// `loadTaskInfoFields`: a legacy array or `{ version: 1, fields }`, known values once each, in
/// the order they were ticked; none is allowed. Read once, as `getTaskInfoFields` caches it.
pub fn fields(prefs: &Preferences) -> Vec<&'static str> {
    let default = vec!["time"];
    let raw = match prefs.text(TASK_INFO) {
        Ok(Some(raw)) => raw,
        Ok(None) => return default,
        Err(error) => {
            log::warn!(target: "sessions", "failed to read taskInfo: {error:#}");
            return default;
        }
    };
    let values = match roundtrip(raw) {
        Ok(Value::Array(values)) => values,
        Ok(Value::Object(mut map)) if map.get("version") == Some(&json!(1)) => {
            match map.remove("fields") {
                Some(Value::Array(values)) => values,
                _ => return default,
            }
        }
        Ok(_) => return default,
        Err(error) => {
            log::warn!(target: "sessions", "failed to parse taskInfo JSON: {error:#}");
            return default;
        }
    };
    let mut out: Vec<&'static str> = Vec::new();
    for value in values.iter().filter_map(Value::as_str) {
        if let Some(field) = FIELDS.iter().find(|f| **f == value)
            && !out.contains(field)
        {
            out.push(field);
        }
    }
    out
}

/// `nextTaskInfoAfterToggle`, then `persistTaskInfoFields`.
pub fn toggle(field: &'static str, cx: &mut App) {
    let mut next = cx.global::<super::Sessions>().task_info.clone();
    match next.iter().position(|f| *f == field) {
        Some(at) => {
            next.remove(at);
        }
        None => next.push(field),
    }
    // `JSON.stringify({ version: 1, fields })`, in that key order.
    let list = serde_json::to_string(&next).expect("strings serialize");
    let value = format!(r#"{{"version":1,"fields":{list}}}"#);
    cx.global_mut::<super::Sessions>().task_info = next;
    super::filter::set(TASK_INFO, &value, cx);
}

/// `formatCompactTokens`.
pub fn compact_tokens(n: i64) -> String {
    if n < 1_000 {
        return n.to_string();
    }
    let units = [(1e3, "k"), (1e6, "M"), (1e9, "B")];
    for (divisor, suffix) in units {
        let scaled = n as f64 / divisor;
        if scaled < 999.95 {
            return format!("{}{suffix}", js_to_fixed(scaled, 1));
        }
    }
    format!("{}B", js_to_fixed(n as f64 / 1e9, 1))
}

/// `sessionToCamel`'s `totalMoney`: this build's total, joined with the legacy USD total
/// when both are USD.
pub fn money(s: &Session) -> Option<(&'static str, f64)> {
    let legacy = (s.cost_usd > 0.0).then_some(("USD", s.cost_usd));
    // `normalizeRegionalMoney` takes finite amounts only.
    let spent = s.cost_amount > 0.0 && s.cost_amount.is_finite();
    let current = match s.cost_currency.as_deref() {
        Some("USD") if spent => Some(("USD", s.cost_amount)),
        Some("CNY") if spent => Some(("CNY", s.cost_amount)),
        _ => None,
    };
    match (legacy, current) {
        (Some((a, x)), Some((b, y))) if a == b => Some((a, x + y)),
        (_, Some(current)) => Some(current),
        (legacy, None) => legacy,
    }
}

/// `formatMoney`: two decimals under 10, then whole units, then thousands.
pub fn format_money((currency, amount): (&str, f64)) -> String {
    let prefix = if currency == "CNY" { "¥" } else { "$" };
    if amount < 10.0 {
        format!("{prefix}{}", js_to_fixed(amount, 2))
    } else if amount >= 1000.0 {
        format!("{prefix}{}k", js_to_fixed(amount / 1000.0, 1))
    } else {
        format!("{prefix}{}", js_round(amount))
    }
}

/// One piece of the slot: its field, text and tip.
pub struct Piece {
    pub field: &'static str,
    pub text: String,
    pub tip: String,
}

/// `buildSessionInfoPieces` without `pr` and `worktree`, which come with chapter 18.
pub fn pieces(
    s: &Session,
    fields: &[&'static str],
    time: impl Fn(i64) -> (String, String),
    t: impl Fn(&str) -> String,
) -> Vec<Piece> {
    fields
        .iter()
        .filter_map(|&field| {
            let (text, tip) = match field {
                "tokens" if s.tokens > 0 => (
                    compact_tokens(s.tokens),
                    t("ccAgent.sidebar.taskInfoTip.tokens"),
                ),
                "cost" => (
                    format_money(money(s)?),
                    t("ccAgent.sidebar.taskInfoTip.cost"),
                ),
                "time" if s.activity_at > 0 => time(s.activity_at),
                _ => return None,
            };
            Some(Piece { field, text, tip })
        })
        .collect()
}
