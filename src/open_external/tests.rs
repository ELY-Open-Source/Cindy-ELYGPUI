use super::{escape, open, web};
use gpui::TestAppContext;
use std::cell::{Cell, RefCell};

thread_local! {
    static OPENED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static FAILS: Cell<bool> = const { Cell::new(false) };
}

pub fn opened(url: &str) -> bool {
    OPENED.with_borrow_mut(|opened| opened.push(url.to_owned()));
    !FAILS.get()
}

/// The addresses handed out on this thread since the last call.
pub fn take() -> Vec<String> {
    OPENED.take()
}

pub fn fail(fails: bool) {
    FAILS.set(fails);
}

#[test]
fn only_web_addresses_leave() {
    for ok in [
        "http://example.com",
        "https://example.com/a?b#c",
        "HTTPS://Example.com",
    ] {
        assert!(web(ok).is_some(), "{ok}");
    }
    let refused = [
        "",
        "example.com",
        "file:///etc/passwd",
        "mailto:someone@example.com",
        "javascript:alert(1)",
        "cindy://session/1",
        "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles",
        "ftp://example.com/",
        "https://",
    ];
    for bad in refused {
        assert!(web(bad).is_none(), "{bad}");
    }
}

#[test]
fn the_address_goes_out_as_the_parser_writes_it() {
    let url = web("https://EXAMPLE.com/a b?q=\"x\"").unwrap();
    assert_eq!(url.as_str(), "https://example.com/a%20b?q=%22x%22");
}

/// Chromium's `kExternalHandlerCharmap`, byte by byte.
#[test]
fn the_windows_escape_is_chromiums() {
    let kept = "azAZ09;/?:@&=+$,!'()*-._~#[]";
    assert_eq!(escape(kept), kept);
    assert_eq!(escape(" \"<>\\^`{|}"), "%20%22%3C%3E%5C%5E%60%7B%7C%7D");
    assert_eq!(escape("\u{1}\u{7f}é"), "%01%7F%C3%A9");
    // An escape stays; a bare percent sign does not.
    assert_eq!(escape("%2f%2F%zz%4"), "%2f%2F%25zz%254");
    assert_eq!(escape("100%"), "100%25");
}

#[gpui::test]
fn a_refused_address_starts_nothing(cx: &mut TestAppContext) {
    use futures::FutureExt as _;
    let (refused, file) = cx.update(|cx| (open("no address", cx), open("file:///etc/hosts", cx)));
    assert_eq!(refused.now_or_never(), Some(false));
    assert_eq!(file.now_or_never(), Some(false));
    assert_eq!(take(), Vec::<String>::new());
    let web = cx.update(|cx| open("https://Example.com/a b", cx));
    assert_eq!(web.now_or_never(), Some(true));
    assert_eq!(take(), ["https://example.com/a%20b"]);
}
