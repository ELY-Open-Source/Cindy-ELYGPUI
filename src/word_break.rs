//! `Intl.Segmenter`'s words in Cindy's renderer: Chromium's ICU `word.txt` and its CJK engine.
use std::sync::LazyLock;

use icu_properties::{
    CodePointMapData,
    props::{GraphemeClusterBreak, LineBreak, Script, WordBreak},
};
use icu_segmenter::{WordSegmenter, options::WordBreakInvariantOptions};

use crate::{theme_import::color::js_trim, v8_classes::within};

mod cjk;
#[cfg(test)]
mod tests;

/// The class `word.txt` gives a character in its pair rules.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Cr,
    Lf,
    Newline,
    ExFm,
    Regional,
    Hebrew,
    Letter,
    Single,
    Double,
    MidNumLet,
    MidLetter,
    MidNum,
    Numeric,
    ExtendNumLet,
    Space,
    Hangul,
    Other,
}

/// A character as the rules and the dictionary engines see it.
#[derive(Clone, Copy)]
struct Class {
    kind: Kind,
    zwj: bool,
    katakana: bool,
    kana_kanji: bool,
    pict: bool,
    complex: bool,
    dictionary: bool,
    cj: bool,
    script: Script,
}

impl Class {
    fn letter(&self) -> bool {
        matches!(self.kind, Kind::Letter | Kind::Hebrew)
    }

    fn numeric(&self) -> bool {
        self.kind == Kind::Numeric
    }

    /// `$MidLetter | $MidNumLet | $Single_Quote`.
    fn mid(&self) -> bool {
        matches!(self.kind, Kind::MidLetter | Kind::MidNumLet | Kind::Single)
    }

    /// `$MidNum | $MidNumLet | $Single_Quote`.
    fn mid_num(&self) -> bool {
        matches!(self.kind, Kind::MidNum | Kind::MidNumLet | Kind::Single)
    }
}

/// What V8's Unicode holds apart from ICU4X's, from `scripts/v8-words.js`.
#[derive(serde::Deserialize)]
struct Held {
    unassigned: Vec<[u32; 2]>,
    pictographs: Vec<[u32; 2]>,
}

fn held() -> &'static Held {
    static HELD: LazyLock<Held> = LazyLock::new(|| {
        let text = include_str!("../resources/word-classes.json");
        serde_json::from_str(text).expect("the bundled word classes parse")
    });
    &HELD
}

/// `word.txt`'s character classes: a full stop is `$MidNum`, a colon no `$MidLetter`.
fn class(c: char) -> Class {
    let pict = within(&held().pictographs, c);
    if within(&held().unassigned, c) {
        // ICU4X's newer Unicode names characters V8 holds unassigned.
        return Class {
            kind: Kind::Other,
            zwj: false,
            katakana: false,
            kana_kanji: false,
            pict,
            complex: false,
            dictionary: false,
            cj: false,
            script: Script::Unknown,
        };
    }
    let word = CodePointMapData::<WordBreak>::new().get(c);
    let script = CodePointMapData::<Script>::new().get(c);
    let line = CodePointMapData::<LineBreak>::new().get(c);
    let cluster = CodePointMapData::<GraphemeClusterBreak>::new().get(c);
    let (han, complex) = (script == Script::Han, line == LineBreak::ComplexContext);
    let katakana = word == WordBreak::Katakana;
    let kana_kanji = han || script == Script::Hiragana || katakana;
    let hangul = ('\u{ac00}'..='\u{d7a3}').contains(&c);
    let extend = word == WordBreak::Extend && !han;
    let marks = extend || matches!(word, WordBreak::Format | WordBreak::ZWJ);
    let spelled = word == WordBreak::ALetter && !kana_kanji && !hangul;
    let letter = spelled || (complex && !extend && cluster != GraphemeClusterBreak::Control);
    let kind = match word {
        WordBreak::CR => Kind::Cr,
        WordBreak::LF => Kind::Lf,
        WordBreak::Newline => Kind::Newline,
        _ if marks => Kind::ExFm,
        _ if letter => Kind::Letter,
        _ if hangul => Kind::Hangul,
        WordBreak::RegionalIndicator => Kind::Regional,
        WordBreak::HebrewLetter => Kind::Hebrew,
        WordBreak::SingleQuote => Kind::Single,
        WordBreak::DoubleQuote => Kind::Double,
        WordBreak::MidNumLet if matches!(c, '.' | '\u{ff0e}') => Kind::MidNum,
        WordBreak::MidNumLet => Kind::MidNumLet,
        WordBreak::MidLetter if matches!(c, ':' | '\u{fe55}' | '\u{ff1a}') => Kind::Other,
        WordBreak::MidLetter => Kind::MidLetter,
        WordBreak::MidNum => Kind::MidNum,
        WordBreak::Numeric => Kind::Numeric,
        WordBreak::ExtendNumLet => Kind::ExtendNumLet,
        WordBreak::WSegSpace => Kind::Space,
        _ => Kind::Other,
    };
    let kana = matches!(script, Script::Hiragana | Script::Katakana);
    Class {
        kind,
        zwj: word == WordBreak::ZWJ,
        katakana,
        kana_kanji,
        pict,
        complex,
        dictionary: complex || kana_kanji || hangul,
        cj: han || kana || matches!(c, '\u{30fc}' | '\u{ff70}' | '\u{ff9e}' | '\u{ff9f}'),
        script,
    }
}

