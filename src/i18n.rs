#![cfg_attr(
    not(test),
    expect(dead_code, reason = "the first UI text lands in 6.5")
)]

mod catalogs;

use std::{cell::RefCell, collections::HashSet};

use anyhow::{Context as _, Result, bail};
use icu_decimal::{DecimalFormatter, input::Decimal};
use icu_locale_core::Locale;
use icu_plurals::{PluralCategory, PluralRules};
use serde_json::Value;

use crate::locale;
use catalogs::{CATALOGS, DEV_WRITE_BLOCKED};

const DEFAULTS: [(&str, &str); 3] = [
    ("appName", "Cindy"),
    ("pronoun", "这位伙伴"),
    ("reservedGhostIdPrefixes", "cindy-"),
];
const INTERNAL: &str = "ipcError.INTERNAL";

/// A value passed into a message.
#[derive(Clone, Copy)]
pub enum Arg<'a> {
    Text(&'a str),
    Number(i64),
}

struct Bundle {
    tag: &'static str,
    common: Value,
    ai_rename: Value,
    plurals: PluralRules,
}

/// Cindy's i18next setup: lookups, plurals, interpolation and fallback.
pub struct Translator {
    chain: Vec<Bundle>,
    numbers: DecimalFormatter,
    missing: RefCell<HashSet<String>>,
}

fn icu(tag: &str) -> Result<Locale> {
    Locale::try_from_str(tag).with_context(|| format!("bad locale tag {tag}"))
}

fn bundle(tag: &'static str) -> Result<Bundle> {
    let (_, common, ai_rename) = CATALOGS
        .iter()
        .find(|(t, ..)| *t == tag)
        .with_context(|| format!("no catalog for {tag}"))?;
    let mut common: Value =
        serde_json::from_str(common).with_context(|| format!("{tag}/common.json"))?;
    if let Some((_, text)) = DEV_WRITE_BLOCKED.iter().find(|(t, _)| *t == tag) {
        let recovery = common
            .as_object_mut()
            .context("common.json is not an object")?
            .entry("chatgptAuthRecovery")
            .or_insert_with(|| Value::Object(Default::default()));
        let Value::Object(recovery) = recovery else {
            bail!("{tag} chatgptAuthRecovery is not an object");
        };
        recovery.insert("devWriteBlocked".into(), Value::String((*text).into()));
    }
    Ok(Bundle {
        tag,
        common,
        ai_rename: serde_json::from_str(ai_rename)
            .with_context(|| format!("{tag}/aiRename.json"))?,
        plurals: PluralRules::try_new_cardinal(icu(tag)?.into())
            .with_context(|| format!("plural rules for {tag}"))?,
    })
}

fn suffix(category: PluralCategory) -> &'static str {
    match category {
        PluralCategory::Zero => "zero",
        PluralCategory::One => "one",
        PluralCategory::Two => "two",
        PluralCategory::Few => "few",
        PluralCategory::Many => "many",
        PluralCategory::Other => "other",
    }
}

fn walk<'a>(root: &'a Value, key: &str) -> Option<&'a Value> {
    key.split('.').try_fold(root, |node, part| node.get(part))
}

impl Translator {
    /// Fallback as Cindy's `fallbackLng`: classical to modern Chinese, then English.
    pub fn new(tag: &'static str) -> Result<Self> {
        let mut tags = vec![tag];
        match tag {
            "zh-Hant-x-lzh" => tags.push("zh-TW"),
            "zh-Hans-x-lzh" => tags.push("zh-CN"),
            _ => {}
        }
        if tag != locale::DEFAULT {
            tags.push(locale::DEFAULT);
        }
        Ok(Self {
            chain: tags.into_iter().map(bundle).collect::<Result<_>>()?,
            numbers: DecimalFormatter::try_new(icu(tag)?.into(), Default::default())
                .with_context(|| format!("number format for {tag}"))?,
            missing: RefCell::default(),
        })
    }

