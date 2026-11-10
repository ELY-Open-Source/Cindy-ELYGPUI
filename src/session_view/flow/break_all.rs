//! Blink's `word-break: break-all`: the breaks its class table adds (`text_break_iterator.cc`).

use icu_properties::{CodePointMapData, props::LineBreak as Class};

/// `kBreakAllLineBreakClassTable`: a break the table adds between two classes.
fn added(last: Class, next: Class) -> bool {
    let (prefix, postfix) = (Class::PrefixNumeric, Class::PostfixNumeric);
    let more: &[Class] = match last {
        Class::Ambiguous
        | Class::Alphabetic
        | Class::BreakAfter
        | Class::Numeric
        | Class::ComplexContext
        | Class::BreakSymbols
        | Class::HebrewLetter => &[Class::OpenPunctuation, prefix, Class::ComplexContext],
        Class::ClosePunctuation => &[prefix],
        Class::Exclamation | Class::PostfixNumeric => &[postfix, prefix],
        Class::InfixNumeric => &[],
        Class::CloseParenthesis => &[prefix, Class::ComplexContext],
        Class::Hyphen => return next == Class::Numeric,
        Class::PrefixNumeric => return next == postfix,
        _ => return false,
    };
    let common = [
        Class::Ambiguous,
        Class::Alphabetic,
        Class::BreakAfter,
        Class::Hyphen,
        Class::Numeric,
        Class::HebrewLetter,
    ];
    common.contains(&next) || more.contains(&next)
}

/// The bytes of `text` before which `break-all` adds a break.
pub fn breaks(text: &str) -> Vec<usize> {
    let classes = CodePointMapData::<Class>::new();
    let (mut found, mut last) = (Vec::new(), None);
    for (at, ch) in text.char_indices() {
        // `LineBreakPropertyValue`: IE's `+` is a letter here.
        let class = match ch {
            '+' => Class::Alphabetic,
            _ => classes.get(ch),
        };
        if last.is_some_and(|last| added(last, class)) {
            found.push(at);
        }
        if class != Class::CombiningMark {
            last = Some(class);
        }
    }
    found
}