/// What stands before a character: the base, the one before it, the marks and flags since.
#[derive(Default)]
struct Before {
    base: Option<Class>,
    earlier: Option<Class>,
    marked: bool,
    zwj: bool,
    flags: usize,
}

/// Whether the rules keep `now` with what stands before it; `next` is the base after it.
fn joined(before: &Before, now: &Class, next: Option<Class>) -> bool {
    let Some(prev) = before.base else {
        return false;
    };
    if now.kind == Kind::ExFm {
        return !matches!(prev.kind, Kind::Cr | Kind::Lf | Kind::Newline);
    }
    let after = |test: fn(&Class) -> bool| next.is_some_and(|next| test(&next));
    let ahead = |test: fn(&Class) -> bool| before.earlier.is_some_and(|earlier| test(&earlier));
    let hebrew = |c: &Class| c.kind == Kind::Hebrew;
    let extends = |c: &Class| c.kind == Kind::ExtendNumLet;
    let word = |c: &Class| c.letter() || c.numeric() || c.katakana;
    let direct = !before.marked;
    (prev.kind == Kind::Cr && now.kind == Kind::Lf && direct)
        || (before.zwj && now.pict)
        || (prev.kind == Kind::Space && now.kind == Kind::Space && direct)
        || (prev.letter() && now.letter())
        || (prev.letter() && now.mid() && after(Class::letter))
        || (prev.mid() && now.letter() && ahead(Class::letter))
        || (hebrew(&prev) && now.kind == Kind::Single)
        || (hebrew(&prev) && now.kind == Kind::Double && after(hebrew))
        || (prev.kind == Kind::Double && hebrew(now) && ahead(hebrew))
        || (prev.numeric() && (now.numeric() || now.letter()))
        || (prev.letter() && now.numeric())
        || (prev.numeric() && now.mid_num() && after(Class::numeric))
        || (prev.mid_num() && now.numeric() && ahead(Class::numeric))
        || (prev.katakana && now.katakana)
        || ((word(&prev) || extends(&prev)) && extends(now))
        || (extends(&prev) && word(now))
        || (prev.kind == Kind::Regional && now.kind == Kind::Regional && before.flags % 2 == 1)
        || (prev.kind == Kind::Hangul && now.kind == Kind::Hangul && direct)
        || (prev.kana_kanji && now.kana_kanji && direct)
}

