//! `SessionHandoffCard`'s words (title, badge, last-active line, short id) and its pill's.

use crate::{
    deep_link::{Card, Wake},
    i18n::{Arg, Translator},
    js_date,
    theme_import::color::js_trim,
};

const MINUTE: f64 = 60_000.0;
const HOUR: f64 = 60.0 * MINUTE;
const DAY: f64 = 24.0 * HOUR;

fn word(t: &Translator, key: &str, args: &[(&str, Arg)]) -> String {
    t.t(&format!("ccAgent.handoff.card.{key}"), args)
}

/// `shortSessionId`, in UTF-16 units; half a pair reads as U+FFFD.
pub fn short(id: &str) -> String {
    let units: Vec<u16> = id.encode_utf16().collect();
    if units.len() <= 13 {
        return id.to_owned();
    }
    let part = String::from_utf16_lossy;
    let (head, tail) = (&units[..8], &units[units.len() - 4..]);
    format!("{}\u{2026}{}", part(head), part(tail))
}

/// `displayTitle`.
pub fn title(card: &Card, t: &Translator) -> String {
    let own = card.title.as_deref().map(js_trim);
    match own.filter(|title| !title.is_empty()) {
        Some(title) => title.to_owned(),
        None => word(t, "unnamedSession", &[]),
    }
}

/// The badge's label, and whether it is the solid one.
pub fn badge(wake: Wake, t: &Translator) -> (String, bool) {
    let key = match wake {
        Wake::Created => "wake.created",
        Wake::Resumed => "wake.resumed",
        Wake::AlreadyActive => "wake.alreadyActive",
    };
    (word(t, key, &[]), wake != Wake::AlreadyActive)
}

pub fn never(t: &Translator) -> String {
    word(t, "neverActive", &[])
}

/// `formatLastActive` at `now`, `Date.now()`.
pub fn last_active(value: &str, now: f64, t: &Translator) -> String {
    let at = js_date::parse(value);
    if at.is_nan() {
        return word(t, "lastActive.recently", &[]);
    }
    let delta = (now - at).max(0.0);
    let (key, unit) = match delta {
        delta if delta < MINUTE => return word(t, "lastActive.justNow", &[]),
        delta if delta < HOUR => ("lastActive.minutes", MINUTE),
        delta if delta < DAY => ("lastActive.hours", HOUR),
        _ => ("lastActive.days", DAY),
    };
    word(t, key, &[("count", Arg::Js((delta / unit).floor()))])
}

/// `HandoffSourcePill`'s line: the dispatcher's title, else its short id.
pub fn pill(dispatcher: &str, title: Option<&str>, t: &Translator) -> String {
    let own = title.map(js_trim).filter(|title| !title.is_empty());
    let label = own.map_or_else(|| short(dispatcher), str::to_owned);
    let args = [("title", Arg::Text(&label))];
    t.t("ccAgent.handoff.pill.fromDispatcher", &args)
}

pub fn dismiss(t: &Translator) -> String {
    t.t("ccAgent.handoff.pill.dismissAria", &[])
}

#[cfg(test)]
mod tests {
    use super::{badge, last_active, short, title};
    use crate::{
        deep_link::{Wake, parse_card},
        i18n::Translator,
    };

    #[test]
    fn a_long_id_keeps_its_ends() {
        assert_eq!(short("1234567890123"), "1234567890123");
        assert_eq!(
            short("83639512-aaaa-bbbb-cccc-00007ed0"),
            "83639512\u{2026}7ed0"
        );
        // The cut falls in UTF-16 units, so it may halve a pair.
        let pairs = "\u{1f600}".repeat(7);
        assert_eq!(short(&pairs).chars().count(), 4 + 1 + 2);
        let odd = format!("a{pairs}");
        assert_eq!(
            short(&odd),
            "a\u{1f600}\u{1f600}\u{1f600}\u{fffd}\u{2026}\u{1f600}\u{1f600}"
        );
    }

    #[test]
    fn the_card_says_its_title_and_wake() {
        let t = Translator::new("en").unwrap();
        let card = |query: &str| parse_card(&format!("cindy://session-card/s?{query}")).unwrap();
        let named = card("wake=created&title=%20Plan%20&last_active=null");
        assert_eq!(title(&named, &t), "Plan");
        let bare = card("wake=resumed&title=&last_active=null");
        assert_eq!(title(&bare, &t), "Untitled session");
        assert_eq!(badge(Wake::Created, &t), ("NEW".into(), true));
        assert_eq!(badge(Wake::Resumed, &t), ("RESUMED".into(), true));
        assert_eq!(
            badge(Wake::AlreadyActive, &t),
            ("ALREADY ACTIVE".into(), false)
        );
    }

    #[test]
    fn last_active_counts_whole_units() {
        let t = Translator::new("en").unwrap();
        let at = "2026-01-02T03:04:05.000Z";
        let base = crate::js_date::parse(at);
        let after = |ms: f64| last_active(at, base + ms, &t);
        assert_eq!(after(-5.0), "last active just now");
        assert_eq!(after(59_999.0), "last active just now");
        assert_eq!(after(60_000.0), "last active 1 minute ago");
        assert_eq!(after(3_599_999.0), "last active 59 minutes ago");
        assert_eq!(after(3_600_000.0), "last active 1 hour ago");
        assert_eq!(after(86_399_999.0), "last active 23 hours ago");
        assert_eq!(after(86_400_000.0), "last active 1 day ago");
        assert_eq!(after(86_400_000.0 * 400.0), "last active 400 days ago");
        assert_eq!(last_active("soon", base, &t), "last active recently");
        assert_eq!(last_active("", base, &t), "last active recently");
    }
}
