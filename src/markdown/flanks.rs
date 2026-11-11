//! micromark-extension-cjk-friendly's `tokenizeAttention`: whether a sequence opens and closes.

use std::sync::LazyLock;

use serde::Deserialize;

use crate::v8_classes::within;

const WHITESPACE: u16 = 1;
const PUNCTUATION: u16 = 2;
const CJK: u16 = 4096;
const CJK_PUNCTUATION: u16 = CJK | PUNCTUATION;
const IVS: u16 = 8192;
const SELECTOR: u16 = 16384;

#[derive(Deserialize)]
struct Classes {
    whitespace: Vec<[u32; 2]>,
    selector: Vec<[u32; 2]>,
    ivs: Vec<[u32; 2]>,
    cjk: Vec<[u32; 2]>,
    punctuation: Vec<[u32; 2]>,
}

fn classes() -> &'static Classes {
    static CLASSES: LazyLock<Classes> = LazyLock::new(|| {
        let text = include_str!("../../resources/attention-classes.json");
        serde_json::from_str(text).expect("the bundled character classes parse")
    });
    &CLASSES
}

/// micromark's `unicodeWhitespace`: JavaScript's `\s`.
pub fn space(ch: char) -> bool {
    within(&classes().whitespace, ch)
}

/// micromark's `unicodePunctuation`: `\p{P}` or `\p{S}`.
pub fn punctuation(ch: char) -> bool {
    within(&classes().punctuation, ch)
}

/// `classifyCharacter`; the end of the stream reads as whitespace.
fn classify(ch: Option<char>) -> u16 {
    let Some(ch) = ch else { return WHITESPACE };
    let classes = classes();
    if within(&classes.whitespace, ch) {
        return WHITESPACE;
    }
    if within(&classes.selector, ch) {
        return SELECTOR;
    }
    if within(&classes.ivs, ch) {
        return IVS;
    }
    let cjk = if within(&classes.cjk, ch) { CJK } else { 0 };
    let punctuation = if within(&classes.punctuation, ch) {
        PUNCTUATION
    } else {
        0
    };
    cjk | punctuation
}

/// `isCjkAmbiguousPunctuation`: a quotation mark its selector makes fullwidth.
fn ambiguous(main: char, selector: char) -> bool {
    selector == '\u{FE01}' && matches!(main, '\u{2018}' | '\u{2019}' | '\u{201C}' | '\u{201D}')
}

fn text(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).expect("the source splits at characters")
}

/// `classifyPrecedingCharacter`: a selector reads the character before it inside its chunk.
fn preceding(bytes: &[u8], nuls: &[usize], slice: usize, start: usize) -> u16 {
    let mut back = text(&bytes[slice..start]).char_indices().rev();
    let previous = back.next();
    let before = classify(previous.map(|(_, ch)| ch));
    let (Some((_, selector)), Some((at, main))) = (previous, back.next()) else {
        return before;
    };
    let two = classify(Some(main));
    if before != SELECTOR || two & WHITESPACE != 0 || nuls.binary_search(&(slice + at)).is_ok() {
        before
    } else if ambiguous(main, selector) {
        CJK_PUNCTUATION
    } else {
        two & !IVS
    }
}

/// `(open, close)` for the sequence at `at`, in the `slice` its tokenizer was fed.
pub fn judge(
    bytes: &[u8],
    nuls: &[usize],
    slice: (usize, usize),
    at: (usize, usize),
) -> (bool, bool) {
    let previous = text(&bytes[slice.0..at.0]).chars().next_back();
    let next = text(&bytes[at.1..slice.1]).chars().next();
    let before = preceding(bytes, nuls, slice.0, at.0);
    let after = classify(next);
    let plain = |class: u16| class & CJK_PUNCTUATION == PUNCTUATION;
    let space = |class: u16| class & WHITESPACE != 0;
    let before_edge = plain(before) || space(before);
    let after_edge = plain(after) || space(after);
    // `attentionMarkers`: remark-gfm adds the tilde.
    let marker = |ch: Option<char>| matches!(ch, Some('*' | '_' | '~'));
    let open =
        !after_edge || plain(after) && (before_edge || before & (CJK | IVS) != 0) || marker(next);
    let close =
        !before_edge || plain(before) && (after_edge || after & CJK != 0) || marker(previous);
    if bytes[at.0] == b'*' {
        return (open, close);
    }
    let edge = |class: u16| class & (WHITESPACE | PUNCTUATION) != 0;
    (
        open && (edge(before) || !close),
        close && (edge(after) || !open),
    )
}
