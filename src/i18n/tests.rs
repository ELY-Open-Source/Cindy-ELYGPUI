use super::{Arg, Translator};

#[test]
fn every_locale_loads() {
    for tag in crate::locale::SUPPORTED {
        Translator::new(tag).unwrap();
    }
}

#[test]
fn plurals_follow_each_locale() {
    let en = Translator::new("en").unwrap();
    let fr = Translator::new("fr").unwrap();
    let key = "mailReader.messageCount";
    assert_eq!(en.t(key, &[("count", Arg::Number(1))]), "1 email");
    assert_eq!(en.t(key, &[("count", Arg::Number(5))]), "5 emails");
    // A JavaScript Number picks its form and reads as `String(n)`.
    assert_eq!(en.t(key, &[("count", Arg::Js(1.0))]), "1 email");
    let big = 9_007_199_254_740_991.0 * 2.0 + 8.0;
    assert_eq!(
        en.t(key, &[("count", Arg::Js(big))]),
        "18014398509481990 emails"
    );
    let many = fr.lookup(key, Some(1_000_000_i64.into())).unwrap();
    assert!(std::ptr::eq(
        many,
        fr.lookup("mailReader.messageCount_many", None).unwrap()
    ));
}

#[test]
fn interpolation_defaults_numbers_and_nesting() {
    let en = Translator::new("en").unwrap();
    assert!(en.t("splash.tips.waking", &[]).contains("Cindy"));
    let es = Translator::new("es").unwrap();
    let text = es.t(
        "taskMigration.estimateTooManyFiles",
        &[("limit", Arg::Number(10_000))],
    );
    assert!(text.contains("10.000"), "{text}");
    let card = en.t("mcpApps.cardTitle", &[("title", Arg::Text("Maps"))]);
    assert_eq!(card, "Maps · MCP App");
}

#[test]
fn fallback_chain_and_error_keys() {
    let lzh = Translator::new("zh-Hant-x-lzh").unwrap();
    assert_eq!(
        lzh.chain.iter().map(|b| b.tag).collect::<Vec<_>>(),
        ["zh-Hant-x-lzh", "zh-TW", "en"]
    );
    let en = Translator::new("en").unwrap();
    let internal = en.t("ipcError.INTERNAL", &[]);
    assert_eq!(en.t("ipcError.NO_SUCH_CODE", &[]), internal);
    assert_eq!(en.t("chat.remoteError.NO_SUCH_CODE", &[]), internal);
    assert_eq!(en.t("no.such.key", &[]), "no.such.key");
}

#[test]
fn dev_write_blocked_overrides_catalog() {
    let en = Translator::new("en").unwrap();
    assert!(
        en.t("chatgptAuthRecovery.devWriteBlocked", &[])
            .contains("--isolated-auth")
    );
}