/// The bytes where the rules break the characters, the text's end among them.
fn rules(chars: &[(usize, Class)], end: usize) -> Vec<usize> {
    let mut cuts = Vec::new();
    let mut before = Before::default();
    for (nth, (at, now)) in chars.iter().enumerate() {
        let mut rest = chars[nth + 1..].iter().map(|(_, class)| *class);
        let next = rest.find(|class| class.kind != Kind::ExFm);
        let joins = joined(&before, now, next);
        if !joins && nth > 0 {
            cuts.push(*at);
        }
        before.zwj = now.zwj;
        if joins && now.kind == Kind::ExFm {
            before.marked = true;
            continue;
        }
        let flag = usize::from(now.kind == Kind::Regional);
        before.flags = if joins {
            before.flags * flag + flag
        } else {
            flag
        };
        before.earlier = before.base.filter(|_| joins);
        (before.base, before.marked) = (Some(*now), false);
    }
    if end > 0 {
        cuts.push(end);
    }
    cuts
}

/// The engines one call has met, as ICU lists them: the CJK one over the unhandled one.
#[derive(Default)]
struct Engines {
    cjk: bool,
    unhandled: Option<Script>,
}

/// `populateDictionary`: the breaks the dictionary engines add inside one piece of the rules.
fn divided(text: &str, piece: &[(usize, Class)], engines: &mut Engines, cuts: &mut Vec<usize>) {
    let mut i = 0;
    while i < piece.len() {
        let class = piece[i].1;
        let run = |test: &dyn Fn(&Class) -> bool| {
            let length = piece[i..].iter().take_while(|(_, c)| test(c)).count();
            let to = piece.get(i + length).map_or(text.len(), |(at, _)| *at);
            (length, to)
        };
        if !class.dictionary {
            i += 1;
        } else if class.cj && (engines.cjk || engines.unhandled != Some(class.script)) {
            engines.cjk = true;
            let (length, to) = run(&|c| c.cj);
            let inside: Vec<char> = text[piece[i].0..to].chars().collect();
            cuts.extend(cjk::inner(&inside).into_iter().map(|cut| piece[i + cut].0));
            i += length;
        } else if class.complex {
            // ICU's Thai, Lao, Khmer and Burmese engines are not ported: ICU4X's stands in.
            let (length, to) = run(&|c| c.complex && c.script == class.script);
            let from = piece[i].0;
            let words = WordSegmenter::new_dictionary(WordBreakInvariantOptions::default());
            let found = words.segment_str(&text[from..to]).map(|cut| from + cut);
            cuts.extend(found.filter(|cut| *cut > from && *cut < to));
            i += length;
        } else {
            engines.unhandled = Some(class.script);
            i += run(&|c| c.script == class.script).0;
        }
    }
}

/// The segments `new Intl.Segmenter(undefined, { granularity: 'word' })` cuts `text` into.
pub fn words(text: &str) -> Vec<&str> {
    let chars: Vec<(usize, Class)> = text.char_indices().map(|(at, c)| (at, class(c))).collect();
    let mut engines = Engines::default();
    let (mut cuts, mut from, mut first) = (Vec::new(), 0, 0);
    for end in rules(&chars, text.len()) {
        let last = first
            + chars[first..]
                .iter()
                .take_while(|(at, _)| *at < end)
                .count();
        // ICU asks no engine about a piece of one UTF-16 unit.
        if text[from..end].encode_utf16().count() > 1 {
            let piece: Vec<(usize, Class)> = chars[first..last].to_vec();
            divided(&text[..end], &piece, &mut engines, &mut cuts);
        }
        cuts.push(end);
        (from, first) = (end, last);
    }
    let starts = std::iter::once(0).chain(cuts.iter().copied());
    starts
        .zip(&cuts)
        .map(|(from, to)| &text[from..*to])
        .collect()
}

/// `splitWordsUncached`: the words, each run of white space kept with the word before it.
pub fn split(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for word in words(text) {
        match out.last_mut().filter(|_| js_trim(word).is_empty()) {
            Some(last) => last.push_str(word),
            None => out.push(word.to_owned()),
        }
    }
    out
}