    pub fn tag(&self) -> &'static str {
        self.chain[0].tag
    }

    /// Nearest of Ely's locales, for Ely's own component text.
    pub fn ely_tag(&self) -> &'static str {
        match self.tag() {
            "zh-CN" | "zh-TW" | "zh-Hans-x-lzh" | "zh-Hant-x-lzh" => "zh-CN",
            "ja" => "ja-JP",
            "ko" => "ko-KR",
            "es" => "es-ES",
            "fr" => "fr-FR",
            "de" => "de-DE",
            _ => "en-US",
        }
    }

    fn lookup(&self, key: &str, count: Option<i64>) -> Option<&str> {
        let (ns, path) = key.split_once(':').unwrap_or(("common", key));
        self.chain.iter().find_map(|bundle| {
            let root = match ns {
                "common" => &bundle.common,
                "aiRename" => &bundle.ai_rename,
                _ => return None,
            };
            let plural =
                count.map(|n| format!("{path}_{}", suffix(bundle.plurals.category_for(n))));
            plural
                .iter()
                .map(String::as_str)
                .chain([path])
                .find_map(|k| walk(root, k)?.as_str())
        })
    }

    fn arg<'a>(args: &[(&str, Arg<'a>)], name: &str) -> Option<Arg<'a>> {
        args.iter().find(|(n, _)| *n == name).map(|(_, a)| *a)
    }

    fn interpolate(&self, key: &str, message: &str, args: &[(&str, Arg)]) -> String {
        let mut out = String::with_capacity(message.len());
        let mut rest = message;
        while let Some(start) = rest.find("{{") {
            let Some(len) = rest[start..].find("}}") else {
                break;
            };
            out.push_str(&rest[..start]);
            let inner = &rest[start + 2..start + len];
            let (name, format) = match inner.split_once(',') {
                Some((name, format)) => (name.trim(), Some(format.trim())),
                None => (inner.trim(), None),
            };
            match (Self::arg(args, name), format) {
                (Some(Arg::Number(n)), Some("number")) => {
                    out.push_str(&self.numbers.format(&Decimal::from(n)).to_string());
                }
                (Some(Arg::Number(n)), None) => out.push_str(&n.to_string()),
                (Some(Arg::Text(text)), None) => out.push_str(text),
                (None, None) => match DEFAULTS.iter().find(|(n, _)| *n == name) {
                    Some((_, text)) => out.push_str(text),
                    None => log::warn!(target: "i18n", "{key}: no value for {{{{{name}}}}}"),
                },
                (_, Some(format)) => {
                    log::warn!(target: "i18n", "{key}: cannot format {name} as {format}");
                }
            }
            rest = &rest[start + len + 2..];
        }
        out.push_str(rest);
        out
    }

    fn nest(&self, text: String, args: &[(&str, Arg)]) -> String {
        let Some(start) = text.find("$t(") else {
            return text;
        };
        let Some(len) = text[start..].find(')') else {
            return text;
        };
        let inner = self.t(&text[start + 3..start + len], args);
        let rest = self.nest(text[start + len + 1..].to_owned(), args);
        format!("{}{inner}{rest}", &text[..start])
    }

    /// The message for `key`; `count` picks the plural form.
    pub fn t(&self, key: &str, args: &[(&str, Arg)]) -> String {
        let count = match Self::arg(args, "count") {
            Some(Arg::Number(n)) => Some(n),
            _ => None,
        };
        let message = self.lookup(key, count).or_else(|| {
            if self.missing.borrow_mut().insert(key.to_owned()) {
                log::warn!(target: "i18n", "missing {key} in {}", self.tag());
            }
            let errors = key.starts_with("ipcError.") || key.starts_with("chat.remoteError.");
            (errors && key != INTERNAL)
                .then(|| self.lookup(INTERNAL, None))
                .flatten()
        });
        match message {
            Some(message) => self.nest(self.interpolate(key, message, args), args),
            None => key.to_owned(),
        }
    }
}

impl gpui::Global for Translator {}

#[cfg(test)]
mod tests;
