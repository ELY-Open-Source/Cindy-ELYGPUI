use icu_locale_core::Locale;

pub const SUPPORTED: [&str; 12] = [
    "zh-CN",
    "zh-TW",
    "en",
    "ja",
    "ko",
    "es",
    "fr",
    "de",
    "pt-PT",
    "pt-BR",
    "zh-Hant-x-lzh",
    "zh-Hans-x-lzh",
];
pub const DEFAULT: &str = "en";

/// Cindy's `normalizeLocaleTag`: `_` to `-`, charset cut, BCP 47 parse.
fn parse(raw: &str) -> Option<Locale> {
    let tag = raw.trim().replace('_', "-");
    let tag = tag.split('.').next()?;
    if tag.is_empty() {
        return None;
    }
    Locale::try_from_str(tag).ok()
}

fn exact(locale: &Locale) -> Option<&'static str> {
    let tag = locale.to_string().to_lowercase();
    SUPPORTED.into_iter().find(|s| s.to_lowercase() == tag)
}

fn match_supported(locale: &Locale) -> Option<&'static str> {
    if let Some(found) = exact(locale) {
        return Some(found);
    }
    let language = locale.id.language.as_str();
    if language == "pt" {
        let brazil = locale.id.region.is_some_and(|r| r.as_str() == "BR");
        return Some(if brazil { "pt-BR" } else { "pt-PT" });
    }
    let mut same = SUPPORTED
        .into_iter()
        .filter(|s| s.split('-').next() == Some(language));
    match (same.next(), same.next()) {
        (Some(only), None) => Some(only),
        _ => None,
    }
}

fn match_system(raw: &str) -> Option<&'static str> {
    let locale = parse(raw)?;
    if let Some(found) = exact(&locale) {
        return Some(found);
    }
    if locale.id.language.as_str() != "zh" {
        return match_supported(&locale);
    }
    let script = locale.id.script.map(|s| s.as_str().to_lowercase());
    let region = locale.id.region.map(|r| r.as_str().to_lowercase());
    Some(match (script.as_deref(), region.as_deref()) {
        (Some("hans"), _) => "zh-CN",
        (Some("hant"), _) => "zh-TW",
        (_, Some("tw" | "hk" | "mo")) => "zh-TW",
        _ => "zh-CN",
    })
}

/// Cindy's `resolvePreferredSystemLocale`.
pub fn resolve_preferred_system_locale<'a>(
    raws: impl IntoIterator<Item = &'a str>,
) -> &'static str {
    raws.into_iter().find_map(match_system).unwrap_or(DEFAULT)
}

/// The locale shown: the saved `language` choice, else the OS's.
pub fn effective(preference: Option<&str>) -> &'static str {
    match preference {
        Some(tag) if tag != "system" => match SUPPORTED.into_iter().find(|s| *s == tag) {
            Some(found) => found,
            None => {
                log::warn!(target: "locale", "ignored unknown language preference {tag:?}");
                system()
            }
        },
        _ => system(),
    }
}

fn system() -> &'static str {
    let os: Vec<String> = sys_locale::get_locales().collect();
    let locale = resolve_preferred_system_locale(os.iter().map(String::as_str));
    log::info!(target: "locale", "system languages {os:?} resolve to {locale}");
    locale
}

#[cfg(test)]
mod tests {
    use super::resolve_preferred_system_locale as resolve;

    #[test]
    fn matches_cindy_system_rules() {
        let cases = [
            (vec!["zh_CN.UTF-8"], "zh-CN"),
            (vec!["zh-Hant-HK"], "zh-TW"),
            (vec!["zh-HK"], "zh-TW"),
            (vec!["zh-SG"], "zh-CN"),
            (vec!["zh"], "zh-CN"),
            (vec!["pt-BR"], "pt-BR"),
            (vec!["pt"], "pt-PT"),
            (vec!["en-US"], "en"),
            (vec!["ja_JP.UTF-8"], "ja"),
            (vec!["C", "fr_FR"], "fr"),
            (vec!["sv-SE", "de-AT"], "de"),
            (vec!["sv-SE"], "en"),
            (vec![], "en"),
        ];
        for (raws, want) in cases {
            assert_eq!(resolve(raws.iter().copied()), want, "{raws:?}");
        }
    }

    #[test]
    fn saved_choice_wins_over_system() {
        assert_eq!(super::effective(Some("ko")), "ko");
        assert_eq!(super::effective(Some("zh-Hant-x-lzh")), "zh-Hant-x-lzh");
    }
}
